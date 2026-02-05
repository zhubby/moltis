# Multi-Agent Architecture Design

This document outlines the design for multi-agent support in Moltis, enabling:
1. Named agent presets with specialized configurations
2. Multiple isolated agents in a single gateway
3. Multi-instance control plane (view multiple Moltis instances in one UI)
4. Swarm orchestration patterns

## Background

### Current State

Moltis currently supports:
- **Sub-agent spawning** via `spawn_agent` tool (depth limit: 3)
- **Single agent identity** per gateway instance
- **Per-session state** with sequential execution locks
- **Event broadcasting** via WebSocket for sub-agent lifecycle

### Industry Patterns

Research of Claude Code, Goose, CrewAI, LangGraph, and others reveals common patterns:
- **Context isolation**: Sub-agents get clean contexts, only results return
- **Role-based presets**: Agents specialize (researcher, coder, reviewer)
- **Parallel execution**: Multiple agents run concurrently (typically 7-10 max)
- **Event buses**: Agents emit status signals for observability
- **Handoff mechanisms**: Agentic (full history) vs programmatic (data only)

---

## Phase 1: Agent Presets (Named Agent Types)

### Concept

Define reusable agent configurations ("presets") that can be selected when spawning sub-agents or starting sessions. Each preset specifies:
- Identity (name, emoji, vibe)
- Default model (e.g., use cheaper models for research)
- Tool restrictions (allow/deny specific tools)
- System prompt additions
- Resource limits

### Configuration Schema

```toml
# moltis.toml

[agents]
# Default agent (used when no preset specified)
default = "assistant"

[agents.presets.assistant]
identity.name = "moltis"
identity.emoji = "🤖"
identity.vibe = "helpful and thorough"

[agents.presets.researcher]
identity.name = "scout"
identity.emoji = "🔍"
identity.vibe = "focused on finding information"
model = "anthropic/claude-haiku-3-5-20241022"
tools.allow = ["web_search", "web_fetch", "read_file", "glob", "grep"]
tools.deny = ["exec", "write_file"]
system_prompt_suffix = """
You are a research specialist. Focus on gathering information efficiently.
Do not modify files or execute commands - only search and report findings.
"""

[agents.presets.coder]
identity.name = "forge"
identity.emoji = "⚡"
identity.vibe = "efficient and precise"
model = "anthropic/claude-sonnet-4-20250514"
tools.allow = ["exec", "read_file", "write_file", "glob", "grep"]
tools.deny = ["spawn_agent"]  # No delegation
max_iterations = 50

[agents.presets.reviewer]
identity.name = "lens"
identity.emoji = "🔬"
identity.vibe = "thorough code reviewer"
model = "anthropic/claude-sonnet-4-20250514"
tools.allow = ["read_file", "glob", "grep"]
tools.deny = ["exec", "write_file"]
system_prompt_suffix = """
You are a code reviewer. Analyze code for bugs, security issues, and style.
Never modify files - only provide analysis and recommendations.
"""
```

### Rust Types

```rust
// crates/config/src/schema.rs

/// Named agent presets configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentsConfig {
    /// Default preset name when none specified.
    pub default: Option<String>,
    /// Named presets keyed by preset name.
    #[serde(default)]
    pub presets: HashMap<String, AgentPreset>,
}

/// A single agent preset definition.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentPreset {
    /// Agent identity overrides.
    pub identity: AgentIdentity,
    /// Model override (provider/model format).
    pub model: Option<String>,
    /// Tool policy for this preset.
    pub tools: PresetToolPolicy,
    /// Additional system prompt text appended to base prompt.
    pub system_prompt_suffix: Option<String>,
    /// Maximum iterations for agent loop.
    pub max_iterations: Option<u32>,
    /// Timeout override in seconds.
    pub timeout_secs: Option<u64>,
}

/// Tool policy within a preset.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PresetToolPolicy {
    pub allow: Vec<String>,
    pub deny: Vec<String>,
}
```

### spawn_agent Enhancement

```rust
// Updated parameters_schema
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
                "description": "Model ID override"
            },
            "preset": {
                "type": "string",
                "description": "Agent preset name (e.g. 'researcher', 'coder', 'reviewer')"
            }
        },
        "required": ["task"]
    })
}
```

### UI Changes

Add preset selector in session settings and spawn-agent tool cards:

