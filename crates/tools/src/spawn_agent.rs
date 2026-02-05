//! Sub-agent tool: lets the LLM delegate tasks to a child agent loop.

use std::sync::Arc;

use {anyhow::Result, async_trait::async_trait, tokio::sync::RwLock, tracing::info};

use {
    moltis_agents::{
        model::LlmProvider,
        providers::ProviderRegistry,
        runner::{RunnerEvent, run_agent_loop_with_context},
        tool_registry::{AgentTool, ToolRegistry},
    },
    moltis_config::schema::AgentsConfig,
};

/// Maximum nesting depth for sub-agents (prevents infinite recursion).
const MAX_SPAWN_DEPTH: u64 = 3;

/// Tool parameter injected via `tool_context` to track nesting depth.
const SPAWN_DEPTH_KEY: &str = "_spawn_depth";

/// A tool that spawns a sub-agent running its own agent loop.
///
/// The sub-agent executes synchronously (blocks until done) and its result
/// is returned as the tool output. Sub-agents get a filtered copy of the
/// parent's tool registry (without the `spawn_agent` tool itself) and a
/// focused system prompt.
///
/// When a preset is specified, the sub-agent uses that preset's model,
/// tool policies, and system prompt additions.
///
/// Callback for emitting events from the sub-agent back to the parent UI.
pub type OnSpawnEvent = Arc<dyn Fn(RunnerEvent) + Send + Sync>;

pub struct SpawnAgentTool {
    provider_registry: Arc<RwLock<ProviderRegistry>>,
    default_provider: Arc<dyn LlmProvider>,
    tool_registry: Arc<ToolRegistry>,
    agents_config: Arc<RwLock<AgentsConfig>>,
    on_event: Option<OnSpawnEvent>,
}

impl SpawnAgentTool {
    pub fn new(
        provider_registry: Arc<RwLock<ProviderRegistry>>,
        default_provider: Arc<dyn LlmProvider>,
        tool_registry: Arc<ToolRegistry>,
        agents_config: Arc<RwLock<AgentsConfig>>,
    ) -> Self {
        Self {
            provider_registry,
            default_provider,
            tool_registry,
            agents_config,
            on_event: None,
        }
    }

    /// Set an event callback so sub-agent activity is visible to the UI.
    pub fn with_on_event(mut self, on_event: OnSpawnEvent) -> Self {
        self.on_event = Some(on_event);
        self
    }

    fn emit(&self, event: RunnerEvent) {
        if let Some(ref cb) = self.on_event {
            cb(event);
        }
    }
}

#[async_trait]
impl AgentTool for SpawnAgentTool {
    fn name(&self) -> &str {
        "spawn_agent"
    }

