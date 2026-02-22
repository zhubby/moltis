use {
    super::{App, InitialData},
    crate::{
        onboarding::{
            AuthStatus, ChannelProvider, EditTarget, OnboardingState, OnboardingStep,
            ProviderConfigurePhase, ProviderConfigureState, ProviderEntry, SecurityState,
            configured_provider_badges, parse_channels, parse_identity, parse_local_backend_note,
            parse_local_models, parse_local_recommended_backend, parse_model_options,
            parse_providers, parse_voice_providers, supports_endpoint,
        },
        rpc::RpcClient,
        state::{DisplayMessage, InputMode, MessageRole, SessionEntry},
    },
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    serde_json::Value,
    std::{collections::BTreeSet, sync::Arc},
    tui_textarea::TextArea,
    url::{Host, Url},
};

impl App {
    pub(super) async fn initialize_onboarding(&mut self, rpc: &Arc<RpcClient>) -> bool {
        let onboarded = rpc
            .call("wizard.status", serde_json::json!({}))
            .await
            .ok()
            .and_then(|status| status.get("onboarded").and_then(|v| v.as_bool()))
            .unwrap_or(false);

        if onboarded {
            self.onboarding = None;
            return true;
        }

        let auth_status = self.fetch_auth_status().await.ok();
        let auth_needed = auth_status.as_ref().is_some_and(|status| {
            status.setup_required || (status.auth_disabled && !status.localhost_only)
        });
        let auth_skippable = auth_status
            .as_ref()
            .is_some_and(|status| !status.setup_required);

        let voice_payload = rpc
            .call("voice.providers.all", serde_json::json!({}))
            .await
            .ok();
        let voice_available = voice_payload.is_some();

        let mut onboarding = OnboardingState::new(
            auth_needed,
            auth_skippable,
            voice_available,
            auth_status.as_ref(),
        );

        if let Ok(identity) = rpc.call("agent.identity.get", serde_json::json!({})).await {
            onboarding.identity = parse_identity(&identity);
        }

        if let Ok(providers) = rpc.call("providers.available", serde_json::json!({})).await {
            onboarding.llm.providers = parse_providers(&providers);
        }

        if let Some(voice) = voice_payload.as_ref() {
            onboarding.voice.providers = parse_voice_providers(voice);
            onboarding.voice.available = true;
        }

        self.onboarding = Some(onboarding);
        self.state.sidebar_visible = false;
        self.state.input_mode = InputMode::Normal;
        self.state.dirty = true;
        false
    }

    async fn fetch_auth_status(&self) -> Result<AuthStatus, String> {
        let base = http_base_url_from_ws(&self.url)
            .ok_or_else(|| "unable to derive HTTP base URL from gateway URL".to_string())?;
        let endpoint = format!("{base}/api/auth/status");

        let response = reqwest::Client::new()
            .get(&endpoint)
            .send()
            .await
            .map_err(|error| format!("failed to fetch auth status: {error}"))?;

        if !response.status().is_success() {
            return Err(format!(
                "auth status endpoint returned {}",
                response.status()
            ));
        }

        let payload = response
            .json::<Value>()
            .await
            .map_err(|error| format!("invalid auth status response: {error}"))?;

        Ok(AuthStatus {
            setup_required: payload
                .get("setup_required")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
            setup_complete: payload
                .get("setup_complete")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
            auth_disabled: payload
                .get("auth_disabled")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
            setup_code_required: payload
                .get("setup_code_required")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
            localhost_only: payload
                .get("localhost_only")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
            webauthn_available: payload
                .get("webauthn_available")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
        })
    }

    pub(super) async fn handle_onboarding_normal_key(
        &mut self,
        key: KeyEvent,
        rpc: &Arc<RpcClient>,
        textarea: &mut TextArea<'_>,
    ) {
        if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
            self.should_quit = true;
            return;
        }

        let Some(step) = self.onboarding.as_ref().map(OnboardingState::current_step) else {
            return;
        };

        match step {
            OnboardingStep::Security => {
                self.handle_security_step_key(key, rpc, textarea).await;
            },
            OnboardingStep::Llm => {
                self.handle_llm_step_key(key, rpc, textarea).await;
            },
            OnboardingStep::Voice => {
                self.handle_voice_step_key(key, rpc, textarea).await;
            },
            OnboardingStep::Channel => {
                self.handle_channel_step_key(key, rpc, textarea).await;
            },
            OnboardingStep::Identity => {
                self.handle_identity_step_key(key, rpc, textarea).await;
            },
            OnboardingStep::Summary => {
                self.handle_summary_step_key(key, rpc).await;
            },
        }

