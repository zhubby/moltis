//! Sidecar process management for the WhatsApp Baileys sidecar.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
};

use {
    anyhow::{Context, Result, bail},
    tokio::{
        io::{AsyncBufReadExt, BufReader},
        process::{Child, Command},
        sync::RwLock,
    },
    tracing::{debug, error, info, warn},
};

use crate::sidecar::DEFAULT_SIDECAR_PORT;

/// Handle to a running sidecar process.
pub struct SidecarProcess {
    child: Child,
    port: u16,
}

impl SidecarProcess {
    /// Get the port the sidecar is listening on.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Check if the process is still running.
    pub fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Gracefully stop the sidecar process.
    pub async fn stop(&mut self) -> Result<()> {
        info!("stopping WhatsApp sidecar process");

        // Send SIGTERM for graceful shutdown.
        #[cfg(unix)]
        {
            use nix::{
                sys::signal::{Signal, kill},
                unistd::Pid,
            };

            if let Some(pid) = self.child.id() {
                let _ = kill(Pid::from_raw(pid as i32), Signal::SIGTERM);
            }
        }

        // On Windows or if SIGTERM didn't work, use kill.
        #[cfg(not(unix))]
        {
            let _ = self.child.kill().await;
        }

        // Wait for process to exit with timeout.
        match tokio::time::timeout(std::time::Duration::from_secs(5), self.child.wait()).await {
            Ok(Ok(status)) => {
                info!(?status, "WhatsApp sidecar process exited");
            },
            Ok(Err(e)) => {
                warn!(error = %e, "error waiting for sidecar process");
            },
            Err(_) => {
                warn!("sidecar process did not exit gracefully, killing");
                let _ = self.child.kill().await;
            },
        }

        Ok(())
    }
}

impl Drop for SidecarProcess {
    fn drop(&mut self) {
        // Best-effort kill on drop.
        if let Some(pid) = self.child.id() {
            debug!(pid, "dropping sidecar process handle");
        }
    }
}

/// Configuration for starting the sidecar process.
#[derive(Debug, Clone)]
pub struct SidecarConfig {
    /// Path to the sidecar directory (containing package.json).
    pub sidecar_dir: PathBuf,
    /// Port for the sidecar WebSocket server.
    pub port: u16,
    /// Base directory for WhatsApp auth files.
    pub auth_dir: Option<PathBuf>,
}

impl Default for SidecarConfig {
    fn default() -> Self {
        Self {
            sidecar_dir: PathBuf::new(),
            port: DEFAULT_SIDECAR_PORT,
            auth_dir: None,
        }
    }
}

/// Find the sidecar directory.
///
/// Searches in order:
/// 1. Explicit path if provided
/// 2. `MOLTIS_WHATSAPP_SIDECAR_DIR` environment variable
/// 3. Relative to the executable: `../sidecar/whatsapp-baileys`
/// 4. Common development paths
pub fn find_sidecar_dir(explicit_path: Option<&Path>) -> Result<PathBuf> {
    // 1. Explicit path.
    if let Some(path) = explicit_path {
        if path.join("package.json").exists() {
            return Ok(path.to_path_buf());
        }
        bail!(
            "sidecar directory does not exist or missing package.json: {}",
            path.display()
        );
    }

    // 2. Environment variable.
    if let Ok(dir) = std::env::var("MOLTIS_WHATSAPP_SIDECAR_DIR") {
        let path = PathBuf::from(&dir);
        if path.join("package.json").exists() {
            return Ok(path);
        }
        warn!(path = %dir, "MOLTIS_WHATSAPP_SIDECAR_DIR set but package.json not found");
    }

    // 3. Relative to executable.
    if let Ok(exe_path) = std::env::current_exe()
        && let Some(exe_dir) = exe_path.parent()
    {
        // Check ../sidecar/whatsapp-baileys (for installed binary).
        let candidate = exe_dir.join("../sidecar/whatsapp-baileys");
        if candidate.join("package.json").exists() {
            return Ok(candidate);
        }

        // Check ../../sidecar/whatsapp-baileys (for cargo run).
        let candidate = exe_dir.join("../../sidecar/whatsapp-baileys");
        if candidate.join("package.json").exists() {
            return Ok(candidate);
        }
    }

    // 4. Development paths (relative to cwd).
    let dev_paths = [
        "sidecar/whatsapp-baileys",
        "../sidecar/whatsapp-baileys",
        "../../sidecar/whatsapp-baileys",
    ];

    for rel_path in dev_paths {
        let path = PathBuf::from(rel_path);
        if path.join("package.json").exists() {
            return Ok(path.canonicalize().unwrap_or(path));
        }
    }

    bail!(
        "WhatsApp sidecar not found. Set MOLTIS_WHATSAPP_SIDECAR_DIR or ensure \
         sidecar/whatsapp-baileys exists with package.json"
    )
}

