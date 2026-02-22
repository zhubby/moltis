//! Import session JSONL files from OpenClaw to Moltis format.
//!
//! OpenClaw sessions live at `~/.openclaw/agents/<id>/sessions/<key>.jsonl`.
//! Moltis sessions live at `<data_dir>/sessions/<key>.jsonl` with metadata in
//! `session-metadata.json`.

use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
};

use {
    serde::{Deserialize, Serialize},
    tracing::{debug, warn},
};

use crate::{
    detect::OpenClawDetection,
    report::{CategoryReport, ImportCategory, ImportStatus},
    types::{OpenClawContent, OpenClawRole, OpenClawSessionRecord},
};

/// Minimal session metadata for the Moltis `session-metadata.json` index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedSessionEntry {
    pub id: String,
    pub key: String,
    pub label: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub message_count: u32,
    #[serde(default)]
    pub last_seen_message_count: u32,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub version: u64,
}

/// A converted Moltis message (matches `PersistedMessage` serde format).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "role", rename_all = "lowercase")]
enum MoltisMessage {
    System {
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        created_at: Option<u64>,
    },
    User {
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        created_at: Option<u64>,
    },
    Assistant {
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        created_at: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        provider: Option<String>,
    },
    Tool {
        tool_call_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        created_at: Option<u64>,
    },
}

/// Import sessions from all agents in an OpenClaw installation.
pub fn import_sessions(detection: &OpenClawDetection, dest_sessions_dir: &Path) -> CategoryReport {
    if detection.agent_ids.is_empty() {
        return CategoryReport::skipped(ImportCategory::Sessions);
    }

    let mut imported = 0;
    let mut skipped = 0;
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut entries = Vec::new();

    // Only import the default or first agent. Add TODO for others.
    let (import_agent, other_agents) = if detection.agent_ids.len() > 1 {
        let default_idx = detection
            .agent_ids
            .iter()
            .position(|id| id == "main")
            .unwrap_or(0);
        let import = &detection.agent_ids[default_idx];
        let others: Vec<&str> = detection
            .agent_ids
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != default_idx)
            .map(|(_, id)| id.as_str())
            .collect();
        (import.as_str(), others)
    } else {
        (detection.agent_ids[0].as_str(), Vec::new())
    };

    if !other_agents.is_empty() {
        warnings.push(format!(
            "only importing sessions from agent '{}'; skipping agents: {}",
            import_agent,
            other_agents.join(", ")
        ));
    }

    let sessions_dir = detection
        .home_dir
        .join("agents")
        .join(import_agent)
        .join("agent")
        .join("sessions");

    if !sessions_dir.is_dir() {
        return CategoryReport::skipped(ImportCategory::Sessions);
    }

    let Ok(dir_entries) = std::fs::read_dir(&sessions_dir) else {
        return CategoryReport::failed(
            ImportCategory::Sessions,
            "failed to read sessions directory".to_string(),
        );
    };

    if let Err(e) = std::fs::create_dir_all(dest_sessions_dir) {
        return CategoryReport::failed(
            ImportCategory::Sessions,
            format!("failed to create destination directory: {e}"),
        );
    }

    for entry in dir_entries.flatten() {
        let path = entry.path();
        if !path.is_file() || path.extension().is_some_and(|e| e != "jsonl") {
            continue;
        }

        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        let dest_key = format!("oc:{import_agent}:{stem}");
        let dest_file = dest_sessions_dir.join(format!("{dest_key}.jsonl"));

        // Skip if already imported (idempotency)
        if dest_file.exists() {
            debug!(key = %dest_key, "session already exists, skipping");
            skipped += 1;
            continue;
        }

        match convert_session(&path, &dest_file) {
            Ok(stats) => {
                debug!(
                    key = %dest_key,
                    messages = stats.message_count,
                    "imported session"
                );
                entries.push(ImportedSessionEntry {
                    id: uuid_v4(),
                    key: dest_key,
                    label: Some(format!("OpenClaw: {stem}")),
                    model: stats.last_model,
                    created_at: stats.first_timestamp.unwrap_or_else(now_ms),
                    updated_at: stats.last_timestamp.unwrap_or_else(now_ms),
                    message_count: stats.message_count,
                    last_seen_message_count: 0,
                    archived: false,
                    version: 0,
                });
                imported += 1;
            },
            Err(e) => {
                warn!(source = %path.display(), error = %e, "failed to convert session");
                errors.push(format!("failed to convert {}: {e}", path.display()));
            },
        }
    }

    // Write/merge session metadata
    if !entries.is_empty() {
        let metadata_path = dest_sessions_dir.join("session-metadata.json");
        if let Err(e) = merge_session_metadata(&metadata_path, &entries) {
            errors.push(format!("failed to update session-metadata.json: {e}"));
        }
    }

    let status = if !errors.is_empty() && imported > 0 {
        ImportStatus::Partial
    } else if !errors.is_empty() {
        ImportStatus::Failed
    } else if imported == 0 {
        ImportStatus::Skipped
    } else {
        ImportStatus::Success
    };

    CategoryReport {
        category: ImportCategory::Sessions,
        status,
        items_imported: imported,
        items_skipped: skipped,
        warnings,
        errors,
    }
}

