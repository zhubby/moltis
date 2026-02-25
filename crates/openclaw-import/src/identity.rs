//! Import user/agent identity from OpenClaw config.

use std::path::Path;

use tracing::debug;

use crate::{
    detect::OpenClawDetection,
    report::{CategoryReport, ImportCategory},
    types::OpenClawConfig,
};

/// Extracted identity from an OpenClaw installation.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ImportedIdentity {
    /// Agent display name (from `ui.assistant.name` or first agent's name).
    pub agent_name: Option<String>,
    /// User display name (from `agents.defaults.userName`).
    pub user_name: Option<String>,
    /// User timezone (from `agents.defaults.userTimezone`).
    pub user_timezone: Option<String>,
}

/// Import identity data from OpenClaw.
pub fn import_identity(detection: &OpenClawDetection) -> (CategoryReport, ImportedIdentity) {
    let config_path = detection.home_dir.join("openclaw.json");
    let config = load_config(&config_path);

    let mut identity = ImportedIdentity::default();
    let mut items = 0;

    // Agent name: prefer ui.assistant.name, fall back to first agent's name
    if let Some(name) = config.ui.assistant.as_ref().and_then(|a| a.name.as_deref()) {
        debug!(name, "importing agent name from ui.assistant.name");
        identity.agent_name = Some(name.to_string());
        items += 1;
    } else if let Some(agent) = config
        .agents
        .list
        .iter()
        .find(|a| a.default)
        .or(config.agents.list.first())
        && let Some(name) = &agent.name
    {
        debug!(name, "importing agent name from agents.list");
        identity.agent_name = Some(name.clone());
        items += 1;
    }

    // User timezone
    if let Some(tz) = &config.agents.defaults.user_timezone {
        debug!(timezone = tz, "importing user timezone");
        identity.user_timezone = Some(tz.clone());
        items += 1;
    }

    // User name
    if let Some(name) = &config.agents.defaults.user_name {
        debug!(user_name = name, "importing user name");
        identity.user_name = Some(name.clone());
        items += 1;
    }

    let report = if items > 0 {
        CategoryReport::success(ImportCategory::Identity, items)
    } else {
        CategoryReport::skipped(ImportCategory::Identity)
    };

    (report, identity)
}

fn load_config(path: &Path) -> OpenClawConfig {
    if !path.is_file() {
        return OpenClawConfig::default();
    }
    let Ok(content) = std::fs::read_to_string(path) else {
        return OpenClawConfig::default();
    };
    json5::from_str(&content).unwrap_or_default()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn make_detection(tmp: &Path) -> OpenClawDetection {
        OpenClawDetection {
            home_dir: tmp.to_path_buf(),
            has_config: true,
            has_credentials: false,
            has_mcp_servers: false,
            workspace_dir: tmp.join("workspace"),
            has_memory: false,
            has_skills: false,
            agent_ids: vec!["main".to_string()],
            session_count: 0,
            unsupported_channels: Vec::new(),
        }
    }

    #[test]
    fn import_agent_name_from_ui() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("openclaw.json"),
            r#"{"ui":{"assistant":{"name":"Claude"}}}"#,
        )
        .unwrap();

        let (report, identity) = import_identity(&make_detection(tmp.path()));
        assert_eq!(identity.agent_name.as_deref(), Some("Claude"));
        assert_eq!(report.items_imported, 1);
    }

    #[test]
    fn import_agent_name_from_agents_list() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("openclaw.json"),
            r#"{"agents":{"list":[{"id":"main","default":true,"name":"Rex"}]}}"#,
        )
        .unwrap();

        let (report, identity) = import_identity(&make_detection(tmp.path()));
        assert_eq!(identity.agent_name.as_deref(), Some("Rex"));
        assert_eq!(report.items_imported, 1);
    }

    #[test]
    fn import_timezone() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("openclaw.json"),
            r#"{"agents":{"defaults":{"userTimezone":"Europe/Paris"}}}"#,
        )
        .unwrap();

        let (_, identity) = import_identity(&make_detection(tmp.path()));
        assert_eq!(identity.user_timezone.as_deref(), Some("Europe/Paris"));
    }

    #[test]
    fn import_user_name() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("openclaw.json"),
            r#"{"agents":{"defaults":{"userName":"Alice"}}}"#,
        )
        .unwrap();

        let (_, identity) = import_identity(&make_detection(tmp.path()));
        assert_eq!(identity.user_name.as_deref(), Some("Alice"));
    }

    #[test]
    fn no_config_returns_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let (report, identity) = import_identity(&make_detection(tmp.path()));
        assert_eq!(report.status, crate::report::ImportStatus::Skipped);
        assert!(identity.agent_name.is_none());
    }
}