/// Check if the sidecar has been built (dist/index.js exists).
pub fn is_sidecar_built(sidecar_dir: &Path) -> bool {
    sidecar_dir.join("dist/index.js").exists()
}

/// Check if node_modules exists.
pub fn has_node_modules(sidecar_dir: &Path) -> bool {
    sidecar_dir.join("node_modules").exists()
}

/// Start the sidecar process.
pub async fn start_sidecar(config: SidecarConfig) -> Result<SidecarProcess> {
    let sidecar_dir = &config.sidecar_dir;

    // Verify the sidecar directory exists.
    if !sidecar_dir.join("package.json").exists() {
        bail!(
            "WhatsApp sidecar not found at {}. \
             Run `cd {} && npm install && npm run build` first.",
            sidecar_dir.display(),
            sidecar_dir.display()
        );
    }

    // Check if built.
    if !is_sidecar_built(sidecar_dir) {
        // Try to build it.
        info!(path = %sidecar_dir.display(), "building WhatsApp sidecar");

        if !has_node_modules(sidecar_dir) {
            run_npm_install(sidecar_dir).await?;
        }

        run_npm_build(sidecar_dir).await?;
    }

    info!(
        path = %sidecar_dir.display(),
        port = config.port,
        "starting WhatsApp sidecar process"
    );

    // Build the command.
    let mut cmd = Command::new("node");
    cmd.arg("dist/index.js")
        .current_dir(sidecar_dir)
        .env("MOLTIS_WHATSAPP_PORT", config.port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    if let Some(auth_dir) = &config.auth_dir {
        cmd.env("MOLTIS_WHATSAPP_AUTH_DIR", auth_dir);
    }

    let mut child = cmd.spawn().context("failed to spawn sidecar process")?;

    // Spawn tasks to forward stdout/stderr to tracing.
    if let Some(stdout) = child.stdout.take() {
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                // Parse JSON logs from pino.
                if line.starts_with('{')
                    && let Ok(log) = serde_json::from_str::<serde_json::Value>(&line)
                {
                    let level = log.get("level").and_then(|v| v.as_u64()).unwrap_or(30);
                    let msg = log.get("msg").and_then(|v| v.as_str()).unwrap_or(&line);
                    match level {
                        10 | 20 => debug!(target: "whatsapp_sidecar", "{}", msg),
                        30 => info!(target: "whatsapp_sidecar", "{}", msg),
                        40 => warn!(target: "whatsapp_sidecar", "{}", msg),
                        _ => error!(target: "whatsapp_sidecar", "{}", msg),
                    }
                    continue;
                }
                info!(target: "whatsapp_sidecar", "{}", line);
            }
        });
    }

    if let Some(stderr) = child.stderr.take() {
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                warn!(target: "whatsapp_sidecar", "{}", line);
            }
        });
    }

    // Wait a moment for the process to start and potentially fail.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Check if process is still running.
    match child.try_wait() {
        Ok(Some(status)) => {
            bail!("sidecar process exited immediately with status: {status}");
        },
        Ok(None) => {
            // Still running, good.
        },
        Err(e) => {
            bail!("failed to check sidecar process status: {e}");
        },
    }

    info!(port = config.port, "WhatsApp sidecar process started");

    Ok(SidecarProcess {
        child,
        port: config.port,
    })
}

/// Run `npm install` in the sidecar directory.
async fn run_npm_install(sidecar_dir: &Path) -> Result<()> {
    info!(path = %sidecar_dir.display(), "running npm install for sidecar");

    let output = Command::new("npm")
        .arg("install")
        .current_dir(sidecar_dir)
        .output()
        .await
        .context("failed to run npm install")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("npm install failed: {stderr}");
    }

    Ok(())
}

/// Run `npm run build` in the sidecar directory.
async fn run_npm_build(sidecar_dir: &Path) -> Result<()> {
    info!(path = %sidecar_dir.display(), "running npm build for sidecar");

    let output = Command::new("npm")
        .arg("run")
        .arg("build")
        .current_dir(sidecar_dir)
        .output()
        .await
        .context("failed to run npm build")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("npm build failed: {stderr}");
    }

    Ok(())
}

/// Shared handle for the sidecar process (for use across the plugin).
pub type SharedSidecarProcess = Arc<RwLock<Option<SidecarProcess>>>;
