//! Gateway adapter: wraps `LiveOnboardingService` to implement `OnboardingService`.

use std::{path::Path, sync::Arc};

use {async_trait::async_trait, serde_json::Value};

use crate::services::{OnboardingService, ServiceError, ServiceResult};

/// Gateway-side onboarding service backed by `moltis_onboarding::service::LiveOnboardingService`.
pub struct GatewayOnboardingService {
    inner: moltis_onboarding::service::LiveOnboardingService,
    session_metadata: Arc<moltis_sessions::metadata::SqliteSessionMetadata>,
}

impl GatewayOnboardingService {
    pub fn new(
        inner: moltis_onboarding::service::LiveOnboardingService,
        session_metadata: Arc<moltis_sessions::metadata::SqliteSessionMetadata>,
    ) -> Self {
        Self {
            inner,
            session_metadata,
        }
    }

    #[cfg(feature = "openclaw-import")]
    async fn sync_imported_sessions_to_sqlite(&self, data_dir: &Path) -> Result<(), String> {
        let metadata_path = data_dir.join("sessions").join("metadata.json");
        if !metadata_path.is_file() {
            return Ok(());
        }

        let legacy_metadata = moltis_sessions::metadata::SessionMetadata::load(metadata_path)
            .map_err(|e| format!("failed to load imported metadata.json: {e}"))?;

        for entry in legacy_metadata.list() {
            self.session_metadata
                .upsert(&entry.key, entry.label.clone())
                .await
                .map_err(|e| format!("failed to upsert session '{}': {e}", entry.key))?;

            self.session_metadata
                .set_model(&entry.key, entry.model.clone())
                .await;
            self.session_metadata
                .set_project_id(&entry.key, entry.project_id.clone())
                .await;
            self.session_metadata
                .set_sandbox_enabled(&entry.key, entry.sandbox_enabled)
                .await;
            self.session_metadata
                .set_sandbox_image(&entry.key, entry.sandbox_image.clone())
                .await;
            self.session_metadata
                .set_worktree_branch(&entry.key, entry.worktree_branch.clone())
                .await;
            self.session_metadata
                .set_channel_binding(&entry.key, entry.channel_binding.clone())
                .await;
            self.session_metadata
                .set_parent(
                    &entry.key,
                    entry.parent_session_key.clone(),
                    entry.fork_point,
                )
                .await;
            self.session_metadata
                .set_mcp_disabled(&entry.key, entry.mcp_disabled)
                .await;
            self.session_metadata
                .set_preview(&entry.key, entry.preview.as_deref())
                .await;
            self.session_metadata
                .set_timestamps_and_counts(
                    &entry.key,
                    entry.created_at,
                    entry.updated_at,
                    entry.message_count,
                    entry.last_seen_message_count,
                )
                .await;
        }

        Ok(())
    }
}

#[async_trait]
impl OnboardingService for GatewayOnboardingService {
    async fn wizard_start(&self, params: Value) -> ServiceResult {
        let force = params
            .get("force")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        Ok(self.inner.wizard_start(force))
    }

    async fn wizard_next(&self, params: Value) -> ServiceResult {
        let input = params.get("input").and_then(|v| v.as_str()).unwrap_or("");
        self.inner.wizard_next(input).map_err(ServiceError::message)
    }

    async fn wizard_cancel(&self) -> ServiceResult {
        self.inner.wizard_cancel();
        Ok(serde_json::json!({}))
    }

    async fn wizard_status(&self) -> ServiceResult {
        Ok(self.inner.wizard_status())
    }

    async fn identity_get(&self) -> ServiceResult {
        Ok(serde_json::to_value(self.inner.identity_get()).unwrap_or_default())
    }

    async fn identity_update(&self, params: Value) -> ServiceResult {
        self.inner
            .identity_update(params)
            .map_err(ServiceError::message)
    }

    async fn identity_update_soul(&self, soul: Option<String>) -> ServiceResult {
        self.inner
            .identity_update_soul(soul)
            .map_err(ServiceError::message)
    }

