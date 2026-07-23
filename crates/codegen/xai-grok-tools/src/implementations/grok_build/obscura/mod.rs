//! `browser` tools — native wrappers around the `obscura` binary.
//!
//! Calls the `obscura` headless browser via the Terminal backend to render
//! JavaScript-heavy pages that `web_fetch` (reqwest) cannot handle.
//!
//! Provides two tools:
//! - `ObscuraFetchTool` (id: "browser") — full-featured `fetch` with all dump modes
//! - `ObscuraScrapeTool` (id: "obscura_scrape") — batch scrape multiple URLs

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

/// Configuration for the `browser` (Obscura) tools.
///
/// When `Enabled`, the tools are registered and call the `obscura` binary
/// via the shared `TerminalBackend`. When `Disabled` (default), the tools
/// are not registered — graceful degradation when the binary is absent.
#[derive(Debug, Clone, Default)]
pub enum ObscuraConfig {
    #[default]
    Disabled,
    Enabled {
        /// Path to the obscura binary. Defaults to "obscura" (PATH lookup).
        binary_path: String,
        /// Default timeout in seconds for browser rendering. Default: 30.
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

// ── Shared helpers ──────────────────────────────────────────────────────────

fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Shared resource extraction — every tool needs the same set.
async fn extract_resources(
    ctx: &xai_tool_runtime::ToolCallContext,
) -> Result<
    (
        Arc<dyn crate::computer::types::TerminalBackend>,
        PathBuf,
        PathBuf,
        crate::notification::ToolNotificationHandle,
        Arc<HashMap<String, String>>,
    ),
    xai_tool_runtime::ToolError,
> {
    use crate::types::tool_metadata::shared_resources;
    let resources = shared_resources(ctx)?;
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
    Ok((terminal, cwd, session_folder, notification_handle, session_env))
}

/// Run an obscura command through the Terminal backend and return BashOutput.
async fn run_obscura(
    cmd: String,
    ctx: &xai_tool_runtime::ToolCallContext,
    input_timeout_secs: Option<u64>,
    default_timeout_secs: u64,
) -> Result<BashOutput, xai_tool_runtime::ToolError> {
    let (terminal, cwd, session_folder, notification_handle, session_env) =
        extract_resources(ctx).await?;

    let output_file = session_folder.join(format!(
        "obscura_{}.txt",
        chrono::Utc::now().format("%Y%m%d_%H%M%S")
    ));

    let timeout = Duration::from_secs(input_timeout_secs.unwrap_or(default_timeout_secs));

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
            format!("Obscura error: {e}"),
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

// ───────────────────────────────────────────────────────────────────────────
// ObscuraFetchTool  (id: "browser")
// ───────────────────────────────────────────────────────────────────────────

/// Output format for `obscura fetch`.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DumpMode {
    /// Clean markdown (default).
    Markdown,
    /// Plain text.
    Text,
    /// Raw HTML.
    Html,
    /// List of all links.
    Links,
    /// Stream raw HTTP response body (binary-safe).
    Original,
    /// JSON listing of all sub-resource URLs.
    Assets,
    /// JSON array of all cookies (including HttpOnly).
    Cookies,
}

impl std::fmt::Display for DumpMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Markdown => write!(f, "markdown"),
            Self::Text => write!(f, "text"),
            Self::Html => write!(f, "html"),
            Self::Links => write!(f, "links"),
            Self::Original => write!(f, "original"),
            Self::Assets => write!(f, "assets"),
            Self::Cookies => write!(f, "cookies"),
        }
    }
}

/// Navigation condition for page load completion.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WaitUntil {
    /// Fires when the initial HTML document is fully loaded and parsed.
    Load,
    /// Fires when there are no more than 2 network connections for at least 500ms.
    NetworkIdle0,
}

impl std::fmt::Display for WaitUntil {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Load => write!(f, "load"),
            Self::NetworkIdle0 => write!(f, "networkidle0"),
        }
    }
}

