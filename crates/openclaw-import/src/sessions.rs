//! Import session JSONL files from OpenClaw to Moltis format.
//!
//! OpenClaw sessions live at either:
//! - `~/.openclaw/agents/<id>/sessions/<key>.jsonl` (legacy layout), or
//! - `~/.openclaw/agents/<id>/agent/sessions/<key>.jsonl` (newer layout).
//!
//! Moltis sessions live at `<data_dir>/sessions/<key>.jsonl` with metadata in
//! `session-metadata.json`.

use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    path::Path,
};

use {
    serde::{Deserialize, Serialize},
    tracing::{debug, warn},
};

use crate::{
    detect::{OpenClawDetection, resolve_agent_sessions_dir},
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
    pub source_line_count: u32,
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
///
/// In addition to converting JSONL files, this also generates markdown
/// transcripts in `memory_sessions_dir` (typically `<data>/memory/sessions/`)
/// so that imported conversations are searchable by the Moltis memory system.
pub fn import_sessions(
    detection: &OpenClawDetection,
    dest_sessions_dir: &Path,
    memory_sessions_dir: &Path,
) -> CategoryReport {
    if detection.agent_ids.is_empty() {
        return CategoryReport::skipped(ImportCategory::Sessions);
    }

    let mut imported = 0;
    let mut updated = 0;
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

    let agent_dir = detection.home_dir.join("agents").join(import_agent);
    let Some(sessions_dir) = resolve_agent_sessions_dir(&agent_dir) else {
        return CategoryReport::skipped(ImportCategory::Sessions);
    };

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

    // Load existing metadata to detect incremental changes
    let metadata_path = dest_sessions_dir.join("session-metadata.json");
    let existing_metadata = load_session_metadata(&metadata_path);

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

        let source_lines = count_lines(&path);

        // Check if we already have this session and whether it has grown
        let existing_entry = existing_metadata.get(&dest_key);
        let is_update = if let Some(prev) = existing_entry {
            // source_line_count == 0 means legacy metadata (pre-incremental), always re-import
            if prev.source_line_count > 0 && source_lines <= prev.source_line_count {
                debug!(key = %dest_key, "session unchanged, skipping");
                skipped += 1;
                continue;
            }
            true
        } else {
            false
        };

        match convert_session(&path, &dest_file) {
            Ok(stats) => {
                let label = format!("OpenClaw: {stem}");

                if is_update {
                    debug!(
                        key = %dest_key,
                        messages = stats.message_count,
                        "updated session (incremental)"
                    );
                } else {
                    debug!(
                        key = %dest_key,
                        messages = stats.message_count,
                        "imported session"
                    );
                }

                // Write/overwrite markdown transcript for memory search indexing
                if !stats.transcript.is_empty()
                    && let Err(e) = write_transcript(
                        memory_sessions_dir,
                        &dest_key,
                        &label,
                        stats.last_model.as_deref(),
                        &stats,
                    )
                {
                    warn!(key = %dest_key, error = %e, "failed to write session transcript");
                }

                // Preserve original id/created_at on update, bump version
                let (id, created_at, version) = if let Some(prev) = existing_entry {
                    (
                        prev.id.clone(),
                        prev.created_at,
                        prev.version.saturating_add(1),
                    )
                } else {
                    (uuid_v4(), stats.first_timestamp.unwrap_or_else(now_ms), 0)
                };

                entries.push(ImportedSessionEntry {
                    id,
                    key: dest_key,
                    label: Some(label),
                    model: stats.last_model,
                    created_at,
                    updated_at: stats.last_timestamp.unwrap_or_else(now_ms),
                    message_count: stats.message_count,
                    last_seen_message_count: stats.message_count,
                    source_line_count: source_lines,
                    archived: false,
                    version,
                });
                if is_update {
                    updated += 1;
                } else {
                    imported += 1;
                }
            },
            Err(e) => {
                warn!(source = %path.display(), error = %e, "failed to convert session");
                errors.push(format!("failed to convert {}: {e}", path.display()));
            },
        }
    }

    // Write/merge session metadata
    if !entries.is_empty()
        && let Err(e) = merge_session_metadata(&metadata_path, &entries)
    {
        errors.push(format!("failed to update session-metadata.json: {e}"));
    }

    let total_changed = imported + updated;
    let status = if !errors.is_empty() && total_changed > 0 {
        ImportStatus::Partial
    } else if !errors.is_empty() {
        ImportStatus::Failed
    } else if total_changed == 0 {
        ImportStatus::Skipped
    } else {
        ImportStatus::Success
    };

    CategoryReport {
        category: ImportCategory::Sessions,
        status,
        items_imported: imported,
        items_updated: updated,
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
    /// Collected transcript entries for markdown export.
    transcript: Vec<TranscriptEntry>,
}

/// A single entry in a session transcript (for markdown export).
struct TranscriptEntry {
    role: &'static str,
    content: String,
}

fn convert_session(src: &Path, dest: &Path) -> crate::error::Result<ConvertStats> {
    let file = std::fs::File::open(src)?;
    let reader = BufReader::new(file);

    let mut dest_file = std::fs::File::create(dest)?;
    let mut stats = ConvertStats {
        message_count: 0,
        first_timestamp: None,
        last_timestamp: None,
        last_model: None,
        transcript: Vec::new(),
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

        match record {
            OpenClawSessionRecord::Message { message } => {
                if let Some(msg) = convert_message(&message, now, &mut stats) {
                    let json = serde_json::to_string(&msg)?;
                    writeln!(dest_file, "{json}")?;
                    stats.message_count += 1;
                }
            },
            OpenClawSessionRecord::Custom { custom_type, data } => {
                // Extract model name from model-snapshot records
                if custom_type.as_deref() == Some("model-snapshot")
                    && let Some(model) = data
                        .as_ref()
                        .and_then(|d| d.get("model"))
                        .and_then(|m| m.as_str())
                {
                    stats.last_model = Some(model.to_string());
                }
            },
            OpenClawSessionRecord::SessionMeta { .. } => {
                // Session metadata is used for detection/scan only
            },
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

    // Collect user/assistant messages for the markdown transcript
    let role_label = match msg.role {
        OpenClawRole::User => Some("User"),
        OpenClawRole::Assistant => Some("Assistant"),
        _ => None,
    };
    if let Some(label) = role_label {
        stats.transcript.push(TranscriptEntry {
            role: label,
            content: content.clone(),
        });
    }

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

/// Count lines in a file without parsing content.
fn count_lines(path: &Path) -> u32 {
    let Ok(file) = std::fs::File::open(path) else {
        return 0;
    };
    BufReader::new(file).lines().count() as u32
}

/// Load existing session metadata from disk.
fn load_session_metadata(path: &Path) -> HashMap<String, ImportedSessionEntry> {
    if !path.is_file() {
        return HashMap::new();
    }
    let Ok(content) = std::fs::read_to_string(path) else {
        return HashMap::new();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

fn merge_session_metadata(
    path: &Path,
    new_entries: &[ImportedSessionEntry],
) -> crate::error::Result<()> {
    let mut existing = load_session_metadata(path);

    for entry in new_entries {
        existing.insert(entry.key.clone(), entry.clone());
    }

    let json = serde_json::to_string_pretty(&existing)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Write a markdown transcript of a session for memory search indexing.
///
/// The file is placed in `memory/sessions/` and includes all user/assistant
/// messages so they become searchable by the Moltis memory system.
fn write_transcript(
    dir: &Path,
    dest_key: &str,
    label: &str,
    model: Option<&str>,
    stats: &ConvertStats,
) -> crate::error::Result<()> {
    std::fs::create_dir_all(dir)?;

    // Use hyphens instead of colons for filesystem safety
    let safe_name = dest_key.replace(':', "-");
    let path = dir.join(format!("{safe_name}.md"));

    let mut content = format!("# Session: {label}\n\n");
    content.push_str("*Imported from OpenClaw*");
    if let Some(m) = model {
        content.push_str(&format!(" | Model: {m}"));
    }
    content.push_str(&format!(" | Messages: {}\n\n---\n\n", stats.message_count));

    for entry in &stats.transcript {
        content.push_str(&format!("**{}:** {}\n\n", entry.role, entry.content));
    }

    std::fs::write(path, content)?;
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

    fn setup_session_legacy_layout(home: &Path, agent: &str, key: &str, lines: &[&str]) {
        let dir = home.join("agents").join(agent).join("sessions");
        std::fs::create_dir_all(&dir).unwrap();
        let content = lines.join("\n");
        std::fs::write(dir.join(format!("{key}.jsonl")), content).unwrap();
    }

    #[test]
    fn convert_basic_session() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let dest = tmp.path().join("sessions");
        let mem = tmp.path().join("memory").join("sessions");

        setup_session(home, "main", "test-session", &[
            r#"{"type":"session-meta","agentId":"main"}"#,
            r#"{"type":"message","message":{"role":"user","content":"Hello"}}"#,
            r#"{"type":"message","message":{"role":"assistant","content":"Hi there!"}}"#,
            r#"{"type":"custom","customType":"model-snapshot","data":{}}"#,
        ]);

        let detection = make_detection(home);
        let report = import_sessions(&detection, &dest, &mem);

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
    fn convert_basic_session_legacy_layout() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let dest = tmp.path().join("sessions");
        let mem = tmp.path().join("memory").join("sessions");

        setup_session_legacy_layout(home, "main", "legacy-session", &[
            r#"{"type":"session-meta","agentId":"main"}"#,
            r#"{"type":"message","message":{"role":"user","content":"Hello"}}"#,
            r#"{"type":"message","message":{"role":"assistant","content":"Hi there!"}}"#,
        ]);

        let detection = make_detection(home);
        let report = import_sessions(&detection, &dest, &mem);

        assert_eq!(report.status, ImportStatus::Success);
        assert_eq!(report.items_imported, 1);
        assert!(dest.join("oc:main:legacy-session.jsonl").is_file());
    }

    #[test]
    fn convert_tool_messages() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let dest = tmp.path().join("sessions");
        let mem = tmp.path().join("memory").join("sessions");

        setup_session(home, "main", "tools", &[
            r#"{"type":"message","message":{"role":"user","content":"Run ls"}}"#,
            r#"{"type":"message","message":{"role":"tool","content":"file.txt","toolUseId":"call_1"}}"#,
        ]);

        let detection = make_detection(home);
        let report = import_sessions(&detection, &dest, &mem);

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
        let mem = tmp.path().join("memory").join("sessions");

        setup_session(home, "main", "existing", &[
            r#"{"type":"message","message":{"role":"user","content":"test"}}"#,
        ]);

        // First import
        let detection = make_detection(home);
        let report1 = import_sessions(&detection, &dest, &mem);
        assert_eq!(report1.items_imported, 1);

        // Second import — should skip
        let report2 = import_sessions(&detection, &dest, &mem);
        assert_eq!(report2.items_imported, 0);
        assert_eq!(report2.items_skipped, 1);
    }

    #[test]
    fn no_agents_returns_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let mem = tmp.path().join("memory").join("sessions");
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

        let report = import_sessions(&detection, &tmp.path().join("dest"), &mem);
        assert_eq!(report.status, ImportStatus::Skipped);
    }

    #[test]
    fn multi_agent_warns() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let dest = tmp.path().join("sessions");
        let mem = tmp.path().join("memory").join("sessions");

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

        let report = import_sessions(&detection, &dest, &mem);
        assert!(report.warnings.iter().any(|w| w.contains("secondary")));
    }

    #[test]
    fn session_metadata_written() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let dest = tmp.path().join("sessions");
        let mem = tmp.path().join("memory").join("sessions");

        setup_session(home, "main", "meta-test", &[
            r#"{"type":"message","message":{"role":"user","content":"hello"}}"#,
        ]);

        let detection = make_detection(home);
        import_sessions(&detection, &dest, &mem);

        let metadata_path = dest.join("session-metadata.json");
        assert!(metadata_path.is_file());

        let content = std::fs::read_to_string(&metadata_path).unwrap();
        let metadata: HashMap<String, serde_json::Value> = serde_json::from_str(&content).unwrap();
        assert!(metadata.contains_key("oc:main:meta-test"));
    }

    #[test]
    fn malformed_lines_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let dest = tmp.path().join("sessions");
        let mem = tmp.path().join("memory").join("sessions");

        setup_session(home, "main", "messy", &[
            r#"not valid json"#,
            r#"{"type":"message","message":{"role":"user","content":"valid"}}"#,
            r#"{"broken":true}"#,
        ]);

        let detection = make_detection(home);
        let report = import_sessions(&detection, &dest, &mem);

        assert_eq!(report.items_imported, 1);
        let content = std::fs::read_to_string(dest.join("oc:main:messy.jsonl")).unwrap();
        assert_eq!(content.lines().count(), 1);
    }

    #[test]
    fn session_transcript_written() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let dest = tmp.path().join("sessions");
        let mem = tmp.path().join("memory").join("sessions");

        setup_session(home, "main", "chat", &[
            r#"{"type":"session-meta","agentId":"main"}"#,
            r#"{"type":"message","message":{"role":"user","content":"What is Rust?"}}"#,
            r#"{"type":"message","message":{"role":"assistant","content":"Rust is a systems programming language."}}"#,
            r#"{"type":"custom","customType":"model-snapshot","data":{"model":"claude-opus-4-6"}}"#,
        ]);

        let detection = make_detection(home);
        let report = import_sessions(&detection, &dest, &mem);

        assert_eq!(report.items_imported, 1);

        // Verify transcript markdown was written
        let transcript_path = mem.join("oc-main-chat.md");
        assert!(transcript_path.is_file());

        let content = std::fs::read_to_string(&transcript_path).unwrap();
        assert!(content.contains("# Session: OpenClaw: chat"));
        assert!(content.contains("Imported from OpenClaw"));
        assert!(content.contains("Model: claude-opus-4-6"));
        assert!(content.contains("**User:** What is Rust?"));
        assert!(content.contains("**Assistant:** Rust is a systems programming language."));
    }

    #[test]
    fn model_extracted_from_custom_record() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let dest = tmp.path().join("sessions");
        let mem = tmp.path().join("memory").join("sessions");

        setup_session(home, "main", "model-test", &[
            r#"{"type":"custom","customType":"model-snapshot","data":{"model":"gpt-4o"}}"#,
            r#"{"type":"message","message":{"role":"user","content":"test"}}"#,
        ]);

        let detection = make_detection(home);
        import_sessions(&detection, &dest, &mem);

        let metadata_path = dest.join("session-metadata.json");
        let content = std::fs::read_to_string(&metadata_path).unwrap();
        let metadata: HashMap<String, serde_json::Value> = serde_json::from_str(&content).unwrap();
        let entry = metadata.get("oc:main:model-test").unwrap();
        assert_eq!(entry["model"], "gpt-4o");
    }

    #[test]
    fn incremental_import_detects_growth() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let dest = tmp.path().join("sessions");
        let mem = tmp.path().join("memory").join("sessions");

        // Initial import with 1 message
        setup_session(home, "main", "growing", &[
            r#"{"type":"message","message":{"role":"user","content":"Hello"}}"#,
        ]);

        let detection = make_detection(home);
        let report1 = import_sessions(&detection, &dest, &mem);
        assert_eq!(report1.items_imported, 1);
        assert_eq!(report1.items_updated, 0);

        let content1 = std::fs::read_to_string(dest.join("oc:main:growing.jsonl")).unwrap();
        assert_eq!(content1.lines().count(), 1);

        // Append a new message to the source
        setup_session(home, "main", "growing", &[
            r#"{"type":"message","message":{"role":"user","content":"Hello"}}"#,
            r#"{"type":"message","message":{"role":"assistant","content":"Hi there!"}}"#,
        ]);

        // Re-import should detect growth and update
        let report2 = import_sessions(&detection, &dest, &mem);
        assert_eq!(report2.items_imported, 0);
        assert_eq!(report2.items_updated, 1);
        assert_eq!(report2.items_skipped, 0);

        // Destination should now have 2 messages
        let content2 = std::fs::read_to_string(dest.join("oc:main:growing.jsonl")).unwrap();
        assert_eq!(content2.lines().count(), 2);
    }

    #[test]
    fn incremental_import_noop_when_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let dest = tmp.path().join("sessions");
        let mem = tmp.path().join("memory").join("sessions");

        setup_session(home, "main", "stable", &[
            r#"{"type":"message","message":{"role":"user","content":"test"}}"#,
        ]);

        let detection = make_detection(home);
        let report1 = import_sessions(&detection, &dest, &mem);
        assert_eq!(report1.items_imported, 1);

        // Re-import without changes — should skip
        let report2 = import_sessions(&detection, &dest, &mem);
        assert_eq!(report2.items_imported, 0);
        assert_eq!(report2.items_updated, 0);
        assert_eq!(report2.items_skipped, 1);
    }

    #[test]
    fn incremental_import_preserves_id_and_created_at() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let dest = tmp.path().join("sessions");
        let mem = tmp.path().join("memory").join("sessions");

        setup_session(home, "main", "preserve", &[
            r#"{"type":"message","message":{"role":"user","content":"first"}}"#,
        ]);

        let detection = make_detection(home);
        import_sessions(&detection, &dest, &mem);

        // Read original metadata
        let metadata_path = dest.join("session-metadata.json");
        let content = std::fs::read_to_string(&metadata_path).unwrap();
        let metadata: HashMap<String, ImportedSessionEntry> =
            serde_json::from_str(&content).unwrap();
        let original = metadata.get("oc:main:preserve").unwrap();
        let original_id = original.id.clone();
        let original_created_at = original.created_at;
        assert_eq!(original.version, 0);

        // Append and re-import
        setup_session(home, "main", "preserve", &[
            r#"{"type":"message","message":{"role":"user","content":"first"}}"#,
            r#"{"type":"message","message":{"role":"assistant","content":"second"}}"#,
        ]);

        import_sessions(&detection, &dest, &mem);

        let content2 = std::fs::read_to_string(&metadata_path).unwrap();
        let metadata2: HashMap<String, ImportedSessionEntry> =
            serde_json::from_str(&content2).unwrap();
        let updated = metadata2.get("oc:main:preserve").unwrap();

        assert_eq!(updated.id, original_id);
        assert_eq!(updated.created_at, original_created_at);
        assert_eq!(updated.version, 1);
        assert_eq!(updated.message_count, 2);
    }

    #[test]
    fn incremental_import_regenerates_transcript() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let dest = tmp.path().join("sessions");
        let mem = tmp.path().join("memory").join("sessions");

        setup_session(home, "main", "transcript", &[
            r#"{"type":"message","message":{"role":"user","content":"What is Rust?"}}"#,
        ]);

        let detection = make_detection(home);
        import_sessions(&detection, &dest, &mem);

        let transcript_path = mem.join("oc-main-transcript.md");
        let content1 = std::fs::read_to_string(&transcript_path).unwrap();
        assert!(content1.contains("**User:** What is Rust?"));
        assert!(!content1.contains("systems programming language"));

        // Append response and re-import
        setup_session(home, "main", "transcript", &[
            r#"{"type":"message","message":{"role":"user","content":"What is Rust?"}}"#,
            r#"{"type":"message","message":{"role":"assistant","content":"A systems programming language."}}"#,
        ]);

        import_sessions(&detection, &dest, &mem);

        let content2 = std::fs::read_to_string(&transcript_path).unwrap();
        assert!(content2.contains("**User:** What is Rust?"));
        assert!(content2.contains("**Assistant:** A systems programming language."));
    }

    #[test]
    fn incremental_import_upgrades_legacy_metadata() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let dest = tmp.path().join("sessions");
        let mem = tmp.path().join("memory").join("sessions");
        std::fs::create_dir_all(&dest).unwrap();

        setup_session(home, "main", "legacy", &[
            r#"{"type":"message","message":{"role":"user","content":"old message"}}"#,
        ]);

        // Write legacy metadata without source_line_count (will deserialize as 0)
        let legacy_metadata = serde_json::json!({
            "oc:main:legacy": {
                "id": "legacy-id-123",
                "key": "oc:main:legacy",
                "label": "OpenClaw: legacy",
                "model": null,
                "created_at": 1000,
                "updated_at": 1000,
                "message_count": 1,
                "last_seen_message_count": 0,
                "archived": false,
                "version": 0
            }
        });
        std::fs::write(
            dest.join("session-metadata.json"),
            serde_json::to_string_pretty(&legacy_metadata).unwrap(),
        )
        .unwrap();

        // Also write a destination JSONL so it looks like a previous import happened
        std::fs::write(
            dest.join("oc:main:legacy.jsonl"),
            r#"{"role":"user","content":"old message"}"#,
        )
        .unwrap();

        // Re-import should detect legacy (source_line_count == 0) and re-import
        let detection = make_detection(home);
        let report = import_sessions(&detection, &dest, &mem);
        assert_eq!(report.items_updated, 1);
        assert_eq!(report.items_imported, 0);

        // Verify metadata now has source_line_count set
        let metadata_path = dest.join("session-metadata.json");
        let content = std::fs::read_to_string(&metadata_path).unwrap();
        let metadata: HashMap<String, ImportedSessionEntry> =
            serde_json::from_str(&content).unwrap();
        let entry = metadata.get("oc:main:legacy").unwrap();
        assert!(entry.source_line_count > 0);
        assert_eq!(entry.id, "legacy-id-123"); // Preserved
        assert_eq!(entry.version, 1); // Bumped
    }
}
