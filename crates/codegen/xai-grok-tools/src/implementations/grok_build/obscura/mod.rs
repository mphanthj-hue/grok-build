//! `browser` tool — native wrapper around the `obscura` binary.
//!
//! Calls `obscura browse <url>` via the Terminal backend to render
//! JavaScript-heavy pages that `web_fetch` (reqwest) cannot handle.

use crate::computer::types::{TaskKind, TerminalRunRequest};
use crate::types::output::BashOutput;
use crate::types::requirements::{Expr, ToolRequirement};
use crate::types::resources::{
    Cwd, NotificationHandle, SessionEnv, SessionFolder, Terminal,
};
use crate::types::tool::{ToolKind, ToolNamespace};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

// ── Config ──────────────────────────────────────────────────────────────────

/// Configuration for the `browser` (Obscura) tool.
///
/// When `Enabled`, the tool is registered and calls `obscura browse <url>`
/// via the shared `TerminalBackend`. When `Disabled` (default), the tool
/// is not registered — graceful degradation when the binary is absent.
#[derive(Debug, Clone, Default)]
pub enum ObscuraConfig {
    #[default]
    Disabled,
    Enabled {
        /// Path to the obscura binary. Defaults to "obscura" (PATH lookup).
        binary_path: String,
        /// Timeout in seconds for browser rendering. Default: 30.
        timeout_secs: u64,
    },
}

impl ObscuraConfig {
    pub fn is_enabled(&self) -> bool {
        matches!(self, Self::Enabled { .. })
    }
    pub fn binary_path(&self) -> &str {
        match self {
            Self::Enabled { binary_path, .. } => binary_path,
            Self::Disabled => "obscura",
        }
    }
    pub fn timeout_secs(&self) -> u64 {
        match self {
            Self::Enabled { timeout_secs, .. } => *timeout_secs,
            Self::Disabled => 30,
        }
    }
}

// ── Input ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ObscuraInput {
    /// The URL to render in the headless browser.
    #[schemars(description = "The URL to render and return as markdown.")]
    pub url: String,
    /// Optional CSS selector to wait for before capturing content.
    #[schemars(description = "Optional CSS selector to wait for before capturing.")]
    pub wait_for: Option<String>,
}

// ── Tool ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct ObscuraTool;

impl crate::types::tool_metadata::ToolMetadata for ObscuraTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Browser
    }
    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }
    fn description_template(&self) -> &str {
        r#"Render a URL in a headless browser and return its content as markdown.

Use this tool instead of web_fetch when:
- The page requires JavaScript to render content
- The page has dynamic content loaded after initial HTML
- web_fetch returns empty or incomplete content

Usage notes:
  - Slower than web_fetch (spawns a headless browser)
  - Use web_fetch first; fall back to browser for JS-heavy pages"#
    }
    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for ObscuraTool {
    type Args = ObscuraInput;
    type Output = BashOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new("browser").expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            "browser",
            crate::types::tool_metadata::ToolMetadata::description_template(self),
        )
    }

    fn capabilities(&self) -> xai_tool_protocol::ToolCapabilities {
        xai_tool_protocol::ToolCapabilities {
            is_read_only: true,
            tool_scope: Some(xai_tool_protocol::ToolScope::Read),
            ..Default::default()
        }
    }

    #[tracing::instrument(name = "tool.browser", skip_all, fields(url = %input.url))]
    async fn run(
        &self,
        ctx: xai_tool_runtime::ToolCallContext,
        input: ObscuraInput,
    ) -> Result<BashOutput, xai_tool_runtime::ToolError> {
        use crate::types::tool_metadata::shared_resources;
        let resources = shared_resources(&ctx)?;

        let (terminal, cwd, session_folder, notification_handle, session_env) = {
            let res = resources.lock().await;
            let terminal = res.require::<Terminal>()?.0.clone();
            let cwd = res.require::<Cwd>()?.0.clone();
            let session_folder = res
                .get::<SessionFolder>()
                .map(|f| f.0.clone())
                .unwrap_or_else(|| PathBuf::from("/tmp"));
            let notification_handle = res
                .get::<NotificationHandle>()
                .map(|h| h.0.clone())
                .unwrap_or_else(crate::notification::ToolNotificationHandle::noop);
            let session_env = res
                .get::<SessionEnv>()
                .map(|e| e.0.clone())
                .unwrap_or_else(|| Arc::new(HashMap::new()));
            (terminal, cwd, session_folder, notification_handle, session_env)
        };

        // Build: obscura browse <url> [--wait-for <selector>]
        let mut cmd = format!("obscura browse {}", shell_escape(&input.url));
        if let Some(sel) = &input.wait_for {
            cmd.push_str(&format!(" --wait-for {}", shell_escape(sel)));
        }

        let output_file = session_folder.join(format!(
            "browser_{}.txt",
            chrono::Utc::now().format("%Y%m%d_%H%M%S")
        ));

        let timeout = Duration::from_secs(30);

        let req = TerminalRunRequest {
            command: cmd,
            working_directory: cwd,
            env: (*session_env).clone(),
            timeout,
            output_byte_limit: crate::DEFAULT_TOOL_OUTPUT_CHARS,
            output_file,
            notification_handle,
            tool_call_id: ctx.call_id.as_str().to_owned(),
            display_command: None,
            auto_background_on_timeout: false,
            foreground_block_budget: None,
            kind: TaskKind::Bash,
            owner_session_id: None,
        };

        let result = terminal.run(req).await.map_err(|e| {
            xai_tool_runtime::ToolError::execution(
                xai_tool_protocol::ToolId::new("browser").expect("valid tool id"),
                format!("Obscura browser error: {e}"),
            )
        })?;

        Ok(BashOutput {
            output: result.combined_output.as_bytes().to_vec(),
            output_for_prompt: BashOutput::make_output_for_prompt(&result.combined_output),
            exit_code: result.exit_code.unwrap_or(-1),
            command: String::new(),
            truncated: result.truncated,
            signal: result.signal,
            timed_out: result.timed_out,
            description: None,
            current_dir: String::new(),
            output_file: result.output_file.to_string_lossy().to_string(),
            total_bytes: result.total_bytes,
            output_delta: None,
            was_bare_echo: false,
        })
    }
}

fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::tool_metadata::test_ctx_with_call_id;

    #[test]
    fn tool_name_and_description() {
        let tool = ObscuraTool;
        assert_eq!(xai_tool_runtime::Tool::id(&tool).as_str(), "browser");
        assert_eq!(
            crate::types::tool_metadata::ToolMetadata::kind(&tool),
            ToolKind::Browser
        );
        assert!(
            crate::types::tool_metadata::ToolMetadata::description_template(&tool)
                .contains("Render a URL in a headless browser")
        );
    }

    #[tokio::test]
    async fn errors_when_terminal_not_in_resources() {
        let resources = crate::types::resources::Resources::new();
        let tool = ObscuraTool;
        let result = xai_tool_runtime::Tool::run(
            &tool,
            test_ctx_with_call_id(resources.into_shared(), "test-call"),
            ObscuraInput {
                url: "https://example.com".into(),
                wait_for: None,
            },
        )
        .await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("missing required resource"),
            "Expected 'missing required resource' error, got: {err_msg}"
        );
    }
}