```html
<!-- Session header preset indicator -->
<span class="agent-preset-badge" data-preset="researcher">
  🔍 scout
</span>
```

---

## Phase 2: Multiple Agents per Gateway

### Concept

Run multiple isolated agents within a single gateway, each with:
- Separate workspace/working directory
- Independent session storage
- Own identity and configuration
- Isolated tool state

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        Gateway                               │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │
│  │   Agent A   │  │   Agent B   │  │   Agent C   │          │
│  │  "moltis"   │  │  "scout"    │  │  "forge"    │          │
│  │  ~/project  │  │  ~/docs     │  │  ~/backend  │          │
│  │  sessions/a │  │  sessions/b │  │  sessions/c │          │
│  └─────────────┘  └─────────────┘  └─────────────┘          │
│         │                │                │                  │
│         └────────────────┴────────────────┘                  │
│                          │                                   │
│                    Message Router                            │
│                          │                                   │
│              ┌───────────┴───────────┐                       │
│              │     WebSocket Hub     │                       │
│              └───────────────────────┘                       │
└─────────────────────────────────────────────────────────────┘
```

### Configuration

```toml
# moltis.toml

[gateway]
multi_agent = true

[[gateway.agents]]
id = "main"
preset = "assistant"
workspace = "~/projects/webapp"
sessions_dir = "sessions/main"

[[gateway.agents]]
id = "docs"
preset = "researcher"
workspace = "~/projects/webapp/docs"
sessions_dir = "sessions/docs"

[[gateway.agents]]
id = "backend"
preset = "coder"
workspace = "~/projects/webapp/backend"
sessions_dir = "sessions/backend"
sandbox.enabled = true
sandbox.image = "moltis-sandbox:backend"
```

### Rust Types

```rust
/// Multi-agent gateway configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GatewayAgentsConfig {
    /// Enable multi-agent mode.
    pub multi_agent: bool,
    /// Agent definitions (only used when multi_agent = true).
    #[serde(default)]
    pub agents: Vec<AgentInstanceConfig>,
}

/// Configuration for a single agent instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInstanceConfig {
    /// Unique identifier for this agent.
    pub id: String,
    /// Preset to use (from [agents.presets]).
    pub preset: Option<String>,
    /// Override identity.
    pub identity: Option<AgentIdentity>,
    /// Working directory for this agent.
    pub workspace: Option<PathBuf>,
    /// Sessions directory (relative to data_dir).
    pub sessions_dir: Option<String>,
    /// Per-agent sandbox configuration overrides.
    pub sandbox: Option<SandboxConfig>,
    /// Per-agent MCP servers.
    pub mcp: Option<McpConfig>,
    /// Channel bindings (route messages to this agent).
    pub channels: Vec<ChannelBinding>,
}

/// Route a channel to a specific agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelBinding {
    /// Channel type (e.g., "telegram").
    pub channel_type: String,
    /// Channel identifier pattern (e.g., "telegram:*:123456").
    pub pattern: String,
}
```

### State Management

```rust
// crates/gateway/src/state.rs

pub struct GatewayState {
    // Existing fields...

    /// Agent instances (keyed by agent_id).
    pub agents: HashMap<String, AgentInstance>,

    /// Connection to agent mapping.
    pub conn_agent: HashMap<String, String>,
}

pub struct AgentInstance {
    pub id: String,
    pub config: AgentInstanceConfig,
    pub chat_service: Arc<dyn ChatService>,
    pub session_store: Arc<dyn SessionService>,
    pub tool_registry: Arc<ToolRegistry>,
    pub working_dir: PathBuf,
}
```

### API Changes

```rust
// RPC methods gain agent_id parameter

// sessions.list now supports agent filter
{
    "method": "sessions.list",
    "params": {
        "agent_id": "main",  // optional filter
        "project_id": "..."
    }
}

// chat.send routes to specific agent
{
    "method": "chat.send",
    "params": {
        "text": "...",
        "agent_id": "backend",  // explicit routing
        "_session_key": "..."
    }
}

// New: agents.list
{
    "method": "agents.list"
}
// Returns:
[
    {"id": "main", "preset": "assistant", "identity": {...}, "workspace": "..."},
    {"id": "docs", "preset": "researcher", "identity": {...}, "workspace": "..."}
]

