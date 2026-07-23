//! Platform detection and warp-cli binary discovery.

use std::path::PathBuf;

/// Supported operating systems for WARP operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// Native Linux (debian/ubuntu-based, apt).
    Linux,
    /// WSL2 (Windows Subsystem for Linux).
    Wsl2,
    /// macOS (Cloudflare WARP.app).
    Macos,
    /// Native Windows.
    Windows,
    /// Unknown / unsupported platform.
    Unknown,
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Linux => write!(f, "linux"),
            Self::Wsl2 => write!(f, "wsl2"),
            Self::Macos => write!(f, "macos"),
            Self::Windows => write!(f, "windows"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Detect the current platform.
pub fn detect_platform() -> Platform {
    match std::env::consts::OS {
        "linux" => {
            // Check for WSL2
            if is_wsl() {
                Platform::Wsl2
            } else {
                Platform::Linux
            }
        }
        "macos" => Platform::Macos,
        "windows" => Platform::Windows,
        _ => Platform::Unknown,
    }
}

/// Check if running inside WSL (Windows Subsystem for Linux).
fn is_wsl() -> bool {
    // /proc/version contains "microsoft" or "Microsoft" on WSL
    if let Ok(content) = std::fs::read_to_string("/proc/version") {
        content.to_lowercase().contains("microsoft")
    } else {
        false
    }
}

/// Check if WSL interop is enabled (can run Windows executables).
pub fn wsl_interop_enabled() -> bool {
    // Check if cmd.exe is available via interop
    if std::process::Command::new("cmd.exe")
        .arg("/c")
        .arg("echo 1")
        .output()
        .is_ok()
    {
        return true;
    }

    // Check the WSL interop flag
    if let Ok(content) = std::fs::read_to_string("/proc/sys/fs/binfmt_misc/WSLInterop") {
        return content.trim() == "enabled";
    }

    false
}

/// Find the `warp-cli` binary for the given platform.
///
/// Returns `None` if the binary cannot be found.
pub fn find_warp_cli(platform: &Platform) -> Option<PathBuf> {
    match platform {
        Platform::Linux => {
            // Check $PATH first
            if let Some(path) = find_in_path("warp-cli") {
                return Some(path);
            }
            // Check standard location
            let standard = PathBuf::from("/usr/bin/warp-cli");
            if standard.is_file() {
                return Some(standard);
            }
            // Check with --accept-tos alias
            let alt = PathBuf::from("/usr/bin/warp-cli");
            if alt.exists() {
                return Some(alt);
            }
            None
        }
        Platform::Wsl2 => {
            // Priority 1: Linux warp-cli in WSL
            if let Some(path) = find_in_path("warp-cli") {
                return Some(path);
            }
            let standard = PathBuf::from("/usr/bin/warp-cli");
            if standard.is_file() {
                return Some(standard);
            }

            // Priority 2: Windows warp-cli.exe via interop
            if wsl_interop_enabled() {
                // Check $PATH for warp-cli.exe
                if let Some(path) = find_in_path("warp-cli.exe") {
                    return Some(path);
                }
                // Check common Windows paths via /mnt/c
                for win_path in [
                    r"/mnt/c/Program Files/Cloudflare/Cloudflare WARP/warp-cli.exe",
                    r"/mnt/c/Program Files (x86)/Cloudflare/Cloudflare WARP/warp-cli.exe",
                ] {
                    let p = PathBuf::from(win_path);
                    if p.is_file() {
                        return Some(p);
                    }
                }
            }

            None
        }
        Platform::Macos => {
            // Check standard macOS locations
            let app_path =
                PathBuf::from("/Applications/Cloudflare WARP.app/Contents/Resources/warp-cli");
            if app_path.is_file() {
                return Some(app_path);
            }
            let homebrew_path = PathBuf::from("/opt/homebrew/bin/warp-cli");
            if homebrew_path.is_file() {
                return Some(homebrew_path);
            }
            // Check $PATH
            find_in_path("warp-cli")
        }
        Platform::Windows => {
            // Check $PATH
            if let Some(path) = find_in_path("warp-cli.exe") {
                return Some(path);
            }
            // Check common locations
            for win_path in [
                r"C:\Program Files\Cloudflare\Cloudflare WARP\warp-cli.exe",
                r"C:\Program Files (x86)\Cloudflare\Cloudflare WARP\warp-cli.exe",
            ] {
                let p = PathBuf::from(win_path);
                if p.is_file() {
                    return Some(p);
                }
            }
            None
        }
        Platform::Unknown => None,
    }
}

/// Find an executable in $PATH.
fn find_in_path(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|dir| {
            let full_path = dir.join(name);
            if full_path.is_file() {
                // On Unix, check if it's executable
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(metadata) = std::fs::metadata(&full_path) {
                        if metadata.permissions().mode() & 0o111 != 0 {
                            return Some(full_path);
                        }
                    }
                    return None;
                }
                #[cfg(not(unix))]
                {
                    Some(full_path)
                }
            } else {
                None
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_display() {
        assert_eq!(Platform::Linux.to_string(), "linux");
        assert_eq!(Platform::Wsl2.to_string(), "wsl2");
        assert_eq!(Platform::Macos.to_string(), "macos");
        assert_eq!(Platform::Windows.to_string(), "windows");
        assert_eq!(Platform::Unknown.to_string(), "unknown");
    }

    #[test]
    fn detect_platform_does_not_panic() {
        // Just ensure it returns something without errors
        let platform = detect_platform();
        // Should match the current OS
        match std::env::consts::OS {
            "linux" => assert!(
                platform == Platform::Linux || platform == Platform::Wsl2,
                "Expected Linux or Wsl2, got {platform:?}"
            ),
            "macos" => assert_eq!(platform, Platform::Macos),
            "windows" => assert_eq!(platform, Platform::Windows),
            _ => assert_eq!(platform, Platform::Unknown),
        }
    }
}
