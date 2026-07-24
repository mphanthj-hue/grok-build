//! Browser session management — spawns `obscura mcp` as a managed subprocess
//! and dynamically registers all ~35 browser automation tools as native tools.
//!
//! # Usage
//!
//! ```ignore
//! // After toolset finalization:
//! let session = register_obscura_tools(&toolset, false).await?;
//! resources.insert(session);
//! ```

use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;

use crate::registry::types::FinalizedToolset;
use crate::types::output::{MCPOutput, ToolOutput};
use crate::types::tool::{ToolKind, ToolNamespace};
use crate::types::tool_metadata::ToolMetadata;

// ── Constants ──────────────────────────────────────────────────────────────

/// Default `obscura` binary name (PATH lookup).
const OBSCURA_BIN: &str = "obscura";

/// Timeout for JSON-RPC responses from Obscura.
const RPC_TIMEOUT: Duration = Duration::from_secs(120);

// ── Tool definition ────────────────────────────────────────────────────────

/// A tool definition discovered from Obscura's `tools/list`.
#[derive(Debug, Clone)]
pub struct ObscuraToolDef {
    /// Tool name (e.g. `"browser_navigate"`).
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// JSON Schema for tool arguments.
    pub input_schema: Value,
}

// ── Browser Session ────────────────────────────────────────────────────────

/// Manages a persistent `obscura mcp` subprocess.
///
/// Sends JSON-RPC requests over stdio and reads responses.
/// Wrapped in `Arc<Mutex<>>` for shared access across tools.
pub struct BrowserSession {
    child: Option<tokio::process::Child>,
    stdin: Mutex<tokio::process::ChildStdin>,
    stdout: Mutex<BufReader<tokio::process::ChildStdout>>,
    next_id: std::sync::atomic::AtomicU64,
}

impl std::fmt::Debug for BrowserSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserSession")
            .field("alive", &self.child.is_some())
            .finish()
    }
}

