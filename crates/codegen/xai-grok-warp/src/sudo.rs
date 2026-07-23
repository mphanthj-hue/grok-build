//! Sudo permission handling for WARP operations.
//!
//! Provides a channel-based mechanism to request sudo permissions
//! from the user via the TUI. The `SudoHandle` is passed to the
//! WARP client and used to send permission requests to the
//! `SessionActor`, which displays a TUI prompt.

use tokio::sync::oneshot;

/// Types of operations that may require sudo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SudoAction {
    /// Install Cloudflare WARP client.
    InstallWarp,
    /// Start the WARP service daemon (warp-svc).
    StartWarpService,
}

impl std::fmt::Display for SudoAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InstallWarp => write!(f, "install_warp"),
            Self::StartWarpService => write!(f, "start_warp_service"),
        }
    }
}

/// A sudo permission request sent from the WARP client to the session actor.
#[derive(Debug)]
pub struct SudoRequest {
    /// What operation requires sudo.
    pub action: SudoAction,
    /// Human-readable description for the TUI prompt.
    pub description: String,
    /// The sudo command to run (for display and execution).
    pub command: String,
    /// Channel to send the response back.
    pub response: oneshot::Sender<bool>,
}

impl SudoRequest {
    /// Create a new sudo request.
    pub fn new(action: SudoAction) -> Self {
        let (cmd, desc) = match action {
            SudoAction::InstallWarp => (
                "sudo apt-get install -y cloudflare-warp".to_string(),
                "Install Cloudflare WARP client".to_string(),
            ),
            SudoAction::StartWarpService => (
                "sudo systemctl enable --now warp-svc".to_string(),
                "Start WARP service (warp-svc)".to_string(),
            ),
        };

        let (response, _) = oneshot::channel();
        Self {
            action,
            description: desc,
            command: cmd,
            response,
        }
    }
}

/// Handle for requesting sudo permissions from the user.
///
/// Cloneable — multiple WARP components can share the same handle.
#[derive(Debug, Clone)]
pub struct SudoHandle {
    /// Channel to send sudo requests to the SessionActor.
    request_tx: tokio::sync::mpsc::Sender<SudoRequest>,
}

impl SudoHandle {
    /// Create a new sudo handle backed by the given sender.
    pub fn new(request_tx: tokio::sync::mpsc::Sender<SudoRequest>) -> Self {
        Self { request_tx }
    }

    /// Request sudo permission from the user.
    ///
    /// Returns `Ok(())` if the user granted permission, `Err(SudoDenied)` otherwise.
    pub async fn request(&self, action: SudoAction) -> Result<(), crate::WarpError> {
        let (tx, rx) = oneshot::channel();

        let (command, description) = match action {
            SudoAction::InstallWarp => (
                "sudo apt-get install -y cloudflare-warp".to_string(),
                "Install Cloudflare WARP client".to_string(),
            ),
            SudoAction::StartWarpService => (
                "sudo systemctl enable --now warp-svc".to_string(),
                "Start WARP service (warp-svc)".to_string(),
            ),
        };

        let request = SudoRequest {
            action,
            description,
            command,
            response: tx,
        };

        self.request_tx
            .send(request)
            .await
            .map_err(|_| crate::WarpError::SudoDenied)?;

        match rx.await {
            Ok(true) => Ok(()),
            Ok(false) | Err(_) => Err(crate::WarpError::SudoDenied),
        }
    }

    /// Check if the handle is still connected.
    pub fn is_connected(&self) -> bool {
        !self.request_tx.is_closed()
    }
}

/// A placeholder sudo handle that always denies (for use when no TUI is available).
#[derive(Debug, Clone)]
pub struct DenyAllSudo;

impl DenyAllSudo {
    /// Always returns `SudoDenied`.
    pub async fn request(&self, _action: SudoAction) -> Result<(), crate::WarpError> {
        Err(crate::WarpError::SudoDenied)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sudo_action_display() {
        assert_eq!(SudoAction::InstallWarp.to_string(), "install_warp");
        assert_eq!(
            SudoAction::StartWarpService.to_string(),
            "start_warp_service"
        );
    }

    #[tokio::test]
    async fn sudo_request_creation() {
        let req = SudoRequest::new(SudoAction::InstallWarp);
        assert_eq!(req.action, SudoAction::InstallWarp);
        assert!(!req.description.is_empty());
        assert!(!req.command.is_empty());
        assert!(req.command.contains("apt-get"));
    }

    #[tokio::test]
    async fn sudo_handle_disconnected_returns_denied() {
        // Create a channel and drop the receiver
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let handle = SudoHandle::new(tx);

        // Drop the receiver
        drop(_rx);

        let result = handle.request(SudoAction::InstallWarp).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn deny_all_sudo_always_denies() {
        let deny = DenyAllSudo;
        let result = deny.request(SudoAction::InstallWarp).await;
        assert!(result.is_err());
    }
}
