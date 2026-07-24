//! WARP client — interacts with the `warp-cli` binary.

use std::path::PathBuf;
use std::time::Duration;

use crate::platform::{Platform, find_warp_cli};
use crate::{WARP_CLI_TIMEOUT_SECS, WarpError, WarpHealth, WarpStatus};

/// Client for interacting with Cloudflare WARP via the `warp-cli` binary.
///
/// ```rust,no_run
/// use xai_grok_warp::WarpClient;
///
/// # async fn example() {
/// if let Some(client) = WarpClient::try_new().ok().flatten() {
///     let status = client.status().await;
///     println!("WARP status: {status:?}");
/// }
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct WarpClient {
    /// Path to the warp-cli binary.
    binary: PathBuf,
    /// Detected platform.
    platform: Platform,
    /// Whether to pass the --accept-tos flag (Linux).
    accept_tos: bool,
}

impl WarpClient {
    /// Try to create a new WARP client by finding `warp-cli` on the system.
    ///
    /// Returns:
    /// - `Ok(Some(client))` — warp-cli found and client created.
    /// - `Ok(None)` — warp-cli not found (caller should try install or skip).
    /// - `Err(e)` — unexpected error during detection.
    pub fn try_new() -> Result<Option<Self>, WarpError> {
        let platform = crate::platform::detect_platform();
        let binary = match find_warp_cli(&platform) {
            Some(b) => b,
            None => return Ok(None),
        };

        Ok(Some(Self {
            binary,
            platform,
            accept_tos: matches!(platform, Platform::Linux | Platform::Wsl2),
        }))
    }

    /// Create a client with explicit binary path and platform.
    pub fn new(binary: PathBuf, platform: Platform) -> Self {
        let accept_tos = matches!(platform, Platform::Linux | Platform::Wsl2);
        Self {
            binary,
            platform,
            accept_tos,
        }
    }

    /// The detected platform.
    pub fn platform(&self) -> Platform {
        self.platform
    }

    /// The warp-cli binary path.
    pub fn binary_path(&self) -> &PathBuf {
        &self.binary
    }

    /// Build the full args list, optionally prepending `--accept-tos`.
    fn build_args(&self, args: &[&str]) -> Vec<String> {
        let mut full = Vec::new();
        if self.accept_tos {
            full.push("--accept-tos".to_string());
        }
        full.extend(args.iter().map(|s| s.to_string()));
        full
    }

