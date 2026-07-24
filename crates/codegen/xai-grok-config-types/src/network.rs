//! Network / WARP configuration types.
//!
//! Types for `[network.warp]` section of `config.toml`:
//!
//! ```toml
//! [network.warp]
//! enabled = true
//! change_ip_on_compact = true
//! change_ip_on_start = false
//! rate_limit_secs = 300
//! sudo_policy = "ask"
//! auto_start_service = true
//! ```

use serde::{Deserialize, Serialize};

/// Top-level `[network]` section.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct NetworkConfig {
    pub warp: WarpConfig,
}

/// WARP-specific sub-configuration, deserialized from `[network.warp]`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct WarpConfig {
    /// Master switch: enable native WARP integration entirely.
    pub enabled: bool,

    /// Reconnect WARP after every compact to force a new public IP.
    pub change_ip_on_compact: bool,

    /// Reconnect WARP at the start of every new session.
    pub change_ip_on_start: bool,

    /// Minimum seconds between WARP reconnects (rate limiting).
    pub rate_limit_secs: u64,

    /// Policy for sudo prompts (install / start service).
    pub sudo_policy: SudoPolicy,

    /// Automatically start warp-svc when it is not running (Linux/WSL2).
    pub auto_start_service: bool,
}

impl Default for WarpConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            change_ip_on_compact: true,
            change_ip_on_start: false,
            rate_limit_secs: 300,
            sudo_policy: SudoPolicy::Ask,
            auto_start_service: true,
        }
    }
}

/// What to do when a WARP operation requires `sudo`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SudoPolicy {
    /// Prompt the user every time.
    Ask,
    /// Auto-approve sudo without prompting.
    AlwaysAllow,
    /// Never run sudo; skip operations that require it.
    AlwaysDeny,
}

impl Default for SudoPolicy {
    fn default() -> Self {
        Self::Ask
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warp_config_defaults() {
        let cfg = WarpConfig::default();
        assert!(!cfg.enabled);
        assert!(cfg.change_ip_on_compact);
        assert!(!cfg.change_ip_on_start);
        assert_eq!(cfg.rate_limit_secs, 300);
        assert_eq!(cfg.sudo_policy, SudoPolicy::Ask);
        assert!(cfg.auto_start_service);
    }

    #[test]
    fn warp_config_deser_toml_defaults() {
        // NetworkConfig is deserialized from the `[network]` sub-table
        // (mirrors `load_config_from_toml` which extracts `section(table, "network")`)
        let toml = r#"
[network]
[network.warp]
enabled = true
"#;
        let root: toml::Value = toml::from_str(toml).unwrap();
        let cfg: NetworkConfig = root
            .get("network")
            .map(|v| v.clone().try_into().unwrap())
            .unwrap_or_default();
        assert!(cfg.warp.enabled);
        // Should inherit defaults for unset fields
        assert!(cfg.warp.change_ip_on_compact);
        assert!(!cfg.warp.change_ip_on_start);
        assert_eq!(cfg.warp.rate_limit_secs, 300);
        assert_eq!(cfg.warp.sudo_policy, SudoPolicy::Ask);
    }

    #[test]
    fn warp_config_deser_toml_full() {
        let toml = r#"
[network]
[network.warp]
enabled = true
change_ip_on_compact = false
change_ip_on_start = true
rate_limit_secs = 600
sudo_policy = "always_allow"
auto_start_service = false
"#;
        let root: toml::Value = toml::from_str(toml).unwrap();
        let cfg: NetworkConfig = root
            .get("network")
            .map(|v| v.clone().try_into().unwrap())
            .unwrap_or_default();
        assert!(cfg.warp.enabled);
        assert!(!cfg.warp.change_ip_on_compact);
        assert!(cfg.warp.change_ip_on_start);
        assert_eq!(cfg.warp.rate_limit_secs, 600);
        assert_eq!(cfg.warp.sudo_policy, SudoPolicy::AlwaysAllow);
        assert!(!cfg.warp.auto_start_service);
    }

    #[test]
    fn sudo_policy_deser() {
        // Deserialize from within a proper TOML table structure
        // (toml 0.9+ doesn't support bare string values as documents)
        let cfg: WarpConfig = toml::from_str("sudo_policy = \"ask\"\n").unwrap();
        assert_eq!(cfg.sudo_policy, SudoPolicy::Ask);

        let cfg: WarpConfig = toml::from_str("sudo_policy = \"always_allow\"\n").unwrap();
        assert_eq!(cfg.sudo_policy, SudoPolicy::AlwaysAllow);

        let cfg: WarpConfig = toml::from_str("sudo_policy = \"always_deny\"\n").unwrap();
        assert_eq!(cfg.sudo_policy, SudoPolicy::AlwaysDeny);
    }

    #[test]
    fn sudo_policy_default() {
        assert_eq!(SudoPolicy::default(), SudoPolicy::Ask);
    }

    #[test]
    fn network_config_default() {
        let cfg = NetworkConfig::default();
        assert_eq!(cfg.warp, WarpConfig::default());
    }

    #[test]
    fn warp_config_deser_without_network_section() {
        // NetworkConfig with empty TOML should use defaults
        let toml = "";
        let cfg: NetworkConfig = toml::from_str(toml).unwrap();
        assert!(!cfg.warp.enabled);
    }
}