struct ConvertStats {
    message_count: u32,
    first_timestamp: Option<u64>,
    last_timestamp: Option<u64>,
    last_model: Option<String>,
}

fn convert_session(src: &Path, dest: &Path) -> anyhow::Result<ConvertStats> {
    let file = std::fs::File::open(src)?;
    let reader = BufReader::new(file);

    let mut dest_file = std::fs::File::create(dest)?;
    let mut stats = ConvertStats {
        message_count: 0,
        first_timestamp: None,
        last_timestamp: None,
        last_model: None,
    };

    let now = now_ms();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let record: OpenClawSessionRecord = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(_) => continue, // Skip malformed lines
        };

        let converted = match record {
            OpenClawSessionRecord::Message { message } => {
                convert_message(&message, now, &mut stats)
            },
            // Skip session-meta and custom records
            _ => None,
        };

        if let Some(msg) = converted {
            let json = serde_json::to_string(&msg)?;
            writeln!(dest_file, "{json}")?;
            stats.message_count += 1;
        }
    }

    Ok(stats)
}

fn convert_message(
    msg: &crate::types::OpenClawMessage,
    now: u64,
    stats: &mut ConvertStats,
) -> Option<MoltisMessage> {
    let content = msg.content.as_ref().map(OpenClawContent::as_text)?;
    if content.is_empty() {
        return None;
    }

    if stats.first_timestamp.is_none() {
        stats.first_timestamp = Some(now);
    }
    stats.last_timestamp = Some(now);

    match msg.role {
        OpenClawRole::System => Some(MoltisMessage::System {
            content,
            created_at: Some(now),
        }),
        OpenClawRole::User => Some(MoltisMessage::User {
            content,
            created_at: Some(now),
        }),
        OpenClawRole::Assistant => Some(MoltisMessage::Assistant {
            content,
            created_at: Some(now),
            model: None,
            provider: None,
        }),
        OpenClawRole::Tool | OpenClawRole::ToolResult => {
            let tool_call_id = msg.tool_use_id.clone().unwrap_or_default();
            Some(MoltisMessage::Tool {
                tool_call_id,
                content,
                created_at: Some(now),
            })
        },
    }
}

