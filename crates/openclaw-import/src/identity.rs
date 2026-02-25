//! Import user/agent identity from OpenClaw config.

use std::{collections::HashMap, path::Path};

use {serde::Deserialize, tracing::debug};

use crate::{
    detect::{OpenClawDetection, resolve_agent_sessions_dir},
    report::{CategoryReport, ImportCategory},
    types::{OpenClawAssistantConfig, OpenClawConfig},
};

/// Extracted identity from an OpenClaw installation.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ImportedIdentity {
    /// Agent display name (from `ui.assistant.name` or first agent's name).
    pub agent_name: Option<String>,
    /// Agent theme (from `ui.assistant.theme` or creature/vibe fields).
    pub theme: Option<String>,
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
    } else if let Some(name) = infer_agent_name_from_workspace(&config, detection) {
        debug!(name, "importing agent name from workspace basename");
        identity.agent_name = Some(name);
        items += 1;
    }

    if let Some(theme) = infer_theme(&config, detection) {
        debug!(theme = %theme, "importing agent theme");
        identity.theme = Some(theme);
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
    } else if let Some(name) = infer_user_name_from_sessions_index(detection) {
        debug!(user_name = name, "importing user name from sessions index");
        identity.user_name = Some(name);
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

fn infer_agent_name_from_workspace(
    config: &OpenClawConfig,
    detection: &OpenClawDetection,
) -> Option<String> {
    let workspace = config
        .agents
        .defaults
        .workspace
        .as_deref()
        .map(Path::new)
        .unwrap_or(detection.workspace_dir.as_path());
    let raw = workspace.file_name()?.to_str()?.trim();
    if raw.is_empty() || raw.eq_ignore_ascii_case("workspace") {
        return None;
    }
    Some(titleize_identifier(raw))
}

fn infer_theme(config: &OpenClawConfig, detection: &OpenClawDetection) -> Option<String> {
    if let Some(theme) = extract_theme_from_assistant(config.ui.assistant.as_ref()) {
        return Some(theme);
    }
    infer_theme_from_workspace_identity(config, detection)
}

fn extract_theme_from_assistant(assistant: Option<&OpenClawAssistantConfig>) -> Option<String> {
    let assistant = assistant?;
    if let Some(theme) = normalize_identity_value(assistant.theme.as_deref()) {
        return Some(theme);
    }

    compose_theme(
        normalize_identity_value(assistant.creature.as_deref()),
        normalize_identity_value(assistant.vibe.as_deref()),
    )
}

fn infer_theme_from_workspace_identity(
    config: &OpenClawConfig,
    detection: &OpenClawDetection,
) -> Option<String> {
    let workspace = config
        .agents
        .defaults
        .workspace
        .as_deref()
        .map(Path::new)
        .unwrap_or(detection.workspace_dir.as_path());
    let identity_path = workspace.join("IDENTITY.md");
    let content = std::fs::read_to_string(identity_path).ok()?;
    let frontmatter = extract_yaml_frontmatter(&content)?;
    let identity = parse_identity_frontmatter(frontmatter);

    if let Some(theme) = identity.theme {
        return Some(theme);
    }
    compose_theme(identity.creature, identity.vibe)
}

fn compose_theme(creature: Option<String>, vibe: Option<String>) -> Option<String> {
    match (vibe, creature) {
        (Some(vibe), Some(creature)) if vibe.eq_ignore_ascii_case(&creature) => Some(vibe),
        (Some(vibe), Some(creature)) => Some(format!("{vibe} {creature}")),
        (Some(vibe), None) => Some(vibe),
        (None, Some(creature)) => Some(creature),
        (None, None) => None,
    }
}

fn normalize_identity_value(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }
    Some(value.to_string())
}

fn extract_yaml_frontmatter(content: &str) -> Option<&str> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }
    let rest = trimmed.strip_prefix("---")?;
    let rest = rest.strip_prefix('\n')?;
    let end = rest.find("\n---")?;
    Some(&rest[..end])
}

#[derive(Default)]
struct WorkspaceIdentityFrontmatter {
    theme: Option<String>,
    creature: Option<String>,
    vibe: Option<String>,
}

fn parse_identity_frontmatter(frontmatter: &str) -> WorkspaceIdentityFrontmatter {
    let mut identity = WorkspaceIdentityFrontmatter::default();
    for raw in frontmatter.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let Some((raw_key, raw_value)) = line.split_once(':') else {
            continue;
        };
        let key = raw_key.trim();
        let value = unquote_yaml_scalar(raw_value.trim());
        let Some(value) = normalize_identity_value(Some(value)) else {
            continue;
        };

        match key {
            "theme" => identity.theme = Some(value),
            "creature" => identity.creature = Some(value),
            "vibe" => identity.vibe = Some(value),
            _ => {},
        }
    }
    identity
}

