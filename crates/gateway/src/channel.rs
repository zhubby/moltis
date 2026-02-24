use std::sync::Arc;

use {
    async_trait::async_trait,
    serde_json::Value,
    tokio::sync::RwLock,
    tracing::{error, info, warn},
};

use {
    moltis_channels::{
        ChannelPlugin, ChannelType,
        message_log::MessageLog,
        store::{ChannelStore, StoredChannel},
    },
    moltis_msteams::MsTeamsPlugin,
    moltis_sessions::metadata::SqliteSessionMetadata,
    moltis_telegram::TelegramPlugin,
};

use crate::services::{ChannelService, ServiceResult};

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Live channel service backed by Telegram and Microsoft Teams plugins.
pub struct LiveChannelService {
    telegram: Arc<RwLock<TelegramPlugin>>,
    msteams: Arc<RwLock<MsTeamsPlugin>>,
    store: Arc<dyn ChannelStore>,
    message_log: Arc<dyn MessageLog>,
    session_metadata: Arc<SqliteSessionMetadata>,
}

impl LiveChannelService {
    pub fn new(
        telegram: Arc<RwLock<TelegramPlugin>>,
        msteams: Arc<RwLock<MsTeamsPlugin>>,
        store: Arc<dyn ChannelStore>,
        message_log: Arc<dyn MessageLog>,
        session_metadata: Arc<SqliteSessionMetadata>,
    ) -> Self {
        Self {
            telegram,
            msteams,
            store,
            message_log,
            session_metadata,
        }
    }

    async fn resolve_channel_type(
        &self,
        params: &Value,
        account_id: &str,
        default_when_unknown: ChannelType,
    ) -> Result<ChannelType, String> {
        if let Some(type_str) = params.get("type").and_then(|v| v.as_str()) {
            return type_str.parse::<ChannelType>().map_err(|e| e.to_string());
        }

        let (tg_has, ms_has) = {
            let tg = self.telegram.read().await;
            let ms = self.msteams.read().await;
            (tg.has_account(account_id), ms.has_account(account_id))
        };

        match (tg_has, ms_has) {
            (true, false) => Ok(ChannelType::Telegram),
            (false, true) => Ok(ChannelType::MsTeams),
            (true, true) => Err(format!(
                "account_id '{account_id}' exists in multiple channel types; pass explicit 'type'"
            )),
            (false, false) => {
                let tg_store = self
                    .store
                    .get(ChannelType::Telegram.as_str(), account_id)
                    .await
                    .map_err(|e| e.to_string())?
                    .is_some();
                let ms_store = self
                    .store
                    .get(ChannelType::MsTeams.as_str(), account_id)
                    .await
                    .map_err(|e| e.to_string())?
                    .is_some();
                match (tg_store, ms_store) {
                    (true, false) => Ok(ChannelType::Telegram),
                    (false, true) => Ok(ChannelType::MsTeams),
                    (true, true) => Err(format!(
                        "account_id '{account_id}' exists in multiple stored channel types; pass explicit 'type'"
                    )),
                    (false, false) => Ok(default_when_unknown),
                }
            },
        }
    }
}