/// Input for `ObscuraFetchTool` (id: "browser").
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ObscuraInput {
    /// The URL to render in the headless browser.
    #[schemars(description = "The URL to render and return.")]
    pub url: String,

    /// Output format. Default: markdown.
    #[schemars(description = "Output format. Default: markdown.")]
    pub dump: Option<DumpMode>,

    /// Optional CSS selector to wait for before capturing content.
    #[schemars(description = "CSS selector to wait for before capturing.")]
    pub wait_for: Option<String>,

    /// Seconds to wait for the page before timing out. Default: 30.
    #[schemars(description = "Seconds to wait before timing out. Default: 30.")]
    pub timeout: Option<u64>,

    /// Seconds to wait for the selector / initial load. Default: 5.
    #[schemars(description = "Seconds to wait for selector before capturing. Default: 5.")]
    pub wait: Option<u64>,

    /// When to consider navigation complete. Default: load.
    #[schemars(description = "Navigation completion condition. Default: load.")]
    pub wait_until: Option<WaitUntil>,

    /// JavaScript expression to evaluate. Result replaces page content.
    #[schemars(description = "JavaScript expression to evaluate on the page.")]
    pub eval: Option<String>,

    /// Enable stealth mode (anti-bot fingerprinting).
    #[schemars(description = "Enable stealth mode (anti-bot detection).")]
    pub stealth: Option<bool>,

    /// Custom User-Agent string.
    #[schemars(description = "Custom User-Agent string.")]
    pub user_agent: Option<String>,

    /// Proxy URL.
    #[schemars(description = "HTTP proxy URL.")]
    pub proxy: Option<String>,
}

/// Native tool that wraps `obscura fetch` with full option support.
///
/// Use this instead of `web_fetch` when:
/// - The page requires JavaScript to render content
/// - The page has dynamic content loaded after initial HTML
/// - web_fetch returns empty or incomplete content
#[derive(Debug, Default)]
pub struct ObscuraFetchTool;

impl crate::types::tool_metadata::ToolMetadata for ObscuraFetchTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Browser
    }
    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }
    fn description_template(&self) -> &str {
        r#"Render a URL in a headless browser and return its content.

Use this tool instead of web_fetch when:
- The page requires JavaScript to render content
- The page has dynamic content loaded after initial HTML
- web_fetch returns empty or incomplete content

Supported output formats:
- markdown (default): Clean markdown text
- text: Plain text
- html: Raw HTML source
- links: Extract all links from the page
- original: Raw HTTP response body (binary-safe)
- assets: JSON listing of all sub-resources
- cookies: Dump all browser cookies

Use --eval to execute custom JavaScript on the page.
Use --stealth to bypass bot detection."#
    }
    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for ObscuraFetchTool {
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
        let mut cmd = String::from("obscura fetch");

        // --dump (default: markdown)
        let dump = input.dump.unwrap_or(DumpMode::Markdown);
        cmd.push_str(&format!(" --dump {}", dump));

        // URL
        cmd.push_str(&format!(" {}", shell_escape(&input.url)));

        // --selector
        if let Some(sel) = &input.wait_for {
            cmd.push_str(&format!(" --selector {}", shell_escape(sel)));
        }

        // --wait (default 5)
        if let Some(w) = input.wait {
            cmd.push_str(&format!(" --wait {}", w));
        }

        // --wait-until
        if let Some(wu) = input.wait_until {
            cmd.push_str(&format!(" --wait-until {}", wu));
        }

        // --timeout
        if let Some(t) = input.timeout {
            cmd.push_str(&format!(" --timeout {}", t));
        }

        // --eval
        if let Some(js) = &input.eval {
            cmd.push_str(&format!(" -e {}", shell_escape(js)));
        }

        // --stealth
        if input.stealth.unwrap_or(false) {
            cmd.push_str(" --stealth");
        }

        // --user-agent
        if let Some(ua) = &input.user_agent {
            cmd.push_str(&format!(" --user-agent {}", shell_escape(ua)));
        }

        // --proxy
        if let Some(p) = &input.proxy {
            cmd.push_str(&format!(" --proxy {}", shell_escape(p)));
        }

        // --quiet
        cmd.push_str(" -q");

        let default_timeout = input.timeout.unwrap_or(30);
        run_obscura(cmd, &ctx, Some(default_timeout), 30).await
    }
}