fn unquote_yaml_scalar(value: &str) -> &str {
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

fn infer_user_name_from_sessions_index(detection: &OpenClawDetection) -> Option<String> {
    let agent = preferred_agent_id(detection)?;
    let agent_dir = detection.home_dir.join("agents").join(agent);
    let sessions_dir = resolve_agent_sessions_dir(&agent_dir)?;
    let sessions_index_path = sessions_dir.join("sessions.json");
    let content = std::fs::read_to_string(sessions_index_path).ok()?;
    let entries: HashMap<String, SessionIndexEntry> = serde_json::from_str(&content).ok()?;

    let mut rows: Vec<&SessionIndexEntry> = entries.values().collect();
    rows.sort_by_key(|e| std::cmp::Reverse(e.updated_at.unwrap_or(0)));

    for row in rows.iter().copied() {
        if row.origin.as_ref().and_then(|o| o.chat_type.as_deref()) != Some("direct") {
            continue;
        }
        if let Some(label) = row.origin.as_ref().and_then(|o| o.label.as_deref())
            && let Some(name) = normalize_display_name(label)
        {
            return Some(name);
        }
    }

    // Fallback: any labeled origin.
    for row in rows {
        if let Some(label) = row.origin.as_ref().and_then(|o| o.label.as_deref())
            && let Some(name) = normalize_display_name(label)
        {
            return Some(name);
        }
    }

    None
}

fn preferred_agent_id(detection: &OpenClawDetection) -> Option<&str> {
    detection
        .agent_ids
        .iter()
        .find(|id| id.as_str() == "main")
        .or_else(|| detection.agent_ids.first())
        .map(String::as_str)
}

fn normalize_display_name(label: &str) -> Option<String> {
    let mut value = label.trim();
    if let Some((left, _)) = value.split_once("(@") {
        value = left;
    }
    if let Some((left, _)) = value.split_once(" id:") {
        value = left;
    }
    if let Some((left, _)) = value.split_once(" (") {
        value = left;
    }
    let trimmed = value.trim_matches(|c: char| c.is_whitespace() || c == '"' || c == '\'');
    if trimmed.is_empty() || trimmed.contains(':') {
        return None;
    }
    Some(trimmed.to_string())
}

fn titleize_identifier(raw: &str) -> String {
    raw.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    let mut out = String::new();
                    out.extend(first.to_uppercase());
                    out.push_str(chars.as_str());
                    out
                },
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Debug, Deserialize)]
struct SessionIndexEntry {
    #[serde(rename = "updatedAt")]
    updated_at: Option<u64>,
    origin: Option<SessionOrigin>,
}

#[derive(Debug, Deserialize)]
struct SessionOrigin {
    #[serde(rename = "chatType")]
    chat_type: Option<String>,
    label: Option<String>,
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
    fn import_theme_from_ui_assistant_theme() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("openclaw.json"),
            r#"{"ui":{"assistant":{"theme":"helpful otter"}}}"#,
        )
        .unwrap();

        let (_, identity) = import_identity(&make_detection(tmp.path()));
        assert_eq!(identity.theme.as_deref(), Some("helpful otter"));
    }

    #[test]
    fn import_theme_from_ui_assistant_creature_and_vibe() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("openclaw.json"),
            r#"{"ui":{"assistant":{"creature":"otter","vibe":"helpful"}}}"#,
        )
        .unwrap();

        let (_, identity) = import_identity(&make_detection(tmp.path()));
        assert_eq!(identity.theme.as_deref(), Some("helpful otter"));
    }

    #[test]
    fn import_theme_from_workspace_identity_frontmatter() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace_dir = tmp.path().join("workspace");
        std::fs::create_dir_all(&workspace_dir).unwrap();
        std::fs::write(
            workspace_dir.join("IDENTITY.md"),
            "---\ncreature: fox\nvibe: calm\n---\n\n# IDENTITY\n",
        )
        .unwrap();

        let (_, identity) = import_identity(&make_detection(tmp.path()));
        assert_eq!(identity.theme.as_deref(), Some("calm fox"));
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
    fn import_user_name_from_sessions_index() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("agents").join("main").join("sessions")).unwrap();
        std::fs::write(
            tmp.path()
                .join("agents")
                .join("main")
                .join("sessions")
                .join("sessions.json"),
            r#"{
              "agent:main:main": {
                "updatedAt": 1770079095530,
                "origin": {
                  "chatType": "direct",
                  "label": "Fabien (@fabienpenso) id:377114917"
                }
              }
            }"#,
        )
        .unwrap();

        let (_, identity) = import_identity(&make_detection(tmp.path()));
        assert_eq!(identity.user_name.as_deref(), Some("Fabien"));
    }

    #[test]
    fn import_agent_name_from_workspace_basename() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("openclaw.json"),
            r#"{"agents":{"defaults":{"workspace":"/root/clawd"}}}"#,
        )
        .unwrap();

        let (_, identity) = import_identity(&make_detection(tmp.path()));
        assert_eq!(identity.agent_name.as_deref(), Some("Clawd"));
    }

    #[test]
    fn no_config_returns_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let (report, identity) = import_identity(&make_detection(tmp.path()));
        assert_eq!(report.status, crate::report::ImportStatus::Skipped);
        assert!(identity.agent_name.is_none());
    }
}