// New: agents.switch (set active agent for connection)
{
    "method": "agents.switch",
    "params": {"agent_id": "backend"}
}
```

### UI Changes

Add agent selector in the navigation:

```javascript
// agents.js - Agent selector component
function AgentSelector({ agents, activeAgentId, onSwitch }) {
    return html`
        <div class="agent-selector">
            ${agents.map(agent => html`
                <button
                    class="agent-btn ${agent.id === activeAgentId ? 'active' : ''}"
                    onClick=${() => onSwitch(agent.id)}
                >
                    <span class="agent-emoji">${agent.identity?.emoji || '🤖'}</span>
                    <span class="agent-name">${agent.identity?.name || agent.id}</span>
                    <span class="agent-workspace">${agent.workspace}</span>
                </button>
            `)}
        </div>
    `;
}
```

---

## Phase 3: Multi-Instance Control Plane

### Concept

View and manage multiple independent Moltis instances from a single UI. This supports:
- Running Moltis on multiple machines/VMs
- Connecting to remote Moltis gateways
- Unified dashboard across instances

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     Control Plane UI                             │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐             │
│  │ Node A  │  │ Node B  │  │ Node C  │  │ Node D  │             │
│  │ local   │  │ server1 │  │ server2 │  │ laptop  │             │
│  │ :8765   │  │ :443    │  │ :443    │  │ :8765   │             │
│  └────┬────┘  └────┬────┘  └────┬────┘  └────┬────┘             │
│       │            │            │            │                   │
│       └────────────┴────────────┴────────────┘                   │
│                          │                                       │
│                   WebSocket Manager                              │
│                   (multi-connection)                             │
└─────────────────────────────────────────────────────────────────┘
         │              │              │              │
         ▼              ▼              ▼              ▼
    ┌─────────┐    ┌─────────┐   ┌─────────┐   ┌─────────┐
    │ Moltis  │    │ Moltis  │   │ Moltis  │   │ Moltis  │
    │ Gateway │    │ Gateway │   │ Gateway │   │ Gateway │
    │ (local) │    │(remote) │   │(remote) │   │ (local) │
    └─────────┘    └─────────┘   └─────────┘   └─────────┘
```

### Node Configuration

Stored in browser localStorage or synced via a config endpoint:

```json
{
  "nodes": [
    {
      "id": "local",
      "name": "Local Dev",
      "url": "wss://localhost:8765",
      "apiKey": null
    },
    {
      "id": "prod-1",
      "name": "Production Server",
      "url": "wss://moltis.example.com",
      "apiKey": "moltis_..."
    },
    {
      "id": "staging",
      "name": "Staging",
      "url": "wss://staging.moltis.example.com",
      "apiKey": "moltis_..."
    }
  ],
  "activeNodeId": "local"
}
```

### WebSocket Manager

```javascript
// websocket-manager.js

class MultiNodeWebSocketManager {
    constructor() {
        this.connections = new Map();  // nodeId -> WebSocket
        this.eventHandlers = new Map(); // eventName -> Set<handler>
    }

    async connect(node) {
        const ws = new WebSocket(node.url);

        ws.onopen = () => {
            this.send(node.id, {
                type: 'request',
                method: 'connect',
                params: {
                    client: 'moltis-control-plane',
                    apiKey: node.apiKey,
                }
            });
        };

        ws.onmessage = (event) => {
            const frame = JSON.parse(event.data);
            // Prefix events with node ID
            this.emit(`${node.id}:${frame.event}`, frame.payload);
            // Also emit unprefixed for global handlers
            this.emit(frame.event, { nodeId: node.id, ...frame.payload });
        };

        this.connections.set(node.id, ws);
    }

    send(nodeId, message) {
        const ws = this.connections.get(nodeId);
        if (ws?.readyState === WebSocket.OPEN) {
            ws.send(JSON.stringify(message));
        }
    }

    broadcast(method, params) {
        for (const nodeId of this.connections.keys()) {
            this.send(nodeId, { type: 'request', method, params });
        }
    }
}
```

### UI: Nodes Dashboard