    #[cfg(feature = "openclaw-import")]
    async fn openclaw_detect(&self) -> ServiceResult {
        let detection = moltis_openclaw_import::detect();
        match detection {
            Some(d) => {
                let scan = moltis_openclaw_import::scan(&d);
                Ok(serde_json::json!({
                    "detected": true,
                    "home_dir": d.home_dir.display().to_string(),
                    "identity_available": scan.identity_available,
                    "providers_available": scan.providers_available,
                    "skills_count": scan.skills_count,
                    "memory_available": scan.memory_available,
                    "memory_files_count": scan.memory_files_count,
                    "channels_available": scan.channels_available,
                    "telegram_accounts": scan.telegram_accounts,
                    "sessions_count": scan.sessions_count,
                    "unsupported_channels": scan.unsupported_channels,
                    "agent_ids": scan.agent_ids,
                }))
            },
            None => Ok(serde_json::json!({ "detected": false })),
        }
    }

    #[cfg(not(feature = "openclaw-import"))]
    async fn openclaw_detect(&self) -> ServiceResult {
        Ok(serde_json::json!({ "detected": false }))
    }

    #[cfg(feature = "openclaw-import")]
    async fn openclaw_scan(&self) -> ServiceResult {
        self.openclaw_detect().await
    }

    #[cfg(not(feature = "openclaw-import"))]
    async fn openclaw_scan(&self) -> ServiceResult {
        Ok(serde_json::json!({ "detected": false }))
    }

    #[cfg(feature = "openclaw-import")]
    async fn openclaw_import(&self, params: Value) -> ServiceResult {
        let detection = moltis_openclaw_import::detect()
            .ok_or_else(|| "no OpenClaw installation found".to_string())?;

        let selection = moltis_openclaw_import::ImportSelection {
            identity: params
                .get("identity")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            providers: params
                .get("providers")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            skills: params
                .get("skills")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            memory: params
                .get("memory")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            channels: params
                .get("channels")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            sessions: params
                .get("sessions")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        };

        let config_dir = moltis_config::config_dir()
            .ok_or_else(|| "could not determine config directory".to_string())?;
        let data_dir = moltis_config::data_dir();

        let report = moltis_openclaw_import::import(&detection, &selection, &config_dir, &data_dir);

        if selection.sessions
            && let Err(e) = self.sync_imported_sessions_to_sqlite(&data_dir).await
        {
            tracing::warn!(error = %e, "openclaw import: failed to sync sessions to sqlite metadata");
        }

        Ok(serde_json::to_value(&report)?)
    }

    #[cfg(not(feature = "openclaw-import"))]
    async fn openclaw_import(&self, _params: Value) -> ServiceResult {
        Err("openclaw import feature not enabled".into())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[cfg(feature = "openclaw-import")]
    #[tokio::test]
    async fn sync_imported_sessions_preserves_timestamps_and_preview() {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path();
        let sessions_dir = data_dir.join("sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();

        std::fs::write(
            sessions_dir.join("metadata.json"),
            r#"{
              "oc:main:demo": {
                "id": "test-id",
                "key": "oc:main:demo",
                "label": "Imported demo",
                "model": "gpt-4o",
                "preview": "hello from imported session",
                "created_at": 1769582795764,
                "updated_at": 1769582801626,
                "message_count": 2,
                "last_seen_message_count": 2,
                "version": 1
              }
            }"#,
        )
        .unwrap();

        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("CREATE TABLE IF NOT EXISTS projects (id TEXT PRIMARY KEY)")
            .execute(&pool)
            .await
            .unwrap();
        moltis_sessions::metadata::SqliteSessionMetadata::init(&pool)
            .await
            .unwrap();

        let service = GatewayOnboardingService::new(
            moltis_onboarding::service::LiveOnboardingService::new(dir.path().join("moltis.toml")),
            Arc::new(moltis_sessions::metadata::SqliteSessionMetadata::new(pool)),
        );

        service
            .sync_imported_sessions_to_sqlite(data_dir)
            .await
            .expect("sync should succeed");

        let entry = service
            .session_metadata
            .get("oc:main:demo")
            .await
            .expect("synced session should exist");
        assert_eq!(entry.created_at, 1769582795764);
        assert_eq!(entry.updated_at, 1769582801626);
        assert_eq!(entry.message_count, 2);
        assert_eq!(entry.last_seen_message_count, 2);
        assert_eq!(
            entry.preview.as_deref(),
            Some("hello from imported session")
        );
    }
}