impl BrowserSession {
    /// Spawn `obscura mcp` (stdio mode) and return a shared session handle.
    ///
    /// `stealth` enables anti-bot fingerprinting when `true`.
    pub async fn spawn(stealth: bool) -> Result<Arc<Mutex<Self>>, String> {
        let mut cmd = Command::new(OBSCURA_BIN);
        cmd.arg("mcp");
        if stealth {
            cmd.arg("--stealth");
        }
        cmd.kill_on_drop(true);

        let mut child = cmd
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to spawn `obscura mcp`: {e}"))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Failed to open stdin on obscura process".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Failed to open stdout on obscura process".to_string())?;

        let session = Arc::new(Mutex::new(Self {
            child: Some(child),
            stdin: Mutex::new(stdin),
            stdout: Mutex::new(BufReader::new(stdout)),
            next_id: std::sync::atomic::AtomicU64::new(1),
        }));

        // Send `initialize` — required by MCP protocol before any other call.
        let init_result: Value = session
            .lock()
            .await
            .send_request(
                "initialize",
                serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": {
                        "name": "grok-build",
                        "version": "0.1"
                    }
                }),
            )
            .await?;

        // Verify the response has protocolVersion
        if init_result.get("protocolVersion").is_none() {
            return Err("Obscura MCP initialization failed: unexpected response".to_string());
        }

        Ok(session)
    }

    /// Query `tools/list` to discover all available Obscura MCP tools.
    pub async fn list_tools(&mut self) -> Result<Vec<ObscuraToolDef>, String> {
        let result: Value = self.send_request("tools/list", Value::Null).await?;

        let tools_array = result
            .get("tools")
            .and_then(Value::as_array)
            .ok_or_else(|| "Obscura tools/list: missing 'tools' array".to_string())?;

        let tools = tools_array
            .iter()
            .map(|t| ObscuraToolDef {
                name: t
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
                description: t
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                input_schema: t
                    .get("inputSchema")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({"type": "object", "properties": {}})),
            })
            .collect();

        Ok(tools)
    }

    /// Call a remote tool and return its JSON result.
    pub async fn call_tool(&mut self, tool_name: &str, args: Value) -> Result<Value, String> {
        let params = serde_json::json!({
            "name": tool_name,
            "arguments": args,
        });
        let result: Value = self.send_request("tools/call", params).await?;

        // MCP tool return can have content array, isError, etc.
        if result
            .get("isError")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            let error_msg = result
                .get("content")
                .and_then(Value::as_array)
                .and_then(|arr| arr.first())
                .and_then(|c| c.get("text"))
                .and_then(Value::as_str)
                .unwrap_or("Unknown error");
            return Err(format!("Obscura tool '{tool_name}' failed: {error_msg}"));
        }

        Ok(result)
    }

    /// Shutdown the browser session (kill the subprocess).
    pub fn shutdown(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
        }
    }

    // ── Internal: JSON-RPC ──────────────────────────────────────────────

    /// Send a JSON-RPC request and read the response.
    async fn send_request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        // Write request (newline-delimited JSON)
        let mut body = serde_json::to_string(&request)
            .map_err(|e| format!("JSON serialization error: {e}"))?;
        body.push('\n');

        {
            let mut stdin = self.stdin.lock().await;
            stdin
                .write_all(body.as_bytes())
                .await
                .map_err(|e| format!("Failed to write to obscura stdin: {e}"))?;
            stdin
                .flush()
                .await
                .map_err(|e| format!("Failed to flush obscura stdin: {e}"))?;
        }

        // Read response (newline-delimited JSON)
        {
            let mut stdout = self.stdout.lock().await;
            let mut line = String::new();

            // Use a timeout for reading the response
            let read_result = tokio::time::timeout(RPC_TIMEOUT, stdout.read_line(&mut line)).await;

            match read_result {
                Ok(Ok(0)) => {
                    return Err("Obscura MCP: connection closed (process exited)".to_string());
                }
                Ok(Ok(_n)) => {
                    // Successfully read a line
                }
                Ok(Err(e)) => {
                    return Err(format!("Failed to read from obscura stdout: {e}"));
                }
                Err(_) => {
                    return Err(format!(
                        "Timeout reading response from obscura (>{:?})",
                        RPC_TIMEOUT
                    ));
                }
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                return Err("Obscura MCP: empty response".to_string());
            }

            let response: Value = serde_json::from_str(trimmed)
                .map_err(|e| format!("JSON parse error from obscura: {e} — raw: {trimmed}"))?;

            // Check for JSON-RPC error
            if let Some(error) = response.get("error") {
                let code = error.get("code").and_then(Value::as_i64).unwrap_or(0);
                let message = error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("Unknown error");
                return Err(format!("Obscura MCP error (code {code}): {message}"));
            }

            response
                .get("result")
                .cloned()
                .ok_or_else(|| format!("Obscura MCP: response missing 'result': {response}"))
        }
    }
}

impl Drop for BrowserSession {
    fn drop(&mut self) {
        self.shutdown();
    }
}

// ── Dynamic Tool Wrapper ───────────────────────────────────────────────────

/// A native Grok Build tool that wraps an Obscura MCP tool.
///
/// Each instance corresponds to one tool discovered via `tools/list`.
/// When called, it sends a JSON-RPC `tools/call` to the shared Obscura
/// subprocess and returns the result.
#[derive(Debug)]
pub struct ObscuraDynamicTool {
    /// Tool name as known to Obscura (e.g. `"browser_navigate"`).
    pub tool_name: String,
    /// Description from Obscura's tool definition.
    pub description: String,
    /// JSON Schema for tool arguments.
    pub input_schema: Value,
    /// Shared handle to the Obscura MCP subprocess session.
    pub session: Arc<Mutex<BrowserSession>>,
}

impl ToolMetadata for ObscuraDynamicTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Browser
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        &self.description
    }
}

impl xai_tool_runtime::Tool for ObscuraDynamicTool {
    type Args = Value;
    type Output = ToolOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        // Use the tool name as registry id (e.g. "browser_navigate")
        xai_tool_protocol::ToolId::new(&self.tool_name)
            .unwrap_or_else(|_| xai_tool_protocol::ToolId::new("obscura_tool").expect("valid"))
    }

    fn description(
        &self,
        _ctx: &xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(&self.tool_name, &self.description)
    }

    fn capabilities(&self) -> xai_tool_protocol::ToolCapabilities {
        xai_tool_protocol::ToolCapabilities {
            is_read_only: true,
            tool_scope: Some(xai_tool_protocol::ToolScope::Read),
            ..Default::default()
        }
    }

    #[tracing::instrument(name = "tool.obscura", skip_all, fields(tool = %self.tool_name))]
    async fn run(
        &self,
        _ctx: xai_tool_runtime::ToolCallContext,
        input: Value,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        let mut session = self.session.lock().await;
        let result = session
            .call_tool(&self.tool_name, input)
            .await
            .map_err(|e| {
                xai_tool_runtime::ToolError::execution(
                    xai_tool_protocol::ToolId::new(&self.tool_name).unwrap_or_else(|_| {
                        xai_tool_protocol::ToolId::new("obscura").expect("valid")
                    }),
                    e,
                )
            })?;

        // Format the result as text
        let text = format_tool_result(&result);
        Ok(ToolOutput::MCP(MCPOutput::okay_output(
            self.tool_name.clone(),
            "obscura".to_string(),
            text,
        )))
    }
}

