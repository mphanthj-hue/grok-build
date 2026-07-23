//! Native WARP IP rotation for `SessionActor`.
//!
//! Provides `maybe_rotate_warp_ip()` which checks config, rate-limit, and
//! platform availability before reconnecting warp-cli. Called from:
//!
//! - Post-compaction (in `compaction.rs` after the hook dispatch, trigger "compact")
//! - Session start (in `run_loop.rs` after the SessionStart hook, trigger "start")

use super::*;
use std::sync::atomic::Ordering;

/// Snapshot of WARP config fields used by the rotation decision.
/// Stored on `SessionActor` so we never need to reach through `Agent` for config.
pub(crate) struct WarpRotationState {
    /// Unix-epoch seconds of the last rotation (for rate-limiting).
    last_rotation: std::sync::atomic::AtomicU64,
    /// Master switch — `[network.warp].enabled`.
    enabled: bool,
    /// Whether to rotate after compaction.
    change_ip_on_compact: bool,
    /// Whether to rotate at session start.
    change_ip_on_start: bool,
    /// Minimum seconds between rotations (clamped ≥ 5).
    rate_limit_secs: u64,
}

impl WarpRotationState {
    /// Build state from the effective `WarpConfig`.
    pub(crate) fn from_warp_config(warp: &xai_grok_config_types::WarpConfig) -> Self {
        Self {
            last_rotation: std::sync::atomic::AtomicU64::new(0),
            enabled: warp.enabled,
            change_ip_on_compact: warp.change_ip_on_compact,
            change_ip_on_start: warp.change_ip_on_start,
            rate_limit_secs: warp.rate_limit_secs.max(5),
        }
    }

    /// Check whether the master switch is on AND the given trigger is enabled.
    fn trigger_allowed(&self, trigger: &str) -> bool {
        if !self.enabled {
            return false;
        }
        match trigger {
            "compact" => self.change_ip_on_compact,
            "start" => self.change_ip_on_start,
            _ => false,
        }
    }
}

impl SessionActor {
    /// Check whether WARP IP rotation should be attempted for `trigger`.
    /// Checks the trigger-specific flag, the rate limiter, and platform
    /// availability before reconnecting warp-cli.
    /// Returns `true` if rotation was attempted.
    pub(crate) async fn maybe_rotate_warp_ip(&self, trigger: &'static str) -> bool {
        // 1. WarpRotationState must exist (None → disabled at spawn)
        let warp_state = match self.warp_rotation_state.as_ref() {
            Some(s) => s,
            None => return false,
        };

        // 2. Check master switch + trigger gate
        if !warp_state.trigger_allowed(trigger) {
            tracing::debug!(target: "warp", %trigger, "WARP rotation skipped: disabled for trigger");
            return false;
        }

        // 3. Rate-limit check
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let last = warp_state.last_rotation.load(Ordering::Relaxed);
        if now < last + warp_state.rate_limit_secs {
            let remaining = last + warp_state.rate_limit_secs - now;
            tracing::debug!(
                target: "warp",
                remaining_secs = remaining,
                "WARP rotation skipped: rate limited"
            );
            return false;
        }

        // 4. Detect platform, find warp-cli
        let platform = xai_grok_warp::detect_platform();
        let cli_path = match xai_grok_warp::find_warp_cli(&platform) {
            Some(p) => p,
            None => {
                tracing::warn!(target: "warp", "WARP rotation skipped: warp-cli not found on {platform}");
                return false;
            }
        };

        // 5. Build client (WarpClient::new is infallible given the path)
        let client = xai_grok_warp::WarpClient::new(cli_path, platform);

        // 6. Attempt reconnect
        tracing::info!(target: "warp", platform = %platform, "WARP rotating IP...");
        match client.reconnect().await {
            Ok(()) => {
                warp_state.last_rotation.store(now, Ordering::Relaxed);

                // Log new IP (best-effort)
                match xai_grok_warp::get_public_ip().await {
                    Ok(ip) => {
                        tracing::info!(target: "warp", new_ip = %ip, "WARP IP rotated successfully");
                    }
                    Err(e) => {
                        tracing::warn!(target: "warp", error = %e, "WARP IP rotated but could not determine new IP");
                    }
                }
                true
            }
            Err(e) => {
                tracing::warn!(target: "warp", error = %e, "WARP rotation failed");
                false
            }
        }
    }
}