#[async_trait]
impl ChannelService for LiveChannelService {
    async fn status(&self) -> ServiceResult {
        let mut channels = Vec::new();

        {
            let tg = self.telegram.read().await;
            let account_ids = tg.account_ids();
            if let Some(status) = tg.status() {
                for aid in &account_ids {
                    match status.probe(aid).await {
                        Ok(snap) => {
                            let mut entry = serde_json::json!({
                                "type": "telegram",
                                "name": format!("Telegram ({aid})"),
                                "account_id": aid,
                                "status": if snap.connected { "connected" } else { "disconnected" },
                                "details": snap.details,
                            });
                            if let Some(cfg) = tg.account_config(aid) {
                                entry["config"] = cfg;
                            }

                            let bound = self
                                .session_metadata
                                .list_account_sessions(ChannelType::Telegram.as_str(), aid)
                                .await;
                            let active_map = self
                                .session_metadata
                                .list_active_sessions(ChannelType::Telegram.as_str(), aid)
                                .await;
                            let sessions: Vec<_> = bound
                                .iter()
                                .map(|s| {
                                    let is_active = active_map.iter().any(|(_, sk)| sk == &s.key);
                                    serde_json::json!({
                                        "key": s.key,
                                        "label": s.label,
                                        "messageCount": s.message_count,
                                        "active": is_active,
                                    })
                                })
                                .collect();
                            if !sessions.is_empty() {
                                entry["sessions"] = serde_json::json!(sessions);
                            }
                            channels.push(entry);
                        },
                        Err(e) => channels.push(serde_json::json!({
                            "type": "telegram",
                            "name": format!("Telegram ({aid})"),
                            "account_id": aid,
                            "status": "error",
                            "details": e.to_string(),
                        })),
                    }
                }
            }
        }

        {
            let ms = self.msteams.read().await;
            let account_ids = ms.account_ids();
            if let Some(status) = ms.status() {
                for aid in &account_ids {
                    match status.probe(aid).await {
                        Ok(snap) => {
                            let mut entry = serde_json::json!({
                                "type": "msteams",
                                "name": format!("Microsoft Teams ({aid})"),
                                "account_id": aid,
                                "status": if snap.connected { "connected" } else { "disconnected" },
                                "details": snap.details,
                            });
                            if let Some(cfg) = ms.account_config(aid) {
                                entry["config"] = cfg;
                            }

                            let bound = self
                                .session_metadata
                                .list_account_sessions(ChannelType::MsTeams.as_str(), aid)
                                .await;
                            let active_map = self
                                .session_metadata
                                .list_active_sessions(ChannelType::MsTeams.as_str(), aid)
                                .await;
                            let sessions: Vec<_> = bound
                                .iter()
                                .map(|s| {
                                    let is_active = active_map.iter().any(|(_, sk)| sk == &s.key);
                                    serde_json::json!({
                                        "key": s.key,
                                        "label": s.label,
                                        "messageCount": s.message_count,
                                        "active": is_active,
                                    })
                                })
                                .collect();
                            if !sessions.is_empty() {
                                entry["sessions"] = serde_json::json!(sessions);
                            }
                            channels.push(entry);
                        },
                        Err(e) => channels.push(serde_json::json!({
                            "type": "msteams",
                            "name": format!("Microsoft Teams ({aid})"),
                            "account_id": aid,
                            "status": "error",
                            "details": e.to_string(),
                        })),
                    }
                }
            }
        }

        Ok(serde_json::json!({ "channels": channels }))
    }

    async fn add(&self, params: Value) -> ServiceResult {
        let account_id = params
            .get("account_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'account_id'".to_string())?;
        let channel_type = self
            .resolve_channel_type(&params, account_id, ChannelType::Telegram)
            .await?;
        let config = params
            .get("config")
            .cloned()
            .unwrap_or(Value::Object(Default::default()));

        match channel_type {
            ChannelType::Telegram => {
                info!(account_id, "adding telegram channel account");
                let mut tg = self.telegram.write().await;
                tg.start_account(account_id, config.clone())
                    .await
                    .map_err(|e| {
                        error!(error = %e, account_id, "failed to start telegram account");
                        e.to_string()
                    })?;
            },
            ChannelType::MsTeams => {
                info!(account_id, "adding microsoft teams channel account");
                let mut ms = self.msteams.write().await;
                ms.start_account(account_id, config.clone())
                    .await
                    .map_err(|e| {
                        error!(error = %e, account_id, "failed to start teams account");
                        e.to_string()
                    })?;
            },
        }

        let now = unix_now();
        if let Err(e) = self
            .store
            .upsert(StoredChannel {
                account_id: account_id.to_string(),
                channel_type: channel_type.to_string(),
                config,
                created_at: now,
                updated_at: now,
            })
            .await
        {
            warn!(error = %e, account_id, "failed to persist channel");
        }

        Ok(serde_json::json!({
            "added": account_id,
            "type": channel_type.to_string()
        }))
    }

    async fn remove(&self, params: Value) -> ServiceResult {
        let account_id = params
            .get("account_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'account_id'".to_string())?;
        let channel_type = self
            .resolve_channel_type(&params, account_id, ChannelType::Telegram)
            .await?;

        match channel_type {
            ChannelType::Telegram => {
                info!(account_id, "removing telegram channel account");
                let mut tg = self.telegram.write().await;
                tg.stop_account(account_id).await.map_err(|e| {
                    error!(error = %e, account_id, "failed to stop telegram account");
                    e.to_string()
                })?;
            },
            ChannelType::MsTeams => {
                info!(account_id, "removing microsoft teams channel account");
                let mut ms = self.msteams.write().await;
                ms.stop_account(account_id).await.map_err(|e| {
                    error!(error = %e, account_id, "failed to stop teams account");
                    e.to_string()
                })?;
            },
        }

        if let Err(e) = self.store.delete(channel_type.as_str(), account_id).await {
            warn!(error = %e, account_id, "failed to delete channel from store");
        }

        Ok(serde_json::json!({
            "removed": account_id,
            "type": channel_type.to_string()
        }))
    }