        // Auto-refresh summary when navigating into the Summary step.
        if step != OnboardingStep::Summary
            && self
                .onboarding
                .as_ref()
                .is_some_and(|o| o.current_step() == OnboardingStep::Summary)
        {
            self.refresh_summary(rpc).await;
        }
    }

    async fn handle_security_step_key(
        &mut self,
        key: KeyEvent,
        _rpc: &Arc<RpcClient>,
        textarea: &mut TextArea<'_>,
    ) {
        if self
            .onboarding
            .as_ref()
            .is_none_or(|onboarding| onboarding.busy)
        {
            return;
        }

        if key.code == KeyCode::Char('c') {
            self.submit_security_step().await;
            return;
        }

        let Some(onboarding) = self.onboarding.as_mut() else {
            return;
        };

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                let max_index = onboarding.security.visible_fields().saturating_sub(1);
                onboarding.security.field_index =
                    (onboarding.security.field_index + 1).min(max_index);
                self.state.dirty = true;
            },
            KeyCode::Char('k') | KeyCode::Up => {
                onboarding.security.field_index = onboarding.security.field_index.saturating_sub(1);
                self.state.dirty = true;
            },
            KeyCode::Char('e') | KeyCode::Enter => {
                let target = security_edit_target(&onboarding.security);
                self.start_onboarding_edit(target, textarea);
            },
            KeyCode::Char('s') => {
                if onboarding.security.skippable || onboarding.security.localhost_only {
                    onboarding.clear_messages();
                    onboarding.go_next();
                    self.state.dirty = true;
                }
            },
            KeyCode::Char('b') => {
                onboarding.clear_messages();
                onboarding.go_back();
                self.state.dirty = true;
            },
            _ => {},
        }
    }

    async fn submit_security_step(&mut self) {
        let (
            setup_complete,
            setup_code_required,
            setup_code,
            password,
            confirm_password,
            localhost_only,
            skippable,
        ) = {
            let Some(onboarding) = self.onboarding.as_ref() else {
                return;
            };
            (
                onboarding.security.setup_complete,
                onboarding.security.setup_code_required,
                onboarding.security.setup_code.clone(),
                onboarding.security.password.clone(),
                onboarding.security.confirm_password.clone(),
                onboarding.security.localhost_only,
                onboarding.security.skippable,
            )
        };

        if setup_complete {
            if let Some(onboarding) = self.onboarding.as_mut() {
                onboarding.clear_messages();
                onboarding.go_next();
                self.state.dirty = true;
            }
            return;
        }

        if password.len() < 8 && !(localhost_only && password.is_empty() && skippable) {
            if let Some(onboarding) = self.onboarding.as_mut() {
                onboarding.set_error("Password must be at least 8 characters.");
                self.state.dirty = true;
            }
            return;
        }
        if !password.is_empty() && password != confirm_password {
            if let Some(onboarding) = self.onboarding.as_mut() {
                onboarding.set_error("Passwords do not match.");
                self.state.dirty = true;
            }
            return;
        }
        if setup_code_required && setup_code.trim().is_empty() {
            if let Some(onboarding) = self.onboarding.as_mut() {
                onboarding.set_error("Setup code is required.");
                self.state.dirty = true;
            }
            return;
        }

        if let Some(onboarding) = self.onboarding.as_mut() {
            onboarding.busy = true;
            onboarding.clear_messages();
        }
        self.state.dirty = true;

        let result = self
            .perform_auth_setup(password, setup_code_required.then_some(setup_code))
            .await;

        if let Some(onboarding) = self.onboarding.as_mut() {
            onboarding.busy = false;
            match result {
                Ok(message) => {
                    onboarding.set_status(message);
                    onboarding.go_next();
                },
                Err(error) => onboarding.set_error(error),
            }
            self.state.dirty = true;
        }
    }

    async fn perform_auth_setup(
        &mut self,
        password: String,
        setup_code: Option<String>,
    ) -> Result<String, String> {
        let base = http_base_url_from_ws(&self.url)
            .ok_or_else(|| "unable to derive HTTP URL for auth setup".to_string())?;
        let endpoint = format!("{base}/api/auth/setup");

        let mut body = serde_json::Map::new();
        if !password.is_empty() {
            body.insert("password".into(), serde_json::json!(password));
        }
        if let Some(code) = setup_code {
            body.insert("setup_code".into(), serde_json::json!(code));
        }

        let response = reqwest::Client::new()
            .post(&endpoint)
            .json(&Value::Object(body))
            .send()
            .await
            .map_err(|error| format!("failed to call auth setup endpoint: {error}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let detail = response.text().await.unwrap_or_else(|_| String::new());
            if detail.trim().is_empty() {
                return Err(format!("setup failed ({status})"));
            }
            return Err(format!("setup failed ({status}): {detail}"));
        }

        if !password.is_empty() {
            self.auth.password = Some(password);
        }

        Ok("Security setup completed.".into())
    }

    async fn handle_llm_step_key(
        &mut self,
        key: KeyEvent,
        rpc: &Arc<RpcClient>,
        textarea: &mut TextArea<'_>,
    ) {
        if self
            .onboarding
            .as_ref()
            .is_some_and(|onboarding| onboarding.llm.configuring.is_some())
        {
            self.handle_llm_config_key(key, rpc, textarea).await;
            return;
        }

        if key.code == KeyCode::Char('r') {
            self.refresh_onboarding_providers(rpc).await;
            return;
        }

        if matches!(key.code, KeyCode::Char('e') | KeyCode::Enter) {
            let provider = self
                .onboarding
                .as_ref()
                .and_then(|onboarding| {
                    onboarding
                        .llm
                        .providers
                        .get(onboarding.llm.selected_provider)
                })
                .cloned();
            if let Some(provider) = provider {
                self.open_llm_provider_config(provider, rpc).await;
            }
            return;
        }

        let Some(onboarding) = self.onboarding.as_mut() else {
            return;
        };

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if !onboarding.llm.providers.is_empty() {
                    let next = onboarding.llm.selected_provider.saturating_add(1);
                    onboarding.llm.selected_provider = next.min(onboarding.llm.providers.len() - 1);
                    self.state.dirty = true;
                }
            },
            KeyCode::Char('k') | KeyCode::Up => {
                onboarding.llm.selected_provider =
                    onboarding.llm.selected_provider.saturating_sub(1);
                self.state.dirty = true;
            },
            KeyCode::Char('c') => {
                onboarding.clear_messages();
                onboarding.go_next();
                self.state.dirty = true;
            },
            KeyCode::Char('s') => {
                onboarding.clear_messages();
                onboarding.go_next();
                self.state.dirty = true;
            },
            KeyCode::Char('b') => {
                onboarding.clear_messages();
                onboarding.go_back();
                self.state.dirty = true;
            },
            _ => {},
        }
    }

    async fn handle_llm_config_key(
        &mut self,
        key: KeyEvent,
        rpc: &Arc<RpcClient>,
        textarea: &mut TextArea<'_>,
    ) {
        let phase = self
            .onboarding
            .as_ref()
            .and_then(|onboarding| onboarding.llm.configuring.as_ref())
            .map(|config| config.phase.clone());
        let Some(phase) = phase else {
            return;
        };

        match phase {
            ProviderConfigurePhase::Form => {
                if key.code == KeyCode::Char('v') {
                    self.save_llm_provider_config(rpc).await;
                    return;
                }
                if key.code == KeyCode::Char('m') {
                    self.open_llm_provider_model_select(rpc).await;
                    return;
                }

                let Some(onboarding) = self.onboarding.as_mut() else {
                    return;
                };
                let Some(config) = onboarding.llm.configuring.as_mut() else {
                    return;
                };
                match key.code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        let max_index = config.visible_fields().saturating_sub(1);
                        config.field_index = (config.field_index + 1).min(max_index);
                        self.state.dirty = true;
                    },
                    KeyCode::Char('k') | KeyCode::Up => {
                        config.field_index = config.field_index.saturating_sub(1);
                        self.state.dirty = true;
                    },
                    KeyCode::Char('e') | KeyCode::Enter => {
                        if let Some(target) = provider_edit_target(config) {
                            self.start_onboarding_edit(target, textarea);
                        }
                    },
                    KeyCode::Esc => {
                        onboarding.llm.configuring = None;
                        onboarding.clear_messages();
                        self.state.dirty = true;
                    },
                    _ => {},
                }
            },
            ProviderConfigurePhase::ModelSelect { .. } => {
                let Some(onboarding) = self.onboarding.as_mut() else {
                    return;
                };
                let Some(config) = onboarding.llm.configuring.as_mut() else {
                    return;
                };
                let ProviderConfigurePhase::ModelSelect {
                    models,
                    selected,
                    cursor,
                } = &mut config.phase
                else {
                    return;
                };

                if key.code == KeyCode::Enter {
                    self.save_llm_selected_models(rpc).await;
                    return;
                }

                match key.code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        if !models.is_empty() {
                            *cursor = cursor.saturating_add(1).min(models.len() - 1);
                            self.state.dirty = true;
                        }
                    },
                    KeyCode::Char('k') | KeyCode::Up => {
                        *cursor = cursor.saturating_sub(1);
                        self.state.dirty = true;
                    },
                    KeyCode::Char(' ') => {
                        if let Some(model) = models.get(*cursor) {
                            if !selected.insert(model.id.clone()) {
                                selected.remove(&model.id);
                            }
                            self.state.dirty = true;
                        }
                    },
                    KeyCode::Esc => {
                        onboarding.llm.configuring = None;
                        onboarding.clear_messages();
                        self.state.dirty = true;
                    },
                    _ => {},
                }
            },
            ProviderConfigurePhase::OAuth { .. } => {
                if matches!(key.code, KeyCode::Char('p') | KeyCode::Enter) {
                    self.poll_oauth_provider_status(rpc).await;
                    return;
                }

                if key.code == KeyCode::Esc
                    && let Some(onboarding) = self.onboarding.as_mut()
                {
                    onboarding.llm.configuring = None;
                    onboarding.clear_messages();
                    self.state.dirty = true;
                }
            },
            ProviderConfigurePhase::Local { .. } => {
                if key.code == KeyCode::Enter {
                    self.configure_local_provider_model(rpc).await;
                    return;
                }

                let Some(onboarding) = self.onboarding.as_mut() else {
                    return;
                };
                let Some(config) = onboarding.llm.configuring.as_mut() else {
                    return;
                };
                let ProviderConfigurePhase::Local { models, cursor, .. } = &mut config.phase else {
                    return;
                };

                match key.code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        if !models.is_empty() {
                            *cursor = cursor.saturating_add(1).min(models.len() - 1);
                            self.state.dirty = true;
                        }
                    },
                    KeyCode::Char('k') | KeyCode::Up => {
                        *cursor = cursor.saturating_sub(1);
                        self.state.dirty = true;
                    },
                    KeyCode::Esc => {
                        onboarding.llm.configuring = None;
                        onboarding.clear_messages();
                        self.state.dirty = true;
                    },
                    _ => {},
                }
            },
        }
    }

    async fn open_llm_provider_config(&mut self, provider: ProviderEntry, rpc: &Arc<RpcClient>) {
        if let Some(onboarding) = self.onboarding.as_mut() {
            onboarding.clear_messages();
            onboarding.busy = true;
            self.state.dirty = true;
        }

        let result = match provider.auth_type.as_str() {
            "api-key" if provider.configured => {
                self.start_api_key_model_select(provider, rpc).await
            },
            "api-key" => {
                let endpoint = provider
                    .base_url
                    .clone()
                    .or(provider.default_base_url.clone())
                    .unwrap_or_default();
                let model = provider.models.first().cloned().unwrap_or_default();
                Ok(ProviderConfigureState {
                    provider_name: provider.name,
                    provider_display_name: provider.display_name,
                    auth_type: provider.auth_type,
                    requires_model: provider.requires_model,
                    key_optional: provider.key_optional,
                    field_index: 0,
                    api_key: String::new(),
                    endpoint,
                    model,
                    phase: ProviderConfigurePhase::Form,
                })
            },
            "oauth" => self.start_oauth_config(provider, rpc).await,
            "local" => self.start_local_config(provider, rpc).await,
            other => Err(format!("Unsupported provider auth type: {other}")),
        };

        if let Some(onboarding) = self.onboarding.as_mut() {
            onboarding.busy = false;
            match result {
                Ok(configure) => {
                    onboarding.llm.configuring = Some(configure);
                    onboarding.clear_messages();
                },
                Err(error) => onboarding.set_error(error),
            }
            self.state.dirty = true;
        }
    }

    async fn start_api_key_model_select(
        &self,
        provider: ProviderEntry,
        rpc: &Arc<RpcClient>,
    ) -> Result<ProviderConfigureState, String> {
        let endpoint = provider
            .base_url
            .clone()
            .or(provider.default_base_url.clone())
            .unwrap_or_default();
        let current_model = provider.models.first().cloned().unwrap_or_default();

        let models_rpc = rpc.call("models.list", serde_json::json!({})).await;
        let mut models = match models_rpc {
            Ok(payload) => parse_provider_models_from_list(&payload, &provider.name),
            Err(error) => {
                if provider.models.is_empty() {
                    return Err(error.to_string());
                }
                provider
                    .models
                    .iter()
                    .map(|id| crate::onboarding::ModelOption {
                        id: id.clone(),
                        display_name: id.clone(),
                        supports_tools: false,
                    })
                    .collect()
            },
        };
        if models.is_empty() && !provider.models.is_empty() {
            models = provider
                .models
                .iter()
                .map(|id| crate::onboarding::ModelOption {
                    id: id.clone(),
                    display_name: id.clone(),
                    supports_tools: false,
                })
                .collect();
        }
        if models.is_empty() {
            return Err(format!(
                "No models available for {}. Try refreshing providers first.",
                provider.display_name
            ));
        }

        let (selected, cursor) =
            select_models_for_picker(&models, &provider.models, &current_model);

        Ok(ProviderConfigureState {
            provider_name: provider.name,
            provider_display_name: provider.display_name,
            auth_type: provider.auth_type,
            requires_model: provider.requires_model,
            key_optional: provider.key_optional,
            field_index: 0,
            api_key: String::new(),
            endpoint,
            model: current_model,
            phase: ProviderConfigurePhase::ModelSelect {
                models,
                selected,
                cursor,
            },
        })
    }

    async fn start_oauth_config(
        &self,
        provider: ProviderEntry,
        rpc: &Arc<RpcClient>,
    ) -> Result<ProviderConfigureState, String> {
        let mut params = serde_json::json!({
            "provider": provider.name,
        });
        if let Some(base) = http_base_url_from_ws(&self.url) {
            params["redirectUri"] = serde_json::json!(format!("{base}/auth/callback"));
        }

        let payload = rpc
            .call("providers.oauth.start", params)
            .await
            .map_err(|error| error.to_string())?;

        if payload
            .get("alreadyAuthenticated")
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
        {
            return Ok(ProviderConfigureState {
                provider_name: provider.name,
                provider_display_name: provider.display_name,
                auth_type: provider.auth_type,
                requires_model: false,
                key_optional: false,
                field_index: 0,
                api_key: String::new(),
                endpoint: String::new(),
                model: String::new(),
                phase: ProviderConfigurePhase::OAuth {
                    auth_url: None,
                    verification_uri: None,
                    user_code: None,
                },
            });
        }

        let auth_url = payload
            .get("authUrl")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned);
        let verification_uri = payload
            .get("verificationUriComplete")
            .or_else(|| payload.get("verificationUri"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned);
        let user_code = payload
            .get("userCode")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned);

        if auth_url.is_none() && verification_uri.is_none() && user_code.is_none() {
            return Err("OAuth start did not return authentication instructions.".into());
        }

        Ok(ProviderConfigureState {
            provider_name: provider.name,
            provider_display_name: provider.display_name,
            auth_type: provider.auth_type,
            requires_model: false,
            key_optional: false,
            field_index: 0,
            api_key: String::new(),
            endpoint: String::new(),
            model: String::new(),
            phase: ProviderConfigurePhase::OAuth {
                auth_url,
                verification_uri,
                user_code,
            },
        })
    }

    async fn start_local_config(
        &self,
        provider: ProviderEntry,
        rpc: &Arc<RpcClient>,
    ) -> Result<ProviderConfigureState, String> {
        let system = rpc
            .call("providers.local.system_info", serde_json::json!({}))
            .await
            .map_err(|error| format!("failed to fetch local system info: {error}"))?;
        let backend = parse_local_recommended_backend(&system);
        let note = parse_local_backend_note(&system);

        let models = rpc
            .call("providers.local.models", serde_json::json!({}))
            .await
            .map_err(|error| format!("failed to fetch local model list: {error}"))?;
        let parsed_models = parse_local_models(&models, &backend);

        if parsed_models.is_empty() {
            return Err(format!("No local models available for backend {backend}."));
        }

        Ok(ProviderConfigureState {
            provider_name: provider.name,
            provider_display_name: provider.display_name,
            auth_type: provider.auth_type,
            requires_model: false,
            key_optional: false,
            field_index: 0,
            api_key: String::new(),
            endpoint: String::new(),
            model: String::new(),
            phase: ProviderConfigurePhase::Local {
                backend,
                models: parsed_models,
                cursor: 0,
                note,
            },
        })
    }

    async fn refresh_onboarding_providers(&mut self, rpc: &Arc<RpcClient>) {
        let result = rpc.call("providers.available", serde_json::json!({})).await;
        if let Some(onboarding) = self.onboarding.as_mut() {
            match result {
                Ok(payload) => {
                    onboarding.llm.providers = parse_providers(&payload);
                    if onboarding.llm.providers.is_empty() {
                        onboarding.llm.selected_provider = 0;
                    } else if onboarding.llm.selected_provider >= onboarding.llm.providers.len() {
                        onboarding.llm.selected_provider = onboarding.llm.providers.len() - 1;
                    }
                    onboarding.set_status("Providers refreshed.");
                },
                Err(error) => onboarding.set_error(format!("Failed to load providers: {error}")),
            }
            self.state.dirty = true;
        }
    }

    async fn save_llm_provider_config(&mut self, rpc: &Arc<RpcClient>) {
        let Some(config) = self
            .onboarding
            .as_ref()
            .and_then(|onboarding| onboarding.llm.configuring.as_ref())
            .cloned()
        else {
            return;
        };

        if config.auth_type != "api-key" {
            return;
        }

        let provider_name = config.provider_name.clone();

        let api_key_value = if config.api_key.trim().is_empty() {
            config.provider_name.clone()
        } else {
            config.api_key.trim().to_string()
        };

        if !config.key_optional && config.api_key.trim().is_empty() {
            if let Some(onboarding) = self.onboarding.as_mut() {
                onboarding.set_error("API key is required.");
                self.state.dirty = true;
            }
            return;
        }

        if let Some(onboarding) = self.onboarding.as_mut() {
            onboarding.busy = true;
            onboarding.clear_messages();
            self.state.dirty = true;
        }

        let mut validate_payload = serde_json::json!({
            "provider": config.provider_name,
            "apiKey": api_key_value,
        });
        if !config.endpoint.trim().is_empty() {
            validate_payload["baseUrl"] = serde_json::json!(config.endpoint.trim());
        }
        if !config.model.trim().is_empty() {
            validate_payload["model"] = serde_json::json!(config.model.trim());
        }

        let result = async {
            let validation = rpc
                .call("providers.validate_key", validate_payload.clone())
                .await
                .map_err(|error| error.to_string())?;

            let valid = validation
                .get("valid")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            if !valid {
                let error = validation
                    .get("error")
                    .and_then(|value| value.as_str())
                    .unwrap_or("Validation failed");
                return Err(error.to_string());
            }

            let mut save_payload = serde_json::json!({
                "provider": config.provider_name,
                "apiKey": api_key_value,
            });
            if !config.endpoint.trim().is_empty() {
                save_payload["baseUrl"] = serde_json::json!(config.endpoint.trim());
            }
            if !config.model.trim().is_empty() {
                save_payload["model"] = serde_json::json!(config.model.trim());
            }

            rpc.call("providers.save_key", save_payload)
                .await
                .map_err(|error| error.to_string())?;

            let models = parse_model_options(validation.get("models").unwrap_or(&Value::Null));
            Ok(models)
        }
        .await;

        let mut refresh_providers = false;
        let mut model_to_save = None::<String>;

        if let Some(onboarding) = self.onboarding.as_mut() {
            onboarding.busy = false;
            match result {
                Ok(models) => {
                    if !models.is_empty() {
                        let (selected, cursor) =
                            select_models_for_picker(&models, &[], &config.model);
                        onboarding.llm.configuring = Some(ProviderConfigureState {
                            phase: ProviderConfigurePhase::ModelSelect {
                                models,
                                selected,
                                cursor,
                            },
                            ..config
                        });
                        onboarding.set_status("Choose preferred models.");
                    } else {
                        if config.requires_model && !config.model.trim().is_empty() {
                            model_to_save = Some(config.model.trim().to_string());
                        }
                        onboarding.llm.configuring = None;
                        onboarding.set_status("Provider saved.");
                        refresh_providers = true;
                    }
                },
                Err(error) => onboarding.set_error(error),
            }
            self.state.dirty = true;
        }

        if let Some(model_id) = model_to_save {
            let _ = rpc
                .call(
                    "providers.save_models",
                    serde_json::json!({
                        "provider": provider_name,
                        "models": [model_id],
                    }),
                )
                .await;
        }

        if refresh_providers {
            self.refresh_onboarding_providers(rpc).await;
        }
    }

    async fn save_llm_selected_models(&mut self, rpc: &Arc<RpcClient>) {
        let Some((provider_name, selected_models)) = self
            .onboarding
            .as_ref()
            .and_then(|onboarding| onboarding.llm.configuring.as_ref())
            .and_then(|config| {
                if let ProviderConfigurePhase::ModelSelect { selected, .. } = &config.phase {
                    Some((config.provider_name.clone(), selected.clone()))
                } else {
                    None
                }
            })
        else {
            return;
        };

        let models = selected_models.into_iter().collect::<Vec<String>>();

        let result = rpc
            .call(
                "providers.save_models",
                serde_json::json!({
                    "provider": provider_name,
                    "models": models,
                }),
            )
            .await;

        let mut refresh_providers = false;
        if let Some(onboarding) = self.onboarding.as_mut() {
            match result {
                Ok(_) => {
                    onboarding.llm.configuring = None;
                    onboarding.set_status("Model preferences saved.");
                    refresh_providers = true;
                },
                Err(error) => onboarding.set_error(format!("Failed to save models: {error}")),
            }
            self.state.dirty = true;
        }

        if refresh_providers {
            self.refresh_onboarding_providers(rpc).await;
        }
    }

    async fn open_llm_provider_model_select(&mut self, rpc: &Arc<RpcClient>) {
        let Some((provider_name, provider_display_name, current_model, saved_models)) = self
            .onboarding
            .as_ref()
            .and_then(|onboarding| onboarding.llm.configuring.as_ref())
            .and_then(|config| {
                if config.auth_type != "api-key"
                    || !matches!(config.phase, ProviderConfigurePhase::Form)
                {
                    return None;
                }

                let saved_models = self
                    .onboarding
                    .as_ref()
                    .and_then(|onboarding| {
                        onboarding
                            .llm
                            .providers
                            .iter()
                            .find(|provider| provider.name == config.provider_name)
                    })
                    .map(|provider| provider.models.clone())
                    .unwrap_or_default();

                Some((
                    config.provider_name.clone(),
                    config.provider_display_name.clone(),
                    config.model.clone(),
                    saved_models,
                ))
            })
        else {
            return;
        };

        if let Some(onboarding) = self.onboarding.as_mut() {
            onboarding.busy = true;
            onboarding.clear_messages();
            self.state.dirty = true;
        }

        let result = rpc
            .call("models.list", serde_json::json!({}))
            .await
            .map(|payload| parse_provider_models_from_list(&payload, &provider_name))
            .map_err(|error| error.to_string());

        if let Some(onboarding) = self.onboarding.as_mut() {
            onboarding.busy = false;
            match result {
                Ok(models) => {
                    if models.is_empty() {
                        onboarding.set_error(
                            "No models available yet. Validate/save first with v, then choose models.",
                        );
                        self.state.dirty = true;
                        return;
                    }

                    let Some(config) = onboarding.llm.configuring.as_mut() else {
                        self.state.dirty = true;
                        return;
                    };
                    if config.provider_name != provider_name
                        || !matches!(config.phase, ProviderConfigurePhase::Form)
                    {
                        self.state.dirty = true;
                        return;
                    }

                    let (selected, cursor) =
                        select_models_for_picker(&models, &saved_models, &current_model);

                    config.phase = ProviderConfigurePhase::ModelSelect {
                        models,
                        selected,
                        cursor,
                    };
                    onboarding.set_status(format!(
                        "Select preferred models for {}.",
                        provider_display_name
                    ));
                },
                Err(error) => onboarding.set_error(format!("Failed to load models: {error}")),
            }
            self.state.dirty = true;
        }
    }

    async fn poll_oauth_provider_status(&mut self, rpc: &Arc<RpcClient>) {
        let provider_name = self
            .onboarding
            .as_ref()
            .and_then(|onboarding| onboarding.llm.configuring.as_ref())
            .map(|config| config.provider_name.clone());
        let Some(provider_name) = provider_name else {
            return;
        };

        let result = rpc
            .call(
                "providers.oauth.status",
                serde_json::json!({ "provider": provider_name }),
            )
            .await;

        let mut refresh_providers = false;
        if let Some(onboarding) = self.onboarding.as_mut() {
            match result {
                Ok(payload) => {
                    if payload
                        .get("authenticated")
                        .and_then(|value| value.as_bool())
                        .unwrap_or(false)
                    {
                        onboarding.llm.configuring = None;
                        onboarding.set_status("OAuth provider authenticated.");
                        refresh_providers = true;
                    } else {
                        onboarding.set_status("OAuth still pending.");
                    }
                },
                Err(error) => onboarding.set_error(format!("OAuth status failed: {error}")),
            }
            self.state.dirty = true;
        }

        if refresh_providers {
            self.refresh_onboarding_providers(rpc).await;
        }
    }

    async fn configure_local_provider_model(&mut self, rpc: &Arc<RpcClient>) {
        let Some((model_id, backend)) = self
            .onboarding
            .as_ref()
            .and_then(|onboarding| onboarding.llm.configuring.as_ref())
            .and_then(|config| {
                if let ProviderConfigurePhase::Local {
                    backend,
                    models,
                    cursor,
                    ..
                } = &config.phase
                {
                    models
                        .get(*cursor)
                        .map(|model| (model.id.clone(), backend.clone()))
                } else {
                    None
                }
            })
        else {
            return;
        };

        let result = rpc
            .call(
                "providers.local.configure",
                serde_json::json!({
                    "modelId": model_id,
                    "backend": backend,
                }),
            )
            .await;

        let mut refresh_providers = false;
        if let Some(onboarding) = self.onboarding.as_mut() {
            match result {
                Ok(_) => {
                    onboarding.llm.configuring = None;
                    onboarding.set_status("Local provider configured.");
                    refresh_providers = true;
                },
                Err(error) => onboarding.set_error(format!("Local model setup failed: {error}")),
            }
            self.state.dirty = true;
        }

        if refresh_providers {
            self.refresh_onboarding_providers(rpc).await;
        }
    }

    async fn handle_voice_step_key(
        &mut self,
        key: KeyEvent,
        rpc: &Arc<RpcClient>,
        textarea: &mut TextArea<'_>,
    ) {
        match key.code {
            KeyCode::Char('t') => {
                self.toggle_selected_voice_provider(rpc).await;
                return;
            },
            KeyCode::Char('v') => {
                self.save_selected_voice_key(rpc).await;
                return;
            },
            KeyCode::Char('r') => {
                self.refresh_voice_providers(rpc).await;
                return;
            },
            _ => {},
        }

        let Some(onboarding) = self.onboarding.as_mut() else {
            return;
        };

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if !onboarding.voice.providers.is_empty() {
                    let next = onboarding.voice.selected_provider.saturating_add(1);
                    onboarding.voice.selected_provider =
                        next.min(onboarding.voice.providers.len() - 1);
                    self.state.dirty = true;
                }
            },
            KeyCode::Char('k') | KeyCode::Up => {
                onboarding.voice.selected_provider =
                    onboarding.voice.selected_provider.saturating_sub(1);
                self.state.dirty = true;
            },
            KeyCode::Char('e') => {
                self.start_onboarding_edit(EditTarget::VoiceApiKey, textarea);
            },
            KeyCode::Char('c') => {
                onboarding.go_next();
                onboarding.clear_messages();
                self.state.dirty = true;
            },
            KeyCode::Char('s') => {
                onboarding.go_next();
                onboarding.clear_messages();
                self.state.dirty = true;
            },
            KeyCode::Char('b') => {
                onboarding.go_back();
                onboarding.clear_messages();
                self.state.dirty = true;
            },
            _ => {},
        }
    }

    async fn refresh_voice_providers(&mut self, rpc: &Arc<RpcClient>) {
        let result = rpc.call("voice.providers.all", serde_json::json!({})).await;
        if let Some(onboarding) = self.onboarding.as_mut() {
            match result {
                Ok(payload) => {
                    onboarding.voice.providers = parse_voice_providers(&payload);
                    if onboarding.voice.providers.is_empty() {
                        onboarding.voice.selected_provider = 0;
                    } else if onboarding.voice.selected_provider >= onboarding.voice.providers.len()
                    {
                        onboarding.voice.selected_provider =
                            onboarding.voice.providers.len().saturating_sub(1);
                    }
                    onboarding.set_status("Voice providers refreshed.");
                },
                Err(error) => {
                    onboarding.set_error(format!("Failed to load voice providers: {error}"))
                },
            }
            self.state.dirty = true;
        }
    }

    async fn toggle_selected_voice_provider(&mut self, rpc: &Arc<RpcClient>) {
        let Some(provider) = self
            .onboarding
            .as_ref()
            .and_then(|onboarding| {
                onboarding
                    .voice
                    .providers
                    .get(onboarding.voice.selected_provider)
            })
            .cloned()
        else {
            return;
        };

        if !provider.available {
            if let Some(onboarding) = self.onboarding.as_mut() {
                onboarding.set_error("Selected voice provider is not available.");
                self.state.dirty = true;
            }
            return;
        }

        let result = rpc
            .call(
                "voice.provider.toggle",
                serde_json::json!({
                    "provider": provider.id,
                    "enabled": !provider.enabled,
                    "type": provider.provider_type,
                }),
            )
            .await;

        let mut refresh_voice = false;
        if let Some(onboarding) = self.onboarding.as_mut() {
            match result {
                Ok(_) => {
                    onboarding.set_status("Voice provider updated.");
                    refresh_voice = true;
                },
                Err(error) => onboarding.set_error(format!("Voice toggle failed: {error}")),
            }
            self.state.dirty = true;
        }

        if refresh_voice {
            self.refresh_voice_providers(rpc).await;
        }
    }

    async fn save_selected_voice_key(&mut self, rpc: &Arc<RpcClient>) {
        let Some((provider_id, key)) = self
            .onboarding
            .as_ref()
            .and_then(|onboarding| {
                onboarding
                    .voice
                    .providers
                    .get(onboarding.voice.selected_provider)
            })
            .map(|provider| {
                (
                    provider.id.clone(),
                    self.onboarding
                        .as_ref()
                        .map(|onboarding| onboarding.voice.pending_api_key.clone())
                        .unwrap_or_default(),
                )
            })
        else {
            return;
        };

        if key.trim().is_empty() {
            if let Some(onboarding) = self.onboarding.as_mut() {
                onboarding.set_error("Voice API key cannot be empty.");
                self.state.dirty = true;
            }
            return;
        }

        let result = rpc
            .call(
                "voice.config.save_key",
                serde_json::json!({
                    "provider": provider_id,
                    "api_key": key.trim(),
                }),
            )
            .await;

        let mut refresh_voice = false;
        if let Some(onboarding) = self.onboarding.as_mut() {
            match result {
                Ok(_) => {
                    onboarding.voice.pending_api_key.clear();
                    onboarding.set_status("Voice API key saved.");
                    refresh_voice = true;
                },
                Err(error) => onboarding.set_error(format!("Failed to save voice key: {error}")),
            }
            self.state.dirty = true;
        }

        if refresh_voice {
            self.refresh_voice_providers(rpc).await;
        }
    }

    async fn handle_channel_step_key(
        &mut self,
        key: KeyEvent,
        rpc: &Arc<RpcClient>,
        textarea: &mut TextArea<'_>,
    ) {
        if self
            .onboarding
            .as_ref()
            .is_some_and(|onboarding| onboarding.channel.configuring)
        {
            self.handle_channel_config_key(key, rpc, textarea).await;
            return;
        }

        let Some(onboarding) = self.onboarding.as_mut() else {
            return;
        };

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                let next = onboarding.channel.selected_provider.saturating_add(1);
                onboarding.channel.selected_provider = next.min(ChannelProvider::ALL.len() - 1);
                self.state.dirty = true;
            },
            KeyCode::Char('k') | KeyCode::Up => {
                onboarding.channel.selected_provider =
                    onboarding.channel.selected_provider.saturating_sub(1);
                self.state.dirty = true;
            },
            KeyCode::Char('e') | KeyCode::Enter => {
                let provider = ChannelProvider::from_index(onboarding.channel.selected_provider);
                if provider.available() {
                    onboarding.channel.configuring = true;
                    onboarding.channel.field_index = 0;
                    onboarding.clear_messages();
                } else {
                    onboarding
                        .set_status(format!("{} onboarding is coming soon.", provider.name()));
                }
                self.state.dirty = true;
            },
            KeyCode::Char('c') => {
                if onboarding.channel.connected {
                    onboarding.go_next();
                    onboarding.clear_messages();
                } else {
                    onboarding.set_error("Connect a channel first, or press s to skip.");
                }
                self.state.dirty = true;
            },
            KeyCode::Char('s') => {
                onboarding.go_next();
                onboarding.clear_messages();
                self.state.dirty = true;
            },
            KeyCode::Char('b') => {
                onboarding.go_back();
                onboarding.clear_messages();
                self.state.dirty = true;
            },
            _ => {},
        }
    }

    async fn handle_channel_config_key(
        &mut self,
        key: KeyEvent,
        rpc: &Arc<RpcClient>,
        textarea: &mut TextArea<'_>,
    ) {
        if key.code == KeyCode::Char('x') {
            self.connect_telegram_channel(rpc).await;
            return;
        }

        let Some(onboarding) = self.onboarding.as_mut() else {
            return;
        };

        if ChannelProvider::from_index(onboarding.channel.selected_provider)
            != ChannelProvider::Telegram
        {
            onboarding.channel.configuring = false;
            onboarding.set_error("Selected channel cannot be configured yet.");
            self.state.dirty = true;
            return;
        }

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                onboarding.channel.field_index = (onboarding.channel.field_index + 1).min(3);
                self.state.dirty = true;
            },
            KeyCode::Char('k') | KeyCode::Up => {
                onboarding.channel.field_index = onboarding.channel.field_index.saturating_sub(1);
                self.state.dirty = true;
            },
            KeyCode::Char('e') | KeyCode::Enter => {
                if let Some(target) = channel_edit_target(onboarding.channel.field_index) {
                    self.start_onboarding_edit(target, textarea);
                } else if onboarding.channel.field_index == 2 {
                    onboarding.channel.dm_policy = next_dm_policy(&onboarding.channel.dm_policy);
                    self.state.dirty = true;
                }
            },
            KeyCode::Char('[') | KeyCode::Left if onboarding.channel.field_index == 2 => {
                onboarding.channel.dm_policy = previous_dm_policy(&onboarding.channel.dm_policy);
                self.state.dirty = true;
            },
            KeyCode::Char(']') | KeyCode::Right if onboarding.channel.field_index == 2 => {
                onboarding.channel.dm_policy = next_dm_policy(&onboarding.channel.dm_policy);
                self.state.dirty = true;
            },
            KeyCode::Esc => {
                onboarding.channel.configuring = false;
                onboarding.clear_messages();
                self.state.dirty = true;
            },
            _ => {},
        }
    }

    async fn connect_telegram_channel(&mut self, rpc: &Arc<RpcClient>) {
        let Some(channel) = self
            .onboarding
            .as_ref()
            .map(|onboarding| onboarding.channel.clone())
        else {
            return;
        };

        if channel.account_id.trim().is_empty() {
            if let Some(onboarding) = self.onboarding.as_mut() {
                onboarding.set_error("Bot username is required.");
                self.state.dirty = true;
            }
            return;
        }
        if channel.token.trim().is_empty() {
            if let Some(onboarding) = self.onboarding.as_mut() {
                onboarding.set_error("Bot token is required.");
                self.state.dirty = true;
            }
            return;
        }

        let allowlist = channel
            .allowlist
            .lines()
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(|entry| entry.trim_start_matches('@').to_string())
            .collect::<Vec<String>>();

        let payload = serde_json::json!({
            "type": "telegram",
            "account_id": channel.account_id.trim(),
            "config": {
                "token": channel.token.trim(),
                "dm_policy": channel.dm_policy,
                "mention_mode": "mention",
                "allowlist": allowlist,
            }
        });

        let result = rpc.call("channels.add", payload).await;

        if let Some(onboarding) = self.onboarding.as_mut() {
            match result {
                Ok(_) => {
                    onboarding.channel.connected = true;
                    onboarding.channel.connected_name = channel.account_id.trim().to_string();
                    onboarding.channel.configuring = false;
                    onboarding.set_status("Telegram bot connected.");
                },
                Err(error) => {
                    onboarding.set_error(format!("Failed to connect Telegram bot: {error}"))
                },
            }
            self.state.dirty = true;
        }
    }

    async fn handle_identity_step_key(
        &mut self,
        key: KeyEvent,
        rpc: &Arc<RpcClient>,
        textarea: &mut TextArea<'_>,
    ) {
        if key.code == KeyCode::Char('c') {
            self.submit_identity_step(rpc).await;
            return;
        }

        let Some(onboarding) = self.onboarding.as_mut() else {
            return;
        };

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                onboarding.identity.field_index = (onboarding.identity.field_index + 1).min(4);
                self.state.dirty = true;
            },
            KeyCode::Char('k') | KeyCode::Up => {
                onboarding.identity.field_index = onboarding.identity.field_index.saturating_sub(1);
                self.state.dirty = true;
            },
            KeyCode::Char('e') | KeyCode::Enter => {
                if let Some(target) = identity_edit_target(onboarding.identity.field_index) {
                    self.start_onboarding_edit(target, textarea);
                }
            },
            KeyCode::Char('b') => {
                onboarding.go_back();
                onboarding.clear_messages();
                self.state.dirty = true;
            },
            _ => {},
        }
    }

    async fn submit_identity_step(&mut self, rpc: &Arc<RpcClient>) {
        let Some(identity) = self
            .onboarding
            .as_ref()
            .map(|onboarding| onboarding.identity.clone())
        else {
            return;
        };

        if identity.user_name.trim().is_empty() {
            if let Some(onboarding) = self.onboarding.as_mut() {
                onboarding.set_error("Your name is required.");
                self.state.dirty = true;
            }
            return;
        }
        if identity.agent_name.trim().is_empty() {
            if let Some(onboarding) = self.onboarding.as_mut() {
                onboarding.set_error("Agent name is required.");
                self.state.dirty = true;
            }
            return;
        }

        let payload = serde_json::json!({
            "name": identity.agent_name.trim(),
            "emoji": identity.emoji.trim(),
            "creature": identity.creature.trim(),
            "vibe": identity.vibe.trim(),
            "user_name": identity.user_name.trim(),
        });

        let result = rpc.call("agent.identity.update", payload).await;
        if let Some(onboarding) = self.onboarding.as_mut() {
            match result {
                Ok(_) => {
                    onboarding.set_status("Identity saved.");
                    onboarding.go_next();
                },
                Err(error) => onboarding.set_error(format!("Failed to save identity: {error}")),
            }
            self.state.dirty = true;
        }
    }

    async fn handle_summary_step_key(&mut self, key: KeyEvent, rpc: &Arc<RpcClient>) {
        match key.code {
            KeyCode::Char('r') => {
                self.refresh_summary(rpc).await;
            },
            KeyCode::Char('b') => {
                if let Some(onboarding) = self.onboarding.as_mut() {
                    onboarding.go_back();
                    onboarding.clear_messages();
                    self.state.dirty = true;
                }
            },
            KeyCode::Char('f') | KeyCode::Char('c') | KeyCode::Enter => {
                self.finish_onboarding(rpc).await;
            },
            _ => {},
        }
    }

    async fn refresh_summary(&mut self, rpc: &Arc<RpcClient>) {
        let (identity_res, providers_res, channels_res, voice_res) = tokio::join!(
            rpc.call("agent.identity.get", serde_json::json!({})),
            rpc.call("providers.available", serde_json::json!({})),
            rpc.call("channels.status", serde_json::json!({})),
            rpc.call("voice.providers.all", serde_json::json!({})),
        );

        if let Some(onboarding) = self.onboarding.as_mut() {
            let mut summary = onboarding.summary.clone();

            if let Ok(identity) = identity_res {
                let user = identity
                    .get("user_name")
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                let name = identity
                    .get("name")
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                let emoji = identity
                    .get("emoji")
                    .and_then(|value| value.as_str())
                    .unwrap_or("");

                if !user.is_empty() && !name.is_empty() {
                    summary.identity_line = Some(format!("You: {user}  Agent: {emoji} {name}"));
                } else {
                    summary.identity_line = None;
                }
            }

            if let Ok(providers) = providers_res {
                let parsed = parse_providers(&providers);
                summary.provider_badges = configured_provider_badges(&parsed);
            }

            if let Ok(channels) = channels_res {
                summary.channels = parse_channels(&channels);
            }

            if let Ok(voice) = voice_res {
                let providers = parse_voice_providers(&voice);
                summary.voice_enabled = providers
                    .iter()
                    .filter(|provider| provider.enabled)
                    .map(|provider| provider.name.clone())
                    .collect::<Vec<String>>();
            }

            onboarding.summary = summary;
            onboarding.set_status("Summary refreshed.");
            self.state.dirty = true;
        }
    }

    async fn finish_onboarding(&mut self, rpc: &Arc<RpcClient>) {
        self.onboarding = None;
        self.state.input_mode = InputMode::Insert;
        self.state.sidebar_visible = true;
        self.state.dirty = true;

        self.load_initial_data_now(rpc).await;
    }

    async fn load_initial_data_now(&mut self, rpc: &Arc<RpcClient>) {
        let (sessions_res, history_res, context_res) = tokio::join!(
            rpc.call("sessions.list", serde_json::json!({})),
            rpc.call("chat.history", serde_json::json!({})),
            rpc.call("chat.context", serde_json::json!({})),
        );

        let mut data = InitialData::default();
        if let Ok(sessions) = sessions_res
            && let Some(arr) = sessions.as_array()
        {
            data.sessions = Some(
                arr.iter()
                    .filter_map(|s| {
                        let key = s.get("key").and_then(|v| v.as_str())?;
                        Some(SessionEntry {
                            key: key.into(),
                            label: s
                                .get("label")
                                .and_then(|v| v.as_str())
                                .map(ToOwned::to_owned),
                            model: s
                                .get("model")
                                .and_then(|v| v.as_str())
                                .map(ToOwned::to_owned),
                            message_count: s
                                .get("message_count")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0),
                            replying: s.get("replying").and_then(|v| v.as_bool()).unwrap_or(false),
                        })
                    })
                    .collect(),
            );
        }

        if let Ok(history) = history_res
            && let Some(arr) = history.as_array()
        {
            data.messages = Some(
                arr.iter()
                    .filter_map(|msg| {
                        let role = msg.get("role").and_then(|v| v.as_str())?;
                        let content = msg
                            .get("content")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let role = match role {
                            "user" => MessageRole::User,
                            "assistant" => MessageRole::Assistant,
                            _ => MessageRole::System,
                        };
                        Some(DisplayMessage {
                            role,
                            content,
                            tool_calls: Vec::new(),
                            thinking: None,
                        })
                    })
                    .collect(),
            );
        }

        if let Ok(context) = context_res
            && let Some(session) = context.get("session")
        {
            data.active_session = session
                .get("key")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned);
            data.model = session
                .get("model")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned);
            data.provider = session
                .get("provider")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned);
        }

        self.apply_initial_data(data);
    }

    fn start_onboarding_edit(&mut self, target: EditTarget, textarea: &mut TextArea<'_>) {
        let Some(onboarding) = self.onboarding.as_mut() else {
            return;
        };

        let current = onboarding.begin_edit(target);
        *textarea = TextArea::from(vec![current]);
        textarea.set_placeholder_text(target.placeholder());
        self.state.input_mode = InputMode::Insert;
        self.state.dirty = true;
    }

    fn cancel_onboarding_edit(&mut self, textarea: &mut TextArea<'_>) {
        if let Some(onboarding) = self.onboarding.as_mut() {
            onboarding.cancel_edit();
        }
        *textarea = TextArea::default();
        textarea.set_placeholder_text("Press 'e' to edit selected field");
        self.state.input_mode = InputMode::Normal;
        self.state.dirty = true;
    }

    pub(super) async fn handle_onboarding_insert_key(
        &mut self,
        key: KeyEvent,
        _rpc: &Arc<RpcClient>,
        textarea: &mut TextArea<'_>,
    ) {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => self.cancel_onboarding_edit(textarea),
            (KeyCode::Enter, KeyModifiers::NONE) => {
                let target = self
                    .onboarding
                    .as_ref()
                    .and_then(|onboarding| onboarding.editing);
                if let Some(target) = target {
                    let value = textarea.lines().join("\n");
                    if let Some(onboarding) = self.onboarding.as_mut() {
                        onboarding.commit_edit(target, value);
                    }
                }
                *textarea = TextArea::default();
                textarea.set_placeholder_text("Press 'e' to edit selected field");
                self.state.input_mode = InputMode::Normal;
                self.state.dirty = true;
            },
            (KeyCode::Enter, KeyModifiers::SHIFT) => {
                textarea.insert_newline();
                self.state.dirty = true;
            },
            _ => {
                textarea.input(key);
                self.state.dirty = true;
            },
        }
    }
}