// ───────────────────────────────────────────────────────────────────────────
// ObscuraScrapeTool  (id: "obscura_scrape")
// ───────────────────────────────────────────────────────────────────────────

/// Output format for `obscura scrape`.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScrapeFormat {
    /// JSON output (default).
    Json,
    /// Plain text output.
    Text,
}

impl std::fmt::Display for ScrapeFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json => write!(f, "json"),
            Self::Text => write!(f, "text"),
        }
    }
}

/// Input for `ObscuraScrapeTool` (id: "obscura_scrape").
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ObscuraScrapeInput {
    /// URLs to scrape in batch.
    #[schemars(description = "The URLs to scrape in batch.")]
    pub urls: Vec<String>,

    /// JavaScript expression to evaluate on each page.
    #[schemars(description = "JavaScript expression to evaluate on each page.")]
    pub eval: Option<String>,

    /// Number of concurrent fetches. Default: 10.
    #[schemars(description = "Number of concurrent fetches. Default: 10.")]
    pub concurrency: Option<u64>,

    /// Output format. Default: json.
    #[schemars(description = "Output format. Default: json.")]
    pub format: Option<ScrapeFormat>,

    /// Timeout in seconds per URL. Default: 60.
    #[schemars(description = "Timeout in seconds per URL. Default: 60.")]
    pub timeout: Option<u64>,

    /// Enable stealth mode (anti-bot fingerprinting).
    #[schemars(description = "Enable stealth mode (anti-bot detection).")]
    pub stealth: Option<bool>,

    /// Proxy URL.
    #[schemars(description = "HTTP proxy URL.")]
    pub proxy: Option<String>,
}

/// Native tool that wraps `obscura scrape` for batch URL scraping.
#[derive(Debug, Default)]
pub struct ObscuraScrapeTool;

impl crate::types::tool_metadata::ToolMetadata for ObscuraScrapeTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Browser
    }
    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }
    fn description_template(&self) -> &str {
        "Scrape multiple URLs in batch using a headless browser. \
         Renders each page with JavaScript, supports concurrent fetching, \
         custom JS evaluation, and stealth mode for bot-protected sites."
    }
    fn requires_expr(&self) -> Expr<ToolRequirement> {
        Expr::True
    }
}

impl xai_tool_runtime::Tool for ObscuraScrapeTool {
    type Args = ObscuraScrapeInput;
    type Output = BashOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new("obscura_scrape").expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            "obscura_scrape",
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

    #[tracing::instrument(name = "tool.obscura_scrape", skip_all, fields(url_count = %input.urls.len()))]
    async fn run(
        &self,
        ctx: xai_tool_runtime::ToolCallContext,
        input: ObscuraScrapeInput,
    ) -> Result<BashOutput, xai_tool_runtime::ToolError> {
        if input.urls.is_empty() {
            return Err(xai_tool_runtime::ToolError::invalid_arguments(
                xai_tool_protocol::ToolId::new("obscura_scrape").expect("valid tool id"),
                "At least one URL is required",
            ));
        }

        let mut cmd = String::from("obscura scrape");

        // --eval
        if let Some(js) = &input.eval {
            cmd.push_str(&format!(" -e {}", shell_escape(js)));
        }

        // --concurrency
        let concurrency = input.concurrency.unwrap_or(10);
        cmd.push_str(&format!(" --concurrency {}", concurrency));

        // --format
        if let Some(f) = input.format {
            cmd.push_str(&format!(" --format {}", f));
        }

        // --timeout
        if let Some(t) = input.timeout {
            cmd.push_str(&format!(" --timeout {}", t));
        }

        // --stealth
        if input.stealth.unwrap_or(false) {
            cmd.push_str(" --stealth");
        }

        // --proxy
        if let Some(p) = &input.proxy {
            cmd.push_str(&format!(" --proxy {}", shell_escape(p)));
        }

        // URLs
        for url in &input.urls {
            cmd.push_str(&format!(" {}", shell_escape(url)));
        }

        // --quiet
        cmd.push_str(" -q");

        let timeout = input.timeout.unwrap_or(60);
        run_obscura(cmd, &ctx, Some(timeout), 60).await
    }
}