```javascript
// page-nodes.js

function NodesPage() {
    const [nodes, setNodes] = useState([]);
    const [nodeStatus, setNodeStatus] = useState({});  // nodeId -> status

    useEffect(() => {
        const off = onEvent('node.status', ({ nodeId, status }) => {
            setNodeStatus(prev => ({ ...prev, [nodeId]: status }));
        });
        return off;
    }, []);

    return html`
        <div class="nodes-page">
            <header class="nodes-header">
                <h1>Moltis Instances</h1>
                <button class="provider-btn" onClick=${addNode}>
                    Add Instance
                </button>
            </header>

            <div class="nodes-grid">
                ${nodes.map(node => html`
                    <${NodeCard}
                        node=${node}
                        status=${nodeStatus[node.id]}
                        onSelect=${() => switchToNode(node.id)}
                        onRemove=${() => removeNode(node.id)}
                    />
                `)}
            </div>
        </div>
    `;
}

function NodeCard({ node, status, onSelect, onRemove }) {
    return html`
        <div class="node-card ${status?.connected ? 'connected' : 'disconnected'}">
            <div class="node-header">
                <span class="node-indicator"></span>
                <h3>${node.name}</h3>
            </div>

            <div class="node-info">
                <div class="node-url">${node.url}</div>
                ${status?.connected && html`
                    <div class="node-stats">
                        <span>Sessions: ${status.sessionCount}</span>
                        <span>Active: ${status.activeRuns}</span>
                    </div>
                `}
            </div>

            <div class="node-actions">
                <button class="provider-btn provider-btn-secondary" onClick=${onSelect}>
                    Open
                </button>
                <button class="provider-btn provider-btn-danger" onClick=${onRemove}>
                    Remove
                </button>
            </div>
        </div>
    `;
}
```

### Split-View Mode

View multiple agents/instances side by side:

```javascript
// page-split.js

function SplitViewPage() {
    const [panes, setPanes] = useState([
        { id: 'left', nodeId: 'local', sessionKey: 'main' },
        { id: 'right', nodeId: 'prod-1', sessionKey: 'deploy' },
    ]);

    return html`
        <div class="split-view">
            ${panes.map(pane => html`
                <div class="split-pane" key=${pane.id}>
                    <${PaneHeader}
                        pane=${pane}
                        onChangeNode=${(nodeId) => updatePane(pane.id, { nodeId })}
                        onChangeSession=${(key) => updatePane(pane.id, { sessionKey: key })}
                    />
                    <${ChatView}
                        nodeId=${pane.nodeId}
                        sessionKey=${pane.sessionKey}
                    />
                </div>
            `)}

            <button class="add-pane-btn" onClick=${addPane}>+</button>
        </div>
    `;
}
```

---

## Phase 4: Swarm Orchestration

### Concept

A supervisor agent coordinates multiple worker agents for complex tasks:
- Decomposes tasks into subtasks
- Spawns parallel workers
- Monitors progress via event stream
- Aggregates results

### Swarm Skill

```markdown
# /swarm skill

You are a swarm coordinator. When the user gives you a complex task:

1. **Decompose**: Break the task into independent subtasks
2. **Spawn Workers**: Use spawn_agent with appropriate presets:
   - "researcher" for information gathering
   - "coder" for implementation
   - "reviewer" for validation
3. **Monitor**: Watch for SubAgentEnd events
4. **Aggregate**: Combine results into a coherent response

## Worker Coordination Rules

- Maximum 5 concurrent workers
- Each worker gets a focused, single-purpose task
- Workers cannot spawn sub-workers (depth=2 limit)
- If a worker fails, retry once with clarified instructions
- Aggregate results only after all workers complete

## Example Decomposition

User: "Add user authentication to the API"

Workers:
1. researcher: "Find existing auth patterns in the codebase"
2. coder: "Implement JWT token generation"
3. coder: "Add login/logout endpoints"
4. coder: "Add auth middleware"
5. reviewer: "Review auth implementation for security issues"
```

### Swarm Tool