fn security_edit_target(security: &SecurityState) -> EditTarget {
    if security.setup_code_required {
        return match security.field_index {
            0 => EditTarget::SecuritySetupCode,
            1 => EditTarget::SecurityPassword,
            _ => EditTarget::SecurityConfirmPassword,
        };
    }

    match security.field_index {
        0 => EditTarget::SecurityPassword,
        _ => EditTarget::SecurityConfirmPassword,
    }
}

fn provider_edit_target(config: &ProviderConfigureState) -> Option<EditTarget> {
    match config.field_index {
        0 => Some(EditTarget::ProviderApiKey),
        1 if supports_endpoint(&config.provider_name) => Some(EditTarget::ProviderEndpoint),
        1 if config.requires_model => Some(EditTarget::ProviderModel),
        2 if supports_endpoint(&config.provider_name) && config.requires_model => {
            Some(EditTarget::ProviderModel)
        },
        _ => None,
    }
}

fn channel_edit_target(field_index: usize) -> Option<EditTarget> {
    match field_index {
        0 => Some(EditTarget::ChannelAccountId),
        1 => Some(EditTarget::ChannelToken),
        3 => Some(EditTarget::ChannelAllowlist),
        _ => None,
    }
}

fn identity_edit_target(field_index: usize) -> Option<EditTarget> {
    match field_index {
        0 => Some(EditTarget::IdentityUserName),
        1 => Some(EditTarget::IdentityAgentName),
        2 => Some(EditTarget::IdentityEmoji),
        3 => Some(EditTarget::IdentityCreature),
        4 => Some(EditTarget::IdentityVibe),
        _ => None,
    }
}