// ── Backward-compat alias ──────────────────────────────────────────────────

/// Backward-compatible alias: the enhanced fetch tool.
/// Previously `ObscuraTool` was the only tool; this type alias preserves
/// existing callers (`registry/types.rs`, `agent/builder.rs`).
pub type ObscuraTool = ObscuraFetchTool;

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::tool_metadata::test_ctx_with_call_id;

    #[test]
    fn browser_tool_name_and_description() {
        let tool = ObscuraFetchTool;
        assert_eq!(
            xai_tool_runtime::Tool::id(&tool).as_str(),
            "browser"
        );
        assert_eq!(
            crate::types::tool_metadata::ToolMetadata::kind(&tool),
            ToolKind::Browser
        );
        assert!(
            crate::types::tool_metadata::ToolMetadata::description_template(&tool)
                .contains("Render a URL in a headless browser")
        );
    }

    #[test]
    fn scrape_tool_name_and_description() {
        let tool = ObscuraScrapeTool;
        assert_eq!(
            xai_tool_runtime::Tool::id(&tool).as_str(),
            "obscura_scrape"
        );
        assert_eq!(
            crate::types::tool_metadata::ToolMetadata::kind(&tool),
            ToolKind::Browser
        );
        assert!(
            crate::types::tool_metadata::ToolMetadata::description_template(&tool)
                .contains("Scrape multiple URLs in batch")
        );
    }

    #[tokio::test]
    async fn browser_errors_when_terminal_not_in_resources() {
        let resources = crate::types::resources::Resources::new();
        let tool = ObscuraFetchTool;
        let result = xai_tool_runtime::Tool::run(
            &tool,
            test_ctx_with_call_id(resources.into_shared(), "test-call"),
            ObscuraInput {
                url: "https://example.com".into(),
                dump: None,
                wait_for: None,
                timeout: None,
                wait: None,
                wait_until: None,
                eval: None,
                stealth: None,
                user_agent: None,
                proxy: None,
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

    #[tokio::test]
    async fn scrape_errors_when_terminal_not_in_resources() {
        let resources = crate::types::resources::Resources::new();
        let tool = ObscuraScrapeTool;
        let result = xai_tool_runtime::Tool::run(
            &tool,
            test_ctx_with_call_id(resources.into_shared(), "test-call"),
            ObscuraScrapeInput {
                urls: vec!["https://example.com".into()],
                eval: None,
                concurrency: None,
                format: None,
                timeout: None,
                stealth: None,
                proxy: None,
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

    #[tokio::test]
    async fn scrape_errors_when_no_urls() {
        let resources = crate::types::resources::Resources::new();
        let tool = ObscuraScrapeTool;
        let result = xai_tool_runtime::Tool::run(
            &tool,
            test_ctx_with_call_id(resources.into_shared(), "test-call"),
            ObscuraScrapeInput {
                urls: vec![],
                eval: None,
                concurrency: None,
                format: None,
                timeout: None,
                stealth: None,
                proxy: None,
            },
        )
        .await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("At least one URL is required"),
            "Expected 'At least one URL is required' error, got: {err_msg}"
        );
    }
}