```rust
// crates/tools/src/swarm.rs

/// Tool for parallel sub-agent orchestration.
pub struct SwarmTool {
    spawn_tool: Arc<SpawnAgentTool>,
    max_concurrent: usize,
}

#[async_trait]
impl AgentTool for SwarmTool {
    fn name(&self) -> &str {
        "swarm"
    }

    fn description(&self) -> &str {
        "Execute multiple sub-agent tasks in parallel. Each task runs \
         concurrently and results are aggregated."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "tasks": {
                    "type": "array",
                    "description": "List of tasks to execute in parallel",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": {"type": "string"},
                            "task": {"type": "string"},
                            "preset": {"type": "string"},
                            "context": {"type": "string"}
                        },
                        "required": ["id", "task"]
                    }
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Timeout for all tasks (default: 300)"
                }
            },
            "required": ["tasks"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> Result<serde_json::Value> {
        let tasks: Vec<SwarmTask> = serde_json::from_value(
            params["tasks"].clone()
        )?;

        // Limit concurrent tasks
        let semaphore = Arc::new(Semaphore::new(self.max_concurrent));

        let handles: Vec<_> = tasks.into_iter().map(|task| {
            let permit = semaphore.clone();
            let spawn = self.spawn_tool.clone();
            tokio::spawn(async move {
                let _permit = permit.acquire().await?;
                let result = spawn.execute(serde_json::json!({
                    "task": task.task,
                    "preset": task.preset,
                    "context": task.context,
                })).await;
                (task.id, result)
            })
        }).collect();

        let results = futures::future::join_all(handles).await;

        let mut output = serde_json::Map::new();
        for result in results {
            if let Ok((id, Ok(value))) = result {
                output.insert(id, value);
            }
        }

        Ok(serde_json::Value::Object(output))
    }
}
```

### Event Stream for Monitoring

```rust
// Enhanced RunnerEvent for swarm visibility

pub enum RunnerEvent {
    // Existing events...

    /// Swarm started with task list
    SwarmStart {
        swarm_id: String,
        tasks: Vec<String>,
    },

    /// Individual swarm task completed
    SwarmTaskComplete {
        swarm_id: String,
        task_id: String,
        success: bool,
        iterations: u32,
    },

    /// All swarm tasks finished
    SwarmEnd {
        swarm_id: String,
        completed: u32,
        failed: u32,
        total_iterations: u32,
    },
}
```

---

## Implementation Roadmap

### Phase 1: Agent Presets (2-3 weeks)
- [ ] Add `AgentsConfig` and `AgentPreset` to schema
- [ ] Extend `spawn_agent` with `preset` parameter
- [ ] Apply preset tool policies in tool registry
- [ ] Add preset indicator in UI tool cards
- [ ] Write tests for preset loading and application

### Phase 2: Multi-Agent Gateway (4-6 weeks)
- [ ] Add `AgentInstance` struct and management
- [ ] Implement per-agent session isolation
- [ ] Add agent routing to chat service
- [ ] Create `agents.list` and `agents.switch` RPC methods
- [ ] Build agent selector UI component
- [ ] Add agent-scoped broadcasts
- [ ] Write integration tests

### Phase 3: Multi-Instance Control Plane (3-4 weeks)
- [ ] Implement `MultiNodeWebSocketManager`
- [ ] Add node configuration storage (localStorage + sync)
- [ ] Build nodes dashboard page
- [ ] Implement split-view mode
- [ ] Add cross-node session management
- [ ] Write E2E tests

### Phase 4: Swarm Orchestration (2-3 weeks)
- [ ] Implement `SwarmTool`
- [ ] Add swarm events to `RunnerEvent`
- [ ] Create `/swarm` skill
- [ ] Build swarm visualization in UI
- [ ] Add progress tracking and cancellation
- [ ] Write swarm tests

---

## Security Considerations

### Multi-Agent Isolation
- Each agent instance should have isolated file system access
- Sandbox configurations should be per-agent
- API keys should not leak between agents

### Multi-Instance Security
- API key authentication required for remote connections
- TLS mandatory for non-localhost connections
- Rate limiting on cross-instance requests
- Audit logging for multi-instance operations

### Swarm Safety
- Depth limits prevent infinite recursion
- Concurrent task limits prevent resource exhaustion
- Timeout enforcement for stuck workers
- No tool escalation (workers can't grant themselves tools)

---

## References

- [OpenClaw Multi-Agent Routing](https://docs.openclaw.ai/concepts/multi-agent)
- [Claude Code Task Tool](https://code.claude.com/docs/en/sub-agents)
- [CrewAI Framework](https://www.crewai.com/)
- [LangGraph Multi-Agent](https://www.langchain.com/langgraph)
- [Goose Subagents](https://block.github.io/goose/docs/guides/subagents/)
- [OpenAI Agents SDK](https://github.com/openai/openai-agents-python)