    /// Run a warp-cli command and return stdout on success.
    async fn run_warp_cli(&self, args: &[&str]) -> Result<String, WarpError> {
        let full_args = self.build_args(args);

        let result = tokio::time::timeout(
            Duration::from_secs(WARP_CLI_TIMEOUT_SECS),
            tokio::process::Command::new(&self.binary)
                .args(&full_args)
                .output(),
        )
        .await;

        let output = match result {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => return Err(WarpError::IoError(e)),
            Err(_) => {
                return Err(WarpError::Timeout(WARP_CLI_TIMEOUT_SECS));
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        if !output.status.success() {
            let message = if stderr.is_empty() { stdout } else { stderr };
            return Err(WarpError::CliError {
                source: crate::CliExitError(output.status.code()),
                message,
            });
        }

        Ok(stdout)
    }

    /// Check current WARP connection status.
    pub async fn status(&self) -> Result<WarpStatus, WarpError> {
        let output = self.run_warp_cli(&["status"]).await?;

        if output.contains("Connected") {
            Ok(WarpStatus::Connected)
        } else if output.contains("Disconnected") {
            Ok(WarpStatus::Disconnected)
        } else {
            Ok(WarpStatus::Unknown)
        }
    }

    /// Disconnect WARP.
    pub async fn disconnect(&self) -> Result<(), WarpError> {
        self.run_warp_cli(&["disconnect"]).await?;
        Ok(())
    }

    /// Connect WARP.
    pub async fn connect(&self) -> Result<(), WarpError> {
        self.run_warp_cli(&["connect"]).await?;
        Ok(())
    }

    /// Force reconnect (disconnect → connect) to get a new IP.
    pub async fn reconnect(&self) -> Result<(), WarpError> {
        // Always disconnect first for a clean state
        let _ = self.disconnect().await;
        tokio::time::sleep(Duration::from_secs(1)).await;
        self.connect().await?;
        tokio::time::sleep(Duration::from_secs(2)).await;
        Ok(())
    }

    /// Check WARP device registration (Linux/WSL2 only).
    pub async fn is_registered(&self) -> Result<bool, WarpError> {
        if !self.accept_tos {
            // macOS/Windows: registration not applicable or handled by GUI
            return Ok(true);
        }

        let output = self.run_warp_cli(&["registration", "show"]).await;
        match output {
            Ok(s) => Ok(!s.contains("No registration") && !s.is_empty()),
            Err(e) => {
                // "No registration" may return non-zero exit
                tracing::warn!(error = %e, "registration check failed");
                Ok(false)
            }
        }
    }

    /// Suggest user to register device (Linux/WSL2 only, usually after install).
    pub async fn register_device(&self) -> Result<(), WarpError> {
        if !self.accept_tos {
            return Ok(());
        }
        let _ = self.run_warp_cli(&["registration", "new"]).await?;
        Ok(())
    }

    /// Validate the WARP environment: service status, registration, and connection.
    ///
    /// This is a comprehensive check that should be called once at session start.
    pub async fn validate_environment(&self) -> Result<WarpHealth, WarpError> {
        let status = self.status().await.unwrap_or(WarpStatus::Unknown);
        let registered = self.is_registered().await.unwrap_or(false);
        let service_running = self.check_service_running().await;

        Ok(WarpHealth {
            registered,
            service_running,
            status,
            platform: self.platform,
        })
    }

    /// Check if the WARP service daemon is running (Linux/WSL2 only).
    async fn check_service_running(&self) -> bool {
        match self.platform {
            Platform::Linux | Platform::Wsl2 => {
                // Try systemctl is-active, fall back to pgrep
                let result = tokio::process::Command::new("systemctl")
                    .args(["is-active", "warp-svc"])
                    .output()
                    .await;

                match result {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        stdout.trim() == "active"
                    }
                    Err(_) => {
                        // Fallback: check process
                        std::process::Command::new("pgrep")
                            .args(["-x", "warp-svc"])
                            .output()
                            .map(|o| o.status.success())
                            .unwrap_or(false)
                    }
                }
            }
            _ => true, // macOS/Windows: service managed by GUI/app
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warp_client_new_returns_none_when_no_warp_cli() {
        // This will likely return None on CI/test machines without warp-cli
        let client = WarpClient::try_new().unwrap();
        // Either Some or None is fine, just shouldn't error
        if let Some(c) = client {
            assert!(c.binary_path().is_file());
        }
    }

    #[test]
    fn build_args_linux_accept_tos() {
        let client = WarpClient::new(PathBuf::from("/usr/bin/warp-cli"), Platform::Linux);
        let args = client.build_args(&["status"]);
        assert_eq!(args, vec!["--accept-tos", "status"]);
    }

    #[test]
    fn build_args_macos_no_accept_tos() {
        let client = WarpClient::new(
            PathBuf::from("/Applications/Cloudflare WARP.app/Contents/Resources/warp-cli"),
            Platform::Macos,
        );
        let args = client.build_args(&["status"]);
        assert_eq!(args, vec!["status"]);
    }

    #[test]
    fn build_args_wsl2_accept_tos() {
        let client = WarpClient::new(PathBuf::from("/usr/bin/warp-cli"), Platform::Wsl2);
        let args = client.build_args(&["status"]);
        assert_eq!(args, vec!["--accept-tos", "status"]);
    }
}