fn merge_session_metadata(path: &Path, new_entries: &[ImportedSessionEntry]) -> anyhow::Result<()> {
    use std::collections::HashMap;

    let mut existing: HashMap<String, ImportedSessionEntry> = if path.is_file() {
        let content = std::fs::read_to_string(path)?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        HashMap::new()
    };

    for entry in new_entries {
        existing
            .entry(entry.key.clone())
            .or_insert_with(|| entry.clone());
    }

    let json = serde_json::to_string_pretty(&existing)?;
    std::fs::write(path, json)?;
    Ok(())
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn uuid_v4() -> String {
    // Simple UUID v4 without pulling in the uuid crate.
    // Format: 8-4-4-4-12 hex characters.
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        (seed >> 96) as u32,
        (seed >> 80) as u16,
        (seed >> 64) as u16 & 0x0FFF,
        ((seed >> 48) as u16 & 0x3FFF) | 0x8000,
        seed as u64 & 0xFFFF_FFFF_FFFF,
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn make_detection(home: &Path) -> OpenClawDetection {
        OpenClawDetection {
            home_dir: home.to_path_buf(),
            has_config: false,
            has_credentials: false,
            has_mcp_servers: false,
            workspace_dir: home.join("workspace"),
            has_memory: false,
            has_skills: false,
            agent_ids: vec!["main".to_string()],
            session_count: 1,
            unsupported_channels: Vec::new(),
        }
    }

    fn setup_session(home: &Path, agent: &str, key: &str, lines: &[&str]) {
        let dir = home
            .join("agents")
            .join(agent)
            .join("agent")
            .join("sessions");
        std::fs::create_dir_all(&dir).unwrap();
        let content = lines.join("\n");
        std::fs::write(dir.join(format!("{key}.jsonl")), content).unwrap();
    }

    #[test]
    fn convert_basic_session() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let dest = tmp.path().join("sessions");

        setup_session(home, "main", "test-session", &[
            r#"{"type":"session-meta","agentId":"main"}"#,
            r#"{"type":"message","message":{"role":"user","content":"Hello"}}"#,
            r#"{"type":"message","message":{"role":"assistant","content":"Hi there!"}}"#,
            r#"{"type":"custom","customType":"model-snapshot","data":{}}"#,
        ]);

        let detection = make_detection(home);
        let report = import_sessions(&detection, &dest);

        assert_eq!(report.status, ImportStatus::Success);
        assert_eq!(report.items_imported, 1);

        // Verify converted JSONL
        let converted_path = dest.join("oc:main:test-session.jsonl");
        assert!(converted_path.is_file());

        let content = std::fs::read_to_string(&converted_path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2); // Only message records, not meta/custom

        let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first["role"], "user");
        assert_eq!(first["content"], "Hello");

        let second: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(second["role"], "assistant");
        assert_eq!(second["content"], "Hi there!");
    }

    #[test]
    fn convert_tool_messages() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let dest = tmp.path().join("sessions");

        setup_session(home, "main", "tools", &[
            r#"{"type":"message","message":{"role":"user","content":"Run ls"}}"#,
            r#"{"type":"message","message":{"role":"tool","content":"file.txt","toolUseId":"call_1"}}"#,
        ]);

        let detection = make_detection(home);
        let report = import_sessions(&detection, &dest);

        assert_eq!(report.items_imported, 1);

        let content = std::fs::read_to_string(dest.join("oc:main:tools.jsonl")).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);

        let tool: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(tool["role"], "tool");
        assert_eq!(tool["tool_call_id"], "call_1");
    }

    #[test]
    fn import_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let dest = tmp.path().join("sessions");

        setup_session(home, "main", "existing", &[
            r#"{"type":"message","message":{"role":"user","content":"test"}}"#,
        ]);

        // First import
        let detection = make_detection(home);
        let report1 = import_sessions(&detection, &dest);
        assert_eq!(report1.items_imported, 1);

        // Second import — should skip
        let report2 = import_sessions(&detection, &dest);
        assert_eq!(report2.items_imported, 0);
        assert_eq!(report2.items_skipped, 1);
    }

    #[test]
    fn no_agents_returns_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let detection = OpenClawDetection {
            home_dir: tmp.path().to_path_buf(),
            has_config: false,
            has_credentials: false,
            has_mcp_servers: false,
            workspace_dir: tmp.path().join("workspace"),
            has_memory: false,
            has_skills: false,
            agent_ids: Vec::new(),
            session_count: 0,
            unsupported_channels: Vec::new(),
        };

        let report = import_sessions(&detection, &tmp.path().join("dest"));
        assert_eq!(report.status, ImportStatus::Skipped);
    }

    #[test]
    fn multi_agent_warns() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let dest = tmp.path().join("sessions");

        setup_session(home, "main", "s1", &[
            r#"{"type":"message","message":{"role":"user","content":"hi"}}"#,
        ]);
        // Create second agent dir (empty sessions)
        std::fs::create_dir_all(
            home.join("agents")
                .join("secondary")
                .join("agent")
                .join("sessions"),
        )
        .unwrap();

        let mut detection = make_detection(home);
        detection.agent_ids = vec!["main".to_string(), "secondary".to_string()];

        let report = import_sessions(&detection, &dest);
        assert!(report.warnings.iter().any(|w| w.contains("secondary")));
    }

    #[test]
    fn session_metadata_written() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let dest = tmp.path().join("sessions");

        setup_session(home, "main", "meta-test", &[
            r#"{"type":"message","message":{"role":"user","content":"hello"}}"#,
        ]);

        let detection = make_detection(home);
        import_sessions(&detection, &dest);

        let metadata_path = dest.join("session-metadata.json");
        assert!(metadata_path.is_file());

        let content = std::fs::read_to_string(&metadata_path).unwrap();
        let metadata: std::collections::HashMap<String, serde_json::Value> =
            serde_json::from_str(&content).unwrap();
        assert!(metadata.contains_key("oc:main:meta-test"));
    }

    #[test]
    fn malformed_lines_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let dest = tmp.path().join("sessions");

        setup_session(home, "main", "messy", &[
            r#"not valid json"#,
            r#"{"type":"message","message":{"role":"user","content":"valid"}}"#,
            r#"{"broken":true}"#,
        ]);

        let detection = make_detection(home);
        let report = import_sessions(&detection, &dest);

        assert_eq!(report.items_imported, 1);
        let content = std::fs::read_to_string(dest.join("oc:main:messy.jsonl")).unwrap();
        assert_eq!(content.lines().count(), 1);
    }
}