    async fn logout(&self, params: Value) -> ServiceResult {
        self.remove(params).await
    }

    async fn update(&self, params: Value) -> ServiceResult {
        let account_id = params
            .get("account_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'account_id'".to_string())?;
        let channel_type = self
            .resolve_channel_type(&params, account_id, ChannelType::Telegram)
            .await?;
        let config = params
            .get("config")
            .cloned()
            .ok_or_else(|| "missing 'config'".to_string())?;

        match channel_type {
            ChannelType::Telegram => {
                info!(account_id, "updating telegram channel account");
                let mut tg = self.telegram.write().await;
                tg.stop_account(account_id).await.map_err(|e| {
                    error!(error = %e, account_id, "failed to stop telegram account for update");
                    e.to_string()
                })?;
                tg.start_account(account_id, config.clone())
                    .await
                    .map_err(|e| {
                        error!(
                            error = %e,
                            account_id,
                            "failed to restart telegram account after update"
                        );
                        e.to_string()
                    })?;
            },
            ChannelType::MsTeams => {
                info!(account_id, "updating microsoft teams channel account");
                let mut ms = self.msteams.write().await;
                ms.stop_account(account_id).await.map_err(|e| {
                    error!(error = %e, account_id, "failed to stop teams account for update");
                    e.to_string()
                })?;
                ms.start_account(account_id, config.clone())
                    .await
                    .map_err(|e| {
                        error!(
                            error = %e,
                            account_id,
                            "failed to restart teams account after update"
                        );
                        e.to_string()
                    })?;
            },
        }

        let created_at = self
            .store
            .get(channel_type.as_str(), account_id)
            .await
            .map_err(|e| e.to_string())?
            .map(|s| s.created_at)
            .unwrap_or_else(unix_now);
        let now = unix_now();
        if let Err(e) = self
            .store
            .upsert(StoredChannel {
                account_id: account_id.to_string(),
                channel_type: channel_type.to_string(),
                config,
                created_at,
                updated_at: now,
            })
            .await
        {
            warn!(error = %e, account_id, "failed to persist channel update");
        }

        Ok(serde_json::json!({
            "updated": account_id,
            "type": channel_type.to_string()
        }))
    }

    async fn send(&self, _params: Value) -> ServiceResult {
        Err("direct channel send not yet implemented".into())
    }

    async fn senders_list(&self, params: Value) -> ServiceResult {
        let account_id = params
            .get("account_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'account_id'".to_string())?;
        let channel_type = self
            .resolve_channel_type(&params, account_id, ChannelType::Telegram)
            .await?;

        let senders = self
            .message_log
            .unique_senders(channel_type.as_str(), account_id)
            .await
            .map_err(|e| e.to_string())?;

        let allowlist: Vec<String> = match channel_type {
            ChannelType::Telegram => {
                let tg = self.telegram.read().await;
                tg.account_config(account_id)
                    .and_then(|cfg| cfg.get("allowlist").cloned())
                    .and_then(|v| serde_json::from_value(v).ok())
                    .unwrap_or_default()
            },
            ChannelType::MsTeams => {
                let ms = self.msteams.read().await;
                ms.account_config(account_id)
                    .and_then(|cfg| cfg.get("allowlist").cloned())
                    .and_then(|v| serde_json::from_value(v).ok())
                    .unwrap_or_default()
            },
        };

        let otp_challenges = if channel_type == ChannelType::Telegram {
            let tg = self.telegram.read().await;
            Some(tg.pending_otp_challenges(account_id))
        } else {
            None
        };

        let list: Vec<Value> = senders
            .into_iter()
            .map(|s| {
                let is_allowed = allowlist.iter().any(|a| {
                    let a_lower = a.to_lowercase();
                    a_lower == s.peer_id.to_lowercase()
                        || s.username
                            .as_ref()
                            .is_some_and(|u| a_lower == u.to_lowercase())
                });
                let mut entry = serde_json::json!({
                    "peer_id": s.peer_id,
                    "username": s.username,
                    "sender_name": s.sender_name,
                    "message_count": s.message_count,
                    "last_seen": s.last_seen,
                    "allowed": is_allowed,
                });
                if let Some(otp) = otp_challenges
                    .as_ref()
                    .and_then(|pending| pending.iter().find(|c| c.peer_id == s.peer_id))
                {
                    entry["otp_pending"] = serde_json::json!({
                        "code": otp.code,
                        "expires_at": otp.expires_at,
                    });
                }
                entry
            })
            .collect();

        Ok(serde_json::json!({
            "senders": list,
            "type": channel_type.to_string()
        }))
    }

