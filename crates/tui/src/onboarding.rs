use std::collections::BTreeSet;

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardingStep {
    Security,
    Llm,
    Voice,
    Channel,
    Identity,
    Summary,
}

impl OnboardingStep {
    pub fn label(self) -> &'static str {
        match self {
            Self::Security => "Security",
            Self::Llm => "LLM",
            Self::Voice => "Voice",
            Self::Channel => "Channel",
            Self::Identity => "Identity",
            Self::Summary => "Summary",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Security => "Secure your instance",
            Self::Llm => "Add LLMs",
            Self::Voice => "Voice (optional)",
            Self::Channel => "Connect Telegram",
            Self::Identity => "Set up your identity",
            Self::Summary => "Setup Summary",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct AuthStatus {
    pub setup_required: bool,
    pub setup_complete: bool,
    pub auth_disabled: bool,
    pub setup_code_required: bool,
    pub localhost_only: bool,
    pub webauthn_available: bool,
}

#[derive(Debug, Clone)]
pub struct SecurityState {
    pub skippable: bool,
    pub setup_required: bool,
    pub setup_complete: bool,
    pub setup_code_required: bool,
    pub localhost_only: bool,
    pub webauthn_available: bool,
    pub setup_code: String,
    pub password: String,
    pub confirm_password: String,
    pub field_index: usize,
}

impl SecurityState {
    fn new(_auth_needed: bool, auth_skippable: bool, status: Option<&AuthStatus>) -> Self {
        let mut state = Self {
            skippable: auth_skippable,
            setup_required: false,
            setup_complete: false,
            setup_code_required: false,
            localhost_only: false,
            webauthn_available: false,
            setup_code: String::new(),
            password: String::new(),
            confirm_password: String::new(),
            field_index: 0,
        };

        if let Some(status) = status {
            state.setup_required = status.setup_required;
            state.setup_complete = status.setup_complete;
            state.setup_code_required = status.setup_code_required;
            state.localhost_only = status.localhost_only;
            state.webauthn_available = status.webauthn_available;
        }

        state
    }

    pub fn visible_fields(&self) -> usize {
        let mut fields = 2usize;
        if self.setup_code_required {
            fields += 1;
        }
        fields
    }
}

#[derive(Debug, Clone)]
pub struct ProviderEntry {
    pub name: String,
    pub display_name: String,
    pub auth_type: String,
    pub configured: bool,
    pub default_base_url: Option<String>,
    pub base_url: Option<String>,
    pub models: Vec<String>,
    pub requires_model: bool,
    pub key_optional: bool,
}

#[derive(Debug, Clone)]
pub struct ModelOption {
    pub id: String,
    pub display_name: String,
    pub supports_tools: bool,
}

#[derive(Debug, Clone)]
pub struct LocalModelOption {
    pub id: String,
    pub display_name: String,
    pub min_ram_gb: u64,
    pub context_window: u64,
    pub suggested: bool,
}

#[derive(Debug, Clone)]
pub enum ProviderConfigurePhase {
    Form,
    ModelSelect {
        models: Vec<ModelOption>,
        selected: BTreeSet<String>,
        cursor: usize,
    },
    OAuth {
        auth_url: Option<String>,
        verification_uri: Option<String>,
        user_code: Option<String>,
    },
    Local {
        backend: String,
        models: Vec<LocalModelOption>,
        cursor: usize,
        note: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct ProviderConfigureState {
    pub provider_name: String,
    pub provider_display_name: String,
    pub auth_type: String,
    pub requires_model: bool,
    pub key_optional: bool,
    pub field_index: usize,
    pub api_key: String,
    pub endpoint: String,
    pub model: String,
    pub phase: ProviderConfigurePhase,
}

impl ProviderConfigureState {
    pub fn visible_fields(&self) -> usize {
        let mut count = 1usize;
        if supports_endpoint(&self.provider_name) {
            count += 1;
        }
        if self.requires_model {
            count += 1;
        }
        count
    }
}

#[derive(Debug, Clone, Default)]
pub struct LlmState {
    pub providers: Vec<ProviderEntry>,
    pub selected_provider: usize,
    pub configuring: Option<ProviderConfigureState>,
}

#[derive(Debug, Clone)]
pub struct VoiceProviderEntry {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub category: String,
    pub available: bool,
    pub enabled: bool,
    pub key_source: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct VoiceState {
    pub available: bool,
    pub providers: Vec<VoiceProviderEntry>,
    pub selected_provider: usize,
    pub pending_api_key: String,
}

#[derive(Debug, Clone)]
pub struct ChannelState {
    pub account_id: String,
    pub token: String,
    pub dm_policy: String,
    pub allowlist: String,
    pub connected: bool,
    pub connected_name: String,
    pub field_index: usize,
}

impl Default for ChannelState {
    fn default() -> Self {
        Self {
            account_id: String::new(),
            token: String::new(),
            dm_policy: "allowlist".into(),
            allowlist: String::new(),
            connected: false,
            connected_name: String::new(),
            field_index: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IdentityState {
    pub user_name: String,
    pub agent_name: String,
    pub emoji: String,
    pub creature: String,
    pub vibe: String,
    pub field_index: usize,
}

impl Default for IdentityState {
    fn default() -> Self {
        Self {
            user_name: String::new(),
            agent_name: "Moltis".into(),
            emoji: "🤖".into(),
            creature: String::new(),
            vibe: String::new(),
            field_index: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChannelSummary {
    pub name: String,
    pub status: String,
}

#[derive(Debug, Clone, Default)]
pub struct SummaryState {
    pub identity_line: Option<String>,
    pub provider_badges: Vec<String>,
    pub channels: Vec<ChannelSummary>,
    pub voice_enabled: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditTarget {
    SecuritySetupCode,
    SecurityPassword,
    SecurityConfirmPassword,
    ProviderApiKey,
    ProviderEndpoint,
    ProviderModel,
    VoiceApiKey,
    ChannelAccountId,
    ChannelToken,
    ChannelAllowlist,
    IdentityUserName,
    IdentityAgentName,
    IdentityEmoji,
    IdentityCreature,
    IdentityVibe,
}

impl EditTarget {
    pub fn placeholder(self) -> &'static str {
        match self {
            Self::SecuritySetupCode => "6-digit setup code from process logs",
            Self::SecurityPassword => "At least 8 characters",
            Self::SecurityConfirmPassword => "Confirm password",
            Self::ProviderApiKey => "Provider API key",
            Self::ProviderEndpoint => "Optional endpoint URL",
            Self::ProviderModel => "Model ID",
            Self::VoiceApiKey => "Voice provider API key",
            Self::ChannelAccountId => "Telegram bot username",
            Self::ChannelToken => "Telegram bot token",
            Self::ChannelAllowlist => "One username per line",
            Self::IdentityUserName => "Your name",
            Self::IdentityAgentName => "Agent name",
            Self::IdentityEmoji => "Emoji",
            Self::IdentityCreature => "Creature",
            Self::IdentityVibe => "Vibe",
        }
    }
}

#[derive(Debug, Clone)]
pub struct OnboardingState {
    pub steps: Vec<OnboardingStep>,
    pub step_index: usize,
    pub busy: bool,
    pub status_message: Option<String>,
    pub error_message: Option<String>,
    pub editing: Option<EditTarget>,
    pub security: SecurityState,
    pub llm: LlmState,
    pub voice: VoiceState,
    pub channel: ChannelState,
    pub identity: IdentityState,
    pub summary: SummaryState,
}

impl OnboardingState {
    pub fn new(
        auth_needed: bool,
        auth_skippable: bool,
        voice_available: bool,
        auth_status: Option<&AuthStatus>,
    ) -> Self {
        let mut steps = Vec::new();
        if auth_needed {
            steps.push(OnboardingStep::Security);
        }
        steps.push(OnboardingStep::Llm);
        if voice_available {
            steps.push(OnboardingStep::Voice);
        }
        steps.push(OnboardingStep::Channel);
        steps.push(OnboardingStep::Identity);
        steps.push(OnboardingStep::Summary);

        Self {
            steps,
            step_index: 0,
            busy: false,
            status_message: None,
            error_message: None,
            editing: None,
            security: SecurityState::new(auth_needed, auth_skippable, auth_status),
            llm: LlmState::default(),
            voice: VoiceState {
                available: voice_available,
                providers: Vec::new(),
                selected_provider: 0,
                pending_api_key: String::new(),
            },
            channel: ChannelState::default(),
            identity: IdentityState::default(),
            summary: SummaryState::default(),
        }
    }

    pub fn current_step(&self) -> OnboardingStep {
        self.steps
            .get(self.step_index)
            .copied()
            .unwrap_or(OnboardingStep::Summary)
    }

    pub fn go_next(&mut self) {
        if self.step_index + 1 < self.steps.len() {
            self.step_index += 1;
        }
    }

    pub fn go_back(&mut self) {
        if self.step_index > 0 {
            self.step_index -= 1;
        }
    }

    pub fn clear_messages(&mut self) {
        self.error_message = None;
        self.status_message = None;
    }

    pub fn set_error(&mut self, message: impl Into<String>) {
        self.status_message = None;
        self.error_message = Some(message.into());
    }

    pub fn set_status(&mut self, message: impl Into<String>) {
        self.error_message = None;
        self.status_message = Some(message.into());
    }

    pub fn begin_edit(&mut self, target: EditTarget) -> String {
        self.editing = Some(target);
        self.current_value_for(target)
    }

    pub fn commit_edit(&mut self, target: EditTarget, value: String) {
        self.editing = None;
        match target {
            EditTarget::SecuritySetupCode => self.security.setup_code = value,
            EditTarget::SecurityPassword => self.security.password = value,
            EditTarget::SecurityConfirmPassword => self.security.confirm_password = value,
            EditTarget::ProviderApiKey => {
                if let Some(config) = self.llm.configuring.as_mut() {
                    config.api_key = value;
                }
            },
            EditTarget::ProviderEndpoint => {
                if let Some(config) = self.llm.configuring.as_mut() {
                    config.endpoint = value;
                }
            },
            EditTarget::ProviderModel => {
                if let Some(config) = self.llm.configuring.as_mut() {
                    config.model = value;
                }
            },
            EditTarget::VoiceApiKey => {
                self.voice.pending_api_key = value;
            },
            EditTarget::ChannelAccountId => self.channel.account_id = value,
            EditTarget::ChannelToken => self.channel.token = value,
            EditTarget::ChannelAllowlist => self.channel.allowlist = value,
            EditTarget::IdentityUserName => self.identity.user_name = value,
            EditTarget::IdentityAgentName => self.identity.agent_name = value,
            EditTarget::IdentityEmoji => self.identity.emoji = value,
            EditTarget::IdentityCreature => self.identity.creature = value,
            EditTarget::IdentityVibe => self.identity.vibe = value,
        }
    }

    pub fn cancel_edit(&mut self) {
        self.editing = None;
    }

    fn current_value_for(&self, target: EditTarget) -> String {
        match target {
            EditTarget::SecuritySetupCode => self.security.setup_code.clone(),
            EditTarget::SecurityPassword => self.security.password.clone(),
            EditTarget::SecurityConfirmPassword => self.security.confirm_password.clone(),
            EditTarget::ProviderApiKey => self
                .llm
                .configuring
                .as_ref()
                .map(|s| s.api_key.clone())
                .unwrap_or_default(),
            EditTarget::ProviderEndpoint => self
                .llm
                .configuring
                .as_ref()
                .map(|s| s.endpoint.clone())
                .unwrap_or_default(),
            EditTarget::ProviderModel => self
                .llm
                .configuring
                .as_ref()
                .map(|s| s.model.clone())
                .unwrap_or_default(),
            EditTarget::VoiceApiKey => self.voice.pending_api_key.clone(),
            EditTarget::ChannelAccountId => self.channel.account_id.clone(),
            EditTarget::ChannelToken => self.channel.token.clone(),
            EditTarget::ChannelAllowlist => self.channel.allowlist.clone(),
            EditTarget::IdentityUserName => self.identity.user_name.clone(),
            EditTarget::IdentityAgentName => self.identity.agent_name.clone(),
            EditTarget::IdentityEmoji => self.identity.emoji.clone(),
            EditTarget::IdentityCreature => self.identity.creature.clone(),
            EditTarget::IdentityVibe => self.identity.vibe.clone(),
        }
    }
}

pub fn supports_endpoint(provider: &str) -> bool {
    matches!(
        provider,
        "openai"
            | "mistral"
            | "openrouter"
            | "cerebras"
            | "minimax"
            | "moonshot"
            | "venice"
            | "ollama"
            | "kimi-code"
            | "xai"
            | "deepseek"
            | "groq"
            | "gemini"
            | "zai"
    )
}

pub fn parse_providers(payload: &Value) -> Vec<ProviderEntry> {
    payload
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let name = row.get("name").and_then(Value::as_str)?.to_string();
                    let display_name = row
                        .get("displayName")
                        .and_then(Value::as_str)
                        .unwrap_or(&name)
                        .to_string();
                    let auth_type = row
                        .get("authType")
                        .and_then(Value::as_str)
                        .unwrap_or("api-key")
                        .to_string();
                    let configured = row
                        .get("configured")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    let default_base_url = row
                        .get("defaultBaseUrl")
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned);
                    let base_url = row
                        .get("baseUrl")
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned);
                    let models = row
                        .get("models")
                        .and_then(Value::as_array)
                        .map(|arr| {
                            arr.iter()
                                .filter_map(Value::as_str)
                                .map(ToOwned::to_owned)
                                .collect::<Vec<String>>()
                        })
                        .unwrap_or_default();
                    let requires_model = row
                        .get("requiresModel")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    let key_optional = row
                        .get("keyOptional")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);

                    Some(ProviderEntry {
                        name,
                        display_name,
                        auth_type,
                        configured,
                        default_base_url,
                        base_url,
                        models,
                        requires_model,
                        key_optional,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn configured_provider_badges(providers: &[ProviderEntry]) -> Vec<String> {
    providers
        .iter()
        .filter(|provider| provider.configured)
        .map(|provider| provider.display_name.clone())
        .collect()
}

pub fn parse_model_options(payload: &Value) -> Vec<ModelOption> {
    payload
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let id = row.get("id").and_then(Value::as_str)?.to_string();
                    let display_name = row
                        .get("displayName")
                        .and_then(Value::as_str)
                        .unwrap_or(&id)
                        .to_string();
                    let supports_tools = row
                        .get("supportsTools")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    Some(ModelOption {
                        id,
                        display_name,
                        supports_tools,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn parse_voice_providers(payload: &Value) -> Vec<VoiceProviderEntry> {
    let parse_side = |side: &str| {
        payload
            .get(side)
            .and_then(Value::as_array)
            .map(|rows| {
                rows.iter()
                    .filter_map(|row| {
                        let id = row.get("id").and_then(Value::as_str)?.to_string();
                        let name = row
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or(&id)
                            .to_string();
                        let provider_type = row
                            .get("type")
                            .and_then(Value::as_str)
                            .unwrap_or(side)
                            .to_string();
                        let category = row
                            .get("category")
                            .and_then(Value::as_str)
                            .unwrap_or("cloud")
                            .to_string();
                        let available = row
                            .get("available")
                            .and_then(Value::as_bool)
                            .unwrap_or(false);
                        let enabled = row.get("enabled").and_then(Value::as_bool).unwrap_or(false);
                        let key_source = row
                            .get("keySource")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned);
                        let description = row
                            .get("description")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned);

                        Some(VoiceProviderEntry {
                            id,
                            name,
                            provider_type,
                            category,
                            available,
                            enabled,
                            key_source,
                            description,
                        })
                    })
                    .collect::<Vec<VoiceProviderEntry>>()
            })
            .unwrap_or_default()
    };

    let mut providers = parse_side("stt");
    providers.extend(parse_side("tts"));
    providers
}

pub fn parse_channels(payload: &Value) -> Vec<ChannelSummary> {
    payload
        .get("channels")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .map(|row| {
                    let status = row
                        .get("status")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                        .to_string();
                    let name = row
                        .get("name")
                        .and_then(Value::as_str)
                        .or_else(|| row.get("account_id").and_then(Value::as_str))
                        .unwrap_or("channel")
                        .to_string();
                    ChannelSummary { name, status }
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn parse_identity(identity: &Value) -> IdentityState {
    IdentityState {
        user_name: identity
            .get("user_name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        agent_name: identity
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Moltis")
            .to_string(),
        emoji: identity
            .get("emoji")
            .and_then(Value::as_str)
            .unwrap_or("🤖")
            .to_string(),
        creature: identity
            .get("creature")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        vibe: identity
            .get("vibe")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        field_index: 0,
    }
}

pub fn parse_local_models(payload: &Value, backend: &str) -> Vec<LocalModelOption> {
    payload
        .get("recommended")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let model_backend =
                        row.get("backend").and_then(Value::as_str).unwrap_or("GGUF");
                    if model_backend != backend {
                        return None;
                    }

                    let id = row.get("id").and_then(Value::as_str)?.to_string();
                    let display_name = row
                        .get("displayName")
                        .and_then(Value::as_str)
                        .unwrap_or(&id)
                        .to_string();
                    let min_ram_gb = row.get("minRamGb").and_then(Value::as_u64).unwrap_or(0);
                    let context_window = row
                        .get("contextWindow")
                        .and_then(Value::as_u64)
                        .unwrap_or(0);
                    let suggested = row
                        .get("suggested")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);

                    Some(LocalModelOption {
                        id,
                        display_name,
                        min_ram_gb,
                        context_window,
                        suggested,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn parse_local_recommended_backend(payload: &Value) -> String {
    payload
        .get("recommendedBackend")
        .and_then(Value::as_str)
        .unwrap_or("GGUF")
        .to_string()
}

pub fn parse_local_backend_note(payload: &Value) -> Option<String> {
    payload
        .get("backendNote")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_follow_web_order() {
        let s = OnboardingState::new(false, false, true, None);
        assert_eq!(s.steps, vec![
            OnboardingStep::Llm,
            OnboardingStep::Voice,
            OnboardingStep::Channel,
            OnboardingStep::Identity,
            OnboardingStep::Summary
        ]);

        let s2 = OnboardingState::new(true, true, false, None);
        assert_eq!(s2.steps, vec![
            OnboardingStep::Security,
            OnboardingStep::Llm,
            OnboardingStep::Channel,
            OnboardingStep::Identity,
            OnboardingStep::Summary
        ]);
    }

    #[test]
    fn parse_provider_rows() {
        let payload = serde_json::json!([
            {
                "name": "openai",
                "displayName": "OpenAI",
                "authType": "api-key",
                "configured": true,
                "defaultBaseUrl": "https://api.openai.com/v1",
                "baseUrl": "https://api.openai.com/v1",
                "models": ["gpt-5"],
                "requiresModel": false,
                "keyOptional": false
            }
        ]);
        let providers = parse_providers(&payload);
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].name, "openai");
        assert!(providers[0].configured);
        assert_eq!(providers[0].models, vec!["gpt-5"]);
    }

    #[test]
    fn parse_voice_rows() {
        let payload = serde_json::json!({
            "tts": [
                {
                    "id": "openai-tts",
                    "name": "OpenAI TTS",
                    "type": "tts",
                    "category": "cloud",
                    "available": true,
                    "enabled": false,
                    "keySource": "env"
                }
            ],
            "stt": []
        });

        let providers = parse_voice_providers(&payload);
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].id, "openai-tts");
        assert_eq!(providers[0].provider_type, "tts");
    }
}