    fn description(&self) -> &str {
        "Spawn a sub-agent to handle a complex, multi-step task autonomously. \
         The sub-agent runs its own agent loop with access to tools and returns \
         the result when done. Use this to delegate tasks that require multiple \
         tool calls or independent reasoning."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "description": "The task to delegate to the sub-agent"
                },
                "context": {
                    "type": "string",
                    "description": "Additional context for the sub-agent (optional)"
                },
                "model": {
                    "type": "string",
                    "description": "Model ID to use (e.g. a cheaper model). If not specified, uses preset model or parent's model."
                },
                "preset": {
                    "type": "string",
                    "description": "Agent preset name (e.g. 'researcher', 'coder', 'reviewer'). Presets define model, tool policies, and behavior."
                }
            },
            "required": ["task"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> Result<serde_json::Value> {
        let task = params["task"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing required parameter: task"))?;
        let context = params["context"].as_str().unwrap_or("");
        let explicit_model_id = params["model"].as_str();
        let preset_name = params["preset"].as_str();

        // Check nesting depth.
        let depth = params
            .get(SPAWN_DEPTH_KEY)
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        if depth >= MAX_SPAWN_DEPTH {
            anyhow::bail!("maximum sub-agent nesting depth ({MAX_SPAWN_DEPTH}) exceeded");
        }

        // Load preset configuration if specified.
        let agents_config = self.agents_config.read().await;
        let preset = preset_name.and_then(|name| agents_config.get_preset(name));

        // Warn if preset was requested but not found.
        if let Some(name) = preset_name
            && preset.is_none()
        {
            info!(
                preset = name,
                "preset not found, using default configuration"
            );
        }

        // Determine model: explicit > preset > default.
        let model_id_to_use = explicit_model_id
            .map(String::from)
            .or_else(|| preset.and_then(|p| p.model.clone()));

        // Resolve provider.
        let provider = if let Some(ref id) = model_id_to_use {
            let reg = self.provider_registry.read().await;
            reg.get(id)
                .ok_or_else(|| anyhow::anyhow!("unknown model: {id}"))?
        } else {
            Arc::clone(&self.default_provider)
        };

        // Capture model ID before provider is moved into the sub-agent loop.
        let model_id = provider.id().to_string();

        // Build identity info for logging/events.
        let preset_identity_name = preset
            .and_then(|p| p.identity.name.as_ref())
            .map(String::as_str);

        info!(
            task = %task,
            depth = depth,
            model = %model_id,
            preset = ?preset_name,
            identity = ?preset_identity_name,
            "spawning sub-agent"
        );

        self.emit(RunnerEvent::SubAgentStart {
            task: task.to_string(),
            model: model_id.clone(),
            depth,
        });

        // Build filtered tool registry based on preset policy.
        let sub_tools = if let Some(p) = preset {
            self.tool_registry.clone_with_policy(
                &p.tools.allow,
                &p.tools.deny,
                &["spawn_agent"], // Always exclude spawn_agent
            )
        } else {
            // Default: exclude only spawn_agent to prevent recursive spawning.
            self.tool_registry.clone_without(&["spawn_agent"])
        };

        // Build system prompt with preset customizations.
        let system_prompt = build_sub_agent_prompt(task, context, preset);

        // Build tool context with incremented depth and propagated session key.
        let mut tool_context = serde_json::json!({
            SPAWN_DEPTH_KEY: depth + 1,
        });
        if let Some(session_key) = params.get("_session_key") {
            tool_context["_session_key"] = session_key.clone();
        }

        // Drop the read lock before running the agent loop.
        drop(agents_config);

        // Run the sub-agent loop (no event forwarding, no hooks, no history).
        let result = run_agent_loop_with_context(
            provider,
            &sub_tools,
            &system_prompt,
            task,
            None,
            None, // no history
            Some(tool_context),
            None, // no hooks for sub-agents
        )
        .await;

        // Emit SubAgentEnd regardless of success/failure.
        let (iterations, tool_calls_made) = match &result {
            Ok(r) => (r.iterations, r.tool_calls_made),
            Err(_) => (0, 0),
        };
        self.emit(RunnerEvent::SubAgentEnd {
            task: task.to_string(),
            model: model_id.clone(),
            depth,
            iterations,
            tool_calls_made,
        });

        let result = result?;

        info!(
            task = %task,
            depth = depth,
            iterations = result.iterations,
            tool_calls = result.tool_calls_made,
            preset = ?preset_name,
            "sub-agent completed"
        );

        Ok(serde_json::json!({
            "text": result.text,
            "iterations": result.iterations,
            "tool_calls_made": result.tool_calls_made,
            "model": model_id,
            "preset": preset_name,
        }))
    }
}

/// Build the system prompt for a sub-agent, incorporating preset customizations.
fn build_sub_agent_prompt(
    task: &str,
    context: &str,
    preset: Option<&moltis_config::schema::AgentPreset>,
) -> String {
    let mut prompt = String::new();

    // Add preset identity if available.
    if let Some(p) = preset {
        if let Some(ref name) = p.identity.name {
            prompt.push_str(&format!("You are {name}"));
            if let Some(ref creature) = p.identity.creature {
                prompt.push_str(&format!(", a {creature}"));
            }
            prompt.push_str(". ");
        }
        if let Some(ref vibe) = p.identity.vibe {
            prompt.push_str(&format!("Your style is {vibe}. "));
        }
        if let Some(ref soul) = p.identity.soul {
            prompt.push_str(soul);
            prompt.push(' ');
        }
    }

    // Add base instruction.
    if prompt.is_empty() {
        prompt.push_str("You are a sub-agent spawned to handle a specific task. ");
    }
    prompt.push_str("Complete the task thoroughly and return a clear result.\n\n");

    // Add task.
    prompt.push_str(&format!("Task: {task}"));

    // Add context if provided.
    if !context.is_empty() {
        prompt.push_str(&format!("\n\nContext: {context}"));
    }

    // Add preset system prompt suffix.
    if let Some(p) = preset
        && let Some(ref suffix) = p.system_prompt_suffix
    {
        prompt.push_str("\n\n");
        prompt.push_str(suffix);
    }

    prompt
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        moltis_agents::model::{CompletionResponse, StreamEvent, Usage},
        moltis_config::schema::{AgentIdentity, AgentPreset, PresetToolPolicy},
        std::{collections::HashMap, pin::Pin},
        tokio_stream::Stream,
    };

    /// Mock provider that returns a fixed response.
    struct MockProvider {
        response: String,
        model_id: String,
    }

    #[async_trait]
    impl LlmProvider for MockProvider {
        fn name(&self) -> &str {
            "mock"
        }

        fn id(&self) -> &str {
            &self.model_id
        }

        async fn complete(
            &self,
            _messages: &[serde_json::Value],
            _tools: &[serde_json::Value],
        ) -> Result<CompletionResponse> {
            Ok(CompletionResponse {
                text: Some(self.response.clone()),
                tool_calls: vec![],
                usage: Usage {
                    input_tokens: 10,
                    output_tokens: 5,
                },
            })
        }

        fn stream(
            &self,
            _messages: Vec<serde_json::Value>,
        ) -> Pin<Box<dyn Stream<Item = StreamEvent> + Send + '_>> {
            Box::pin(tokio_stream::empty())
        }
    }

    fn make_empty_provider_registry() -> Arc<RwLock<ProviderRegistry>> {
        Arc::new(RwLock::new(ProviderRegistry::from_env_with_config(
            &Default::default(),
        )))
    }

    fn make_empty_agents_config() -> Arc<RwLock<AgentsConfig>> {
        Arc::new(RwLock::new(AgentsConfig::default()))
    }

    fn make_spawn_tool(
        provider: Arc<dyn LlmProvider>,
        tool_registry: Arc<ToolRegistry>,
    ) -> SpawnAgentTool {
        SpawnAgentTool::new(
            make_empty_provider_registry(),
            provider,
            tool_registry,
            make_empty_agents_config(),
        )
    }

    #[tokio::test]
    async fn test_sub_agent_runs_and_returns_result() {
        let provider: Arc<dyn LlmProvider> = Arc::new(MockProvider {
            response: "Sub-agent result".into(),
            model_id: "mock-model".into(),
        });
        let tool_registry = Arc::new(ToolRegistry::new());
        let spawn_tool = make_spawn_tool(Arc::clone(&provider), tool_registry);

        let params = serde_json::json!({ "task": "do something" });
        let result = spawn_tool.execute(params).await.unwrap();

        assert_eq!(result["text"], "Sub-agent result");
        assert_eq!(result["iterations"], 1);
        assert_eq!(result["tool_calls_made"], 0);
        assert_eq!(result["model"], "mock-model");
    }

    #[tokio::test]
    async fn test_depth_limit_rejects() {
        let provider: Arc<dyn LlmProvider> = Arc::new(MockProvider {
            response: "nope".into(),
            model_id: "mock".into(),
        });
        let tool_registry = Arc::new(ToolRegistry::new());
        let spawn_tool = make_spawn_tool(provider, tool_registry);

        let params = serde_json::json!({
            "task": "do something",
            "_spawn_depth": MAX_SPAWN_DEPTH,
        });
        let result = spawn_tool.execute(params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("nesting depth"));
    }

    #[tokio::test]
    async fn test_spawn_agent_excluded_from_sub_registry() {
        let provider: Arc<dyn LlmProvider> = Arc::new(MockProvider {
            response: "ok".into(),
            model_id: "mock".into(),
        });

        // Create a registry with spawn_agent in it.
        let mut registry = ToolRegistry::new();

        struct DummyTool;
        #[async_trait]
        impl AgentTool for DummyTool {
            fn name(&self) -> &str {
                "spawn_agent"
            }

            fn description(&self) -> &str {
                "dummy"
            }

            fn parameters_schema(&self) -> serde_json::Value {
                serde_json::json!({})
            }

            async fn execute(&self, _: serde_json::Value) -> Result<serde_json::Value> {
                Ok(serde_json::json!("dummy"))
            }
        }

        struct EchoTool;
        #[async_trait]
        impl AgentTool for EchoTool {
            fn name(&self) -> &str {
                "echo"
            }

            fn description(&self) -> &str {
                "echo"
            }

            fn parameters_schema(&self) -> serde_json::Value {
                serde_json::json!({})
            }

            async fn execute(&self, p: serde_json::Value) -> Result<serde_json::Value> {
                Ok(p)
            }
        }

        registry.register(Box::new(DummyTool));
        registry.register(Box::new(EchoTool));

        let filtered = registry.clone_without(&["spawn_agent"]);
        assert!(filtered.get("spawn_agent").is_none());
        assert!(filtered.get("echo").is_some());

        // Also verify schemas don't include spawn_agent.
        let schemas = filtered.list_schemas();
        assert_eq!(schemas.len(), 1);
        assert_eq!(schemas[0]["name"], "echo");

        // Ensure original is unaffected.
        assert!(registry.get("spawn_agent").is_some());

        // The SpawnAgentTool itself should work with the filtered registry.
        let spawn_tool = make_spawn_tool(provider, Arc::new(registry));
        let result = spawn_tool
            .execute(serde_json::json!({ "task": "test" }))
            .await
            .unwrap();
        assert_eq!(result["text"], "ok");
    }

    #[tokio::test]
    async fn test_context_passed_to_sub_agent() {
        let provider: Arc<dyn LlmProvider> = Arc::new(MockProvider {
            response: "done with context".into(),
            model_id: "mock".into(),
        });
        let spawn_tool = make_spawn_tool(provider, Arc::new(ToolRegistry::new()));

        let params = serde_json::json!({
            "task": "analyze code",
            "context": "The code is in src/main.rs",
        });
        let result = spawn_tool.execute(params).await.unwrap();
        assert_eq!(result["text"], "done with context");
    }

    #[tokio::test]
    async fn test_missing_task_parameter() {
        let provider: Arc<dyn LlmProvider> = Arc::new(MockProvider {
            response: "nope".into(),
            model_id: "mock".into(),
        });
        let spawn_tool = make_spawn_tool(provider, Arc::new(ToolRegistry::new()));

        let result = spawn_tool.execute(serde_json::json!({})).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("task"));
    }

    #[tokio::test]
    async fn test_preset_applies_tool_policy() {
        // Create a registry with multiple tools.
        let mut registry = ToolRegistry::new();

        struct ReadTool;
        #[async_trait]
        impl AgentTool for ReadTool {
            fn name(&self) -> &str {
                "read_file"
            }

            fn description(&self) -> &str {
                "read"
            }

            fn parameters_schema(&self) -> serde_json::Value {
                serde_json::json!({})
            }

            async fn execute(&self, _: serde_json::Value) -> Result<serde_json::Value> {
                Ok(serde_json::json!("read"))
            }
        }

        struct ExecTool;
        #[async_trait]
        impl AgentTool for ExecTool {
            fn name(&self) -> &str {
                "exec"
            }

            fn description(&self) -> &str {
                "exec"
            }

            fn parameters_schema(&self) -> serde_json::Value {
                serde_json::json!({})
            }

            async fn execute(&self, _: serde_json::Value) -> Result<serde_json::Value> {
                Ok(serde_json::json!("exec"))
            }
        }

        registry.register(Box::new(ReadTool));
        registry.register(Box::new(ExecTool));

        // Test clone_with_policy with allow list.
        let filtered = registry.clone_with_policy(&["read_file".into()], &[], &["spawn_agent"]);
        assert!(filtered.get("read_file").is_some());
        assert!(filtered.get("exec").is_none());

        // Test clone_with_policy with deny list.
        let filtered2 = registry.clone_with_policy(&[], &["exec".into()], &["spawn_agent"]);
        assert!(filtered2.get("read_file").is_some());
        assert!(filtered2.get("exec").is_none());
    }

    #[tokio::test]
    async fn test_preset_with_identity_builds_correct_prompt() {
        let preset = AgentPreset {
            identity: AgentIdentity {
                name: Some("scout".into()),
                creature: Some("helpful owl".into()),
                vibe: Some("focused and efficient".into()),
                soul: Some("I love finding information.".into()),
                ..Default::default()
            },
            system_prompt_suffix: Some("Focus on accuracy over speed.".into()),
            ..Default::default()
        };

        let prompt = build_sub_agent_prompt("find bugs", "in main.rs", Some(&preset));

        assert!(prompt.contains("You are scout"));
        assert!(prompt.contains("a helpful owl"));
        assert!(prompt.contains("focused and efficient"));
        assert!(prompt.contains("I love finding information"));
        assert!(prompt.contains("Task: find bugs"));
        assert!(prompt.contains("Context: in main.rs"));
        assert!(prompt.contains("Focus on accuracy over speed"));
    }

    #[tokio::test]
    async fn test_preset_returns_in_result() {
        let provider: Arc<dyn LlmProvider> = Arc::new(MockProvider {
            response: "researched".into(),
            model_id: "mock".into(),
        });

        // Create agents config with a preset.
        let mut presets = HashMap::new();
        presets.insert("researcher".into(), AgentPreset {
            identity: AgentIdentity {
                name: Some("scout".into()),
                ..Default::default()
            },
            tools: PresetToolPolicy {
                allow: vec![],
                deny: vec!["exec".into()],
            },
            ..Default::default()
        });
        let agents_config = Arc::new(RwLock::new(AgentsConfig {
            presets,
            ..Default::default()
        }));

        let spawn_tool = SpawnAgentTool::new(
            make_empty_provider_registry(),
            provider,
            Arc::new(ToolRegistry::new()),
            agents_config,
        );

        let params = serde_json::json!({
            "task": "find patterns",
            "preset": "researcher",
        });
        let result = spawn_tool.execute(params).await.unwrap();

        assert_eq!(result["text"], "researched");
        assert_eq!(result["preset"], "researcher");
    }
}