    async fn sender_approve(&self, params: Value) -> ServiceResult {
        let account_id = params
            .get("account_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'account_id'".to_string())?;
        let identifier = params
            .get("identifier")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'identifier'".to_string())?;
        let channel_type = self
            .resolve_channel_type(&params, account_id, ChannelType::Telegram)
            .await?;

        let stored = self
            .store
            .get(channel_type.as_str(), account_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| {
                format!(
                    "channel '{}' ({}) not found in store",
                    account_id,
                    channel_type.as_str()
                )
            })?;

        let mut config = stored.config.clone();
        let allowlist = config
            .as_object_mut()
            .ok_or_else(|| "config is not an object".to_string())?
            .entry("allowlist")
            .or_insert_with(|| serde_json::json!([]));
        let arr = allowlist
            .as_array_mut()
            .ok_or_else(|| "allowlist is not an array".to_string())?;

        let id_lower = identifier.to_lowercase();
        if !arr
            .iter()
            .any(|v| v.as_str().is_some_and(|s| s.to_lowercase() == id_lower))
        {
            arr.push(serde_json::json!(identifier));
        }
        if let Some(obj) = config.as_object_mut() {
            obj.insert("dm_policy".into(), serde_json::json!("allowlist"));
        }

        if let Err(e) = self
            .store
            .upsert(StoredChannel {
                account_id: account_id.to_string(),
                channel_type: channel_type.to_string(),
                config: config.clone(),
                created_at: stored.created_at,
                updated_at: unix_now(),
            })
            .await
        {
            warn!(error = %e, account_id, "failed to persist sender approval");
        }

        match channel_type {
            ChannelType::Telegram => {
                let tg = self.telegram.read().await;
                if let Err(e) = tg.update_account_config(account_id, config) {
                    warn!(error = %e, account_id, "failed to hot-update telegram config");
                }
            },
            ChannelType::MsTeams => {
                let ms = self.msteams.read().await;
                if let Err(e) = ms.update_account_config(account_id, config) {
                    warn!(error = %e, account_id, "failed to hot-update teams config");
                }
            },
        }

        info!(
            account_id,
            identifier,
            channel_type = channel_type.as_str(),
            "sender approved"
        );
        Ok(serde_json::json!({
            "approved": identifier,
            "type": channel_type.to_string()
        }))
    }

    async fn sender_deny(&self, params: Value) -> ServiceResult {
        let account_id = params
            .get("account_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'account_id'".to_string())?;
        let identifier = params
            .get("identifier")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'identifier'".to_string())?;
        let channel_type = self
            .resolve_channel_type(&params, account_id, ChannelType::Telegram)
            .await?;

        let stored = self
            .store
            .get(channel_type.as_str(), account_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| {
                format!(
                    "channel '{}' ({}) not found in store",
                    account_id,
                    channel_type.as_str()
                )
            })?;

        let mut config = stored.config.clone();
        if let Some(arr) = config
            .as_object_mut()
            .and_then(|o| o.get_mut("allowlist"))
            .and_then(|v| v.as_array_mut())
        {
            let id_lower = identifier.to_lowercase();
            arr.retain(|v| v.as_str().is_none_or(|s| s.to_lowercase() != id_lower));
        }

        if let Err(e) = self
            .store
            .upsert(StoredChannel {
                account_id: account_id.to_string(),
                channel_type: channel_type.to_string(),
                config: config.clone(),
                created_at: stored.created_at,
                updated_at: unix_now(),
            })
            .await
        {
            warn!(error = %e, account_id, "failed to persist sender denial");
        }

        match channel_type {
            ChannelType::Telegram => {
                let tg = self.telegram.read().await;
                if let Err(e) = tg.update_account_config(account_id, config) {
                    warn!(error = %e, account_id, "failed to hot-update telegram config");
                }
            },
            ChannelType::MsTeams => {
                let ms = self.msteams.read().await;
                if let Err(e) = ms.update_account_config(account_id, config) {
                    warn!(error = %e, account_id, "failed to hot-update teams config");
                }
            },
        }

        info!(
            account_id,
            identifier,
            channel_type = channel_type.as_str(),
            "sender denied"
        );
        Ok(serde_json::json!({
            "denied": identifier,
            "type": channel_type.to_string()
        }))
    }
}
