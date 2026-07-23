//! Native Cloudflare WARP integration for Grok Build.
//!
//! Provides platform detection, warp-cli interaction, IP checking,
//! rate limiting, and sudo permission handling. Replaces the shell-script
//! hook approach with a fully typed Rust implementation.
//!
//! ## Usage
//!
//! ```rust,no_run
//! # async fn example() {
//! use xai_grok_warp::WarpClient;
//!
//! // Try to create a WARP client (returns None if warp-cli not found)
//! if let Some(client) = WarpClient::try_new().ok().flatten() {
//!     let status = client.status().await;
//!     println!("WARP status: {status:?}");
//! }
//! # }
//! ```

pub mod client;
pub mod ip_check;
pub mod platform;
pub mod rate_limiter;
pub mod sudo;

// Re-exports
pub use client::WarpClient;
pub use ip_check::get_public_ip;
pub use platform::{Platform, detect_platform, find_warp_cli};
pub use rate_limiter::RateLimiter;
pub use sudo::{SudoAction, SudoHandle, SudoRequest};

/// Errors that can occur during WARP operations.
#[derive(Debug, thiserror::Error)]
pub enum WarpError {
    /// warp-cli binary not found anywhere.
    #[error("warp-cli not found")]
    NotInstalled,

    /// Platform is not supported for WARP operations.
    #[error("platform not supported: {0}")]
    UnsupportedPlatform(String),

    /// warp-cli returned a non-zero exit code.
    #[error("warp-cli error: {message}")]
    CliError {
        #[source]
        source: CliExitError,
        message: String,
    },

    /// Network request failed (IP check).
    #[error("network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    /// Could not determine public IP.
    #[error("IP check failed: {0}")]
    IpCheckFailed(String),

    /// User denied a sudo permission request.
    #[error("sudo permission denied")]
    SudoDenied,

    /// WARP operation timed out.
    #[error("timeout after {0}s")]
    Timeout(u64),

    /// I/O error.
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Wrapper for warp-cli exit code that implements `Display` for any `Option<i32>`.
#[derive(Debug)]
pub struct CliExitError(pub Option<i32>);

impl std::fmt::Display for CliExitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(code) => write!(f, "exit code {code}"),
            None => write!(f, "unknown exit code"),
        }
    }
}

impl std::error::Error for CliExitError {}

/// WARP connection status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarpStatus {
    Connected,
    Disconnected,
    Unknown,
}

impl std::fmt::Display for WarpStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connected => write!(f, "Connected"),
            Self::Disconnected => write!(f, "Disconnected"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Health information about the WARP environment.
#[derive(Debug, Clone)]
pub struct WarpHealth {
    /// Whether the device is registered with Cloudflare WARP.
    pub registered: bool,
    /// Whether the warp-svc daemon is running (Linux/WSL2).
    pub service_running: bool,
    /// Current WARP connection status.
    pub status: WarpStatus,
    /// Platform this check ran on.
    pub platform: Platform,
}

impl std::fmt::Display for WarpHealth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "WARP[platform={}, registered={}, service={}, status={}]",
            self.platform, self.registered, self.service_running, self.status
        )
    }
}

/// Default timeout for warp-cli commands, in seconds.
pub const WARP_CLI_TIMEOUT_SECS: u64 = 15;
/// Default interval between WARP reconnects, in seconds.
pub const WARP_RATE_LIMIT_SECS: u64 = 300;