/// Extract text from an Obscura MCP tool result.
///
/// Obscura returns results in MCP `content` array format:
/// ```json
/// {"content": [{"type": "text", "text": "..."}], "isError": false}
/// ```
fn format_tool_result(result: &Value) -> String {
    if let Some(content) = result.get("content").and_then(Value::as_array) {
        let mut parts = Vec::new();
        for item in content {
            if let Some(text) = item.get("text").and_then(Value::as_str) {
                parts.push(text.to_string());
            } else if let Some(_data) = item.get("data") {
                // Could be binary data (base64-encoded image, etc.)
                if let Some(s) = _data.as_str() {
                    parts.push(format!("[data: {} bytes]", s.len()));
                }
            }
        }
        parts.join("\n")
    } else {
        // Fallback: serialize the whole result
        serde_json::to_string_pretty(result).unwrap_or_else(|_| "{}".to_string())
    }
}

// ── Registration ───────────────────────────────────────────────────────────

/// Spawn an Obscura MCP subprocess, discover all tools, and register them
/// on the given `toolset`.
///
/// Returns the shared session handle so callers can insert it into Resources
/// for other tools to use (e.g., to implement custom browser tools).
///
/// `stealth` enables anti-bot fingerprinting (passed to `obscura mcp`).
///
/// # Errors
///
/// Returns an error string if the subprocess cannot be spawned, the
/// `tools/list` query fails, or any individual tool registration fails.
pub async fn register_obscura_tools(
    toolset: &FinalizedToolset,
    stealth: bool,
) -> Result<Arc<Mutex<BrowserSession>>, String> {
    // 1. Spawn Obscura MCP subprocess
    let session = BrowserSession::spawn(stealth).await?;

    // 2. Discover all tools
    let tool_defs = {
        let mut sess = session.lock().await;
        sess.list_tools().await?
    };

    if tool_defs.is_empty() {
        return Err("Obscura tools/list returned no tools".to_string());
    }

    // 3. Register each tool on the finalized toolset
    for def in &tool_defs {
        let tool = ObscuraDynamicTool {
            tool_name: def.name.clone(),
            description: def.description.clone(),
            input_schema: def.input_schema.clone(),
            session: Arc::clone(&session),
        };

        toolset
            .register_tool(def.name.clone(), tool, Some(def.input_schema.clone()))
            .map_err(|e| format!("Failed to register Obscura tool '{}': {e}", def.name))?;

        tracing::info!(
            tool_name = %def.name,
            "Registered Obscura native tool"
        );
    }

    tracing::info!(
        count = tool_defs.len(),
        "Obscura native tools registered successfully"
    );

    Ok(session)
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_tool_result_text_content() {
        let result = serde_json::json!({
            "content": [
                {"type": "text", "text": "Hello, world!"}
            ],
            "isError": false
        });
        assert_eq!(format_tool_result(&result), "Hello, world!");
    }

    #[test]
    fn format_tool_result_multiple_contents() {
        let result = serde_json::json!({
            "content": [
                {"type": "text", "text": "Line 1"},
                {"type": "text", "text": "Line 2"}
            ],
            "isError": false
        });
        assert_eq!(format_tool_result(&result), "Line 1\nLine 2");
    }

    #[test]
    fn format_tool_result_empty_content() {
        let result = serde_json::json!({
            "content": [],
            "isError": false
        });
        assert_eq!(format_tool_result(&result), "");
    }

    #[test]
    fn format_tool_result_no_content() {
        let result = serde_json::json!({"isError": false});
        assert!(format_tool_result(&result).contains("isError"));
    }
}