fn previous_dm_policy(current: &str) -> String {
    const OPTIONS: [&str; 3] = ["allowlist", "open", "disabled"];
    let idx = OPTIONS
        .iter()
        .position(|value| *value == current)
        .unwrap_or(0);
    let previous = if idx == 0 {
        OPTIONS.len() - 1
    } else {
        idx - 1
    };
    OPTIONS[previous].to_string()
}

fn next_dm_policy(current: &str) -> String {
    const OPTIONS: [&str; 3] = ["allowlist", "open", "disabled"];
    let idx = OPTIONS
        .iter()
        .position(|value| *value == current)
        .unwrap_or(0);
    let next = (idx + 1) % OPTIONS.len();
    OPTIONS[next].to_string()
}

fn parse_provider_models_from_list(
    payload: &Value,
    provider_name: &str,
) -> Vec<crate::onboarding::ModelOption> {
    payload
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let id = row.get("id").and_then(Value::as_str)?.to_string();
                    let row_provider = row
                        .get("provider")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    if !model_matches_provider(provider_name, row_provider, &id) {
                        return None;
                    }

                    let display_name = row
                        .get("displayName")
                        .and_then(Value::as_str)
                        .unwrap_or(&id)
                        .to_string();
                    let supports_tools = row
                        .get("supportsTools")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);

                    Some(crate::onboarding::ModelOption {
                        id,
                        display_name,
                        supports_tools,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn select_models_for_picker(
    models: &[crate::onboarding::ModelOption],
    saved_models: &[String],
    current_model: &str,
) -> (BTreeSet<String>, usize) {
    let mut selected = saved_models.iter().cloned().collect::<BTreeSet<String>>();
    if selected.is_empty() && !current_model.trim().is_empty() {
        selected.insert(current_model.trim().to_string());
    }
    selected.retain(|model_id| models.iter().any(|model| model.id == *model_id));

    let cursor = models
        .iter()
        .position(|model| selected.contains(&model.id))
        .unwrap_or(0);
    (selected, cursor)
}

fn model_matches_provider(provider_name: &str, row_provider: &str, model_id: &str) -> bool {
    if !row_provider.is_empty() {
        if row_provider == provider_name {
            return true;
        }
        if (provider_name == "zai" && row_provider == "z.ai")
            || (provider_name == "z.ai" && row_provider == "zai")
        {
            return true;
        }
        return false;
    }

    model_id.starts_with(&format!("{provider_name}/"))
        || model_id.starts_with(&format!("{provider_name}:"))
}

fn http_base_url_from_ws(gateway_url: &str) -> Option<String> {
    let mut url = Url::parse(gateway_url).ok()?;

    match url.scheme() {
        "ws" => {
            let _ = url.set_scheme("http");
        },
        "wss" => {
            let _ = url.set_scheme("https");
        },
        "http" | "https" => {},
        _ => return None,
    }

    let is_loopback_ip = url.host().is_some_and(|host| match host {
        Host::Ipv4(ip) => ip.is_loopback(),
        Host::Ipv6(ip) => ip.is_loopback(),
        Host::Domain(_) => false,
    });

    if is_loopback_ip {
        let _ = url.set_host(Some("localhost"));
    }

    url.set_path("");
    url.set_query(None);
    url.set_fragment(None);

    Some(url.to_string().trim_end_matches('/').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_ws_loopback_to_http_localhost() {
        let url = http_base_url_from_ws("ws://127.0.0.1:57223/ws/chat");
        assert_eq!(url.as_deref(), Some("http://localhost:57223"));
    }

    #[test]
    fn converts_wss_loopback_ipv6_to_https_localhost() {
        let url = http_base_url_from_ws("wss://[::1]:57223/ws/chat");
        assert_eq!(url.as_deref(), Some("https://localhost:57223"));
    }

    #[test]
    fn dm_policy_cycles_both_directions() {
        assert_eq!(previous_dm_policy("allowlist"), "disabled");
        assert_eq!(next_dm_policy("disabled"), "allowlist");
    }

    #[test]
    fn parse_provider_models_filters_by_provider_and_alias() {
        let payload = serde_json::json!([
            {"id":"openai/gpt-5", "provider":"openai", "displayName":"GPT-5", "supportsTools":true},
            {"id":"zai/glm-4.6", "provider":"z.ai", "displayName":"GLM-4.6", "supportsTools":true},
            {"id":"anthropic/claude-sonnet-4", "provider":"anthropic", "displayName":"Claude Sonnet 4", "supportsTools":true}
        ]);

        let openai_models = parse_provider_models_from_list(&payload, "openai");
        assert_eq!(openai_models.len(), 1);
        assert_eq!(openai_models[0].id, "openai/gpt-5");

        let zai_models = parse_provider_models_from_list(&payload, "zai");
        assert_eq!(zai_models.len(), 1);
        assert_eq!(zai_models[0].id, "zai/glm-4.6");
    }

    #[test]
    fn select_models_for_picker_prefers_saved_and_falls_back_to_current() {
        let models = vec![
            crate::onboarding::ModelOption {
                id: "openai/gpt-5".to_string(),
                display_name: "GPT-5".to_string(),
                supports_tools: true,
            },
            crate::onboarding::ModelOption {
                id: "openai/gpt-4.1".to_string(),
                display_name: "GPT-4.1".to_string(),
                supports_tools: true,
            },
        ];

        let (selected_saved, cursor_saved) =
            select_models_for_picker(&models, &["openai/gpt-4.1".to_string()], "openai/gpt-5");
        assert!(selected_saved.contains("openai/gpt-4.1"));
        assert_eq!(cursor_saved, 1);

        let (selected_current, cursor_current) =
            select_models_for_picker(&models, &[], "openai/gpt-5");
        assert!(selected_current.contains("openai/gpt-5"));
        assert_eq!(cursor_current, 0);
    }
}
