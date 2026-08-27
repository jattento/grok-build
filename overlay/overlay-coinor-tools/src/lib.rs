//! `point_to_code` — a native Grok tool available only when Grok is
//! launched inside Conan Code (<https://github.com/jattento/coinor>).
//!
//! Detection: Conan Code sets `CONAN_CODE_CONTROL_SOCKET`,
//! `CONAN_CODE_CONTROL_TOKEN`, and `CONAN_CODE_SESSION_ID` in the
//! environment of the root Grok process it launches for a local
//! conversation (never for a remote one). Their absence hides this tool
//! everywhere else via `should_list`.
//!
//! Talks directly to Conan Code's private Unix-domain control socket using
//! the line-delimited JSON wire format `coinorctl` already speaks — see
//! `Coinor/Control/TerminalControlShared.swift` in the Conan Code repo for
//! the authoritative field and method names
//! (`TerminalControlContract`). There is no compiled contract shared
//! between the two repos; keep this file and that one in sync by hand.
//! See Conan Code's ADR 0019 for the full design.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use xai_grok_tools::types::tool::{ToolKind, ToolNamespace};
use xai_grok_tools::types::tool_metadata::ToolMetadata;
use xai_tool_protocol::{ToolCapabilities, ToolId, ToolScope};
use xai_tool_runtime::render::ToolOutput as ToolOutputTrait;
use xai_tool_runtime::{ListToolsContext, Tool, ToolCallContext, ToolError};
use xai_tool_types::ToolDescription;

const CONTROL_SOCKET_ENV: &str = "CONAN_CODE_CONTROL_SOCKET";
const CONTROL_TOKEN_ENV: &str = "CONAN_CODE_CONTROL_TOKEN";
const SESSION_ID_ENV: &str = "CONAN_CODE_SESSION_ID";
/// `TerminalControlContract.protocolVersion` on the Conan Code side.
const PROTOCOL_VERSION: u32 = 1;
/// `TerminalControlContract.Method.pointToCode`.
const METHOD: &str = "point-to-code";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PointToCodeInput {
    /// Path to the file to open, relative to the workspace root or absolute.
    pub file_path: String,
    /// First line of the range to highlight (1-indexed, inclusive).
    pub line_start: u32,
    /// Last line of the range to highlight (1-indexed, inclusive).
    pub line_end: u32,
    /// Short explanation shown alongside the highlighted code. Markdown.
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PointToCodeOutput {
    pub queued: bool,
}

impl ToolOutputTrait for PointToCodeOutput {}

#[derive(Debug, Default)]
pub struct PointToCodeTool;

struct Credentials {
    socket_path: String,
    token: String,
    session_id: String,
}

impl PointToCodeTool {
    fn credentials() -> Option<Credentials> {
        let socket_path = std::env::var(CONTROL_SOCKET_ENV).ok()?;
        let token = std::env::var(CONTROL_TOKEN_ENV).ok()?;
        let session_id = std::env::var(SESSION_ID_ENV).ok()?;
        if socket_path.is_empty() || token.is_empty() || session_id.is_empty() {
            return None;
        }
        Some(Credentials {
            socket_path,
            token,
            session_id,
        })
    }
}

impl ToolMetadata for PointToCodeTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Other
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::MCP
    }

    fn description_template(&self) -> &str {
        "Open a file at a specific line range in the user's Conan Code IDE tab, \
         with a short explanation of what that code does. Use this to show the \
         user exactly which code you are talking about instead of only \
         describing it in text or quoting it inline."
    }
}

impl Tool for PointToCodeTool {
    type Args = PointToCodeInput;
    type Output = PointToCodeOutput;

    fn id(&self) -> ToolId {
        ToolId::new("point_to_code").expect("valid tool id")
    }

    fn description(&self, _ctx: &ListToolsContext) -> ToolDescription {
        ToolDescription::new("point_to_code", ToolMetadata::description_template(self))
    }

    fn capabilities(&self) -> ToolCapabilities {
        ToolCapabilities {
            is_read_only: true,
            tool_scope: Some(ToolScope::Read),
            ..Default::default()
        }
    }

    /// Hides the tool everywhere except inside a Conan Code-launched
    /// process (see module docs).
    fn should_list(&self, _ctx: &ListToolsContext) -> bool {
        Self::credentials().is_some()
    }

    async fn run(
        &self,
        _ctx: ToolCallContext,
        args: PointToCodeInput,
    ) -> Result<PointToCodeOutput, ToolError> {
        let tool_id = Tool::id(self);
        let Some(credentials) = Self::credentials() else {
            return Err(ToolError::execution(
                tool_id,
                "point_to_code is only available inside Conan Code.",
            ));
        };
        let request = serde_json::json!({
            "version": PROTOCOL_VERSION,
            "method": METHOD,
            "token": credentials.token,
            "sessionID": credentials.session_id,
            "filePath": args.file_path,
            "lineStart": args.line_start,
            "lineEnd": args.line_end,
            "comment": args.comment,
        });
        // `send_request` is blocking socket I/O; keep it off the async
        // executor's worker thread (same convention as `list_dir`'s
        // `spawn_list_dir_walk`).
        tokio::task::spawn_blocking(move || send_request(&credentials.socket_path, &request))
            .await
            .map_err(|e| {
                ToolError::execution(tool_id.clone(), format!("point_to_code was cancelled: {e}"))
            })?
            .map_err(|detail| ToolError::execution(tool_id, detail))?;
        Ok(PointToCodeOutput { queued: true })
    }
}

/// One line-delimited JSON request/response over Conan Code's private
/// control socket, matching `coinorctl`'s own transport exactly (see
/// `CoinorCtl/main.swift`'s `sendRequest`).
fn send_request(socket_path: &str, request: &serde_json::Value) -> Result<(), String> {
    let mut stream = UnixStream::connect(socket_path)
        .map_err(|e| format!("could not reach Conan Code's control socket: {e}"))?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let mut body =
        serde_json::to_vec(request).map_err(|e| format!("failed to encode request: {e}"))?;
    body.push(b'\n');
    stream
        .write_all(&body)
        .map_err(|e| format!("failed to write request: {e}"))?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|e| format!("failed to shut down socket for writing: {e}"))?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|e| format!("failed to read response: {e}"))?;
    let parsed: serde_json::Value = serde_json::from_str(response.trim())
        .map_err(|e| format!("Conan Code sent an invalid response: {e}"))?;
    if parsed.get("ok").and_then(|v| v.as_bool()) == Some(true) {
        return Ok(());
    }
    let message = parsed
        .get("error")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
        .unwrap_or("Conan Code rejected the request.");
    Err(message.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    use std::sync::Mutex;

    /// `should_list`/`run` read process-global env vars, and `cargo test`
    /// runs this file's tests concurrently by default. Every test that
    /// touches `CONAN_CODE_*` env vars holds this for its whole body so
    /// they cannot interleave and read each other's values.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_env<F: FnOnce()>(vars: &[(&str, &str)], f: F) {
        let _guard = ENV_LOCK.lock().unwrap();
        for (k, v) in vars {
            set_test_env(k, v);
        }
        f();
        for (k, _) in vars {
            clear_test_env(k);
        }
    }

    /// SAFETY: every caller holds `ENV_LOCK` for the duration these vars
    /// are visible, so no other test observes a partial set/clear.
    fn set_test_env(key: &str, value: &str) {
        unsafe { std::env::set_var(key, value) };
    }

    fn clear_test_env(key: &str) {
        unsafe { std::env::remove_var(key) };
    }

    #[test]
    fn hidden_without_credentials() {
        with_env(&[], || {
            let tool = PointToCodeTool;
            assert!(!Tool::should_list(&tool, &ListToolsContext::default()));
        });
    }

    #[test]
    fn listed_once_all_three_env_vars_are_set() {
        with_env(
            &[
                (CONTROL_SOCKET_ENV, "/tmp/does-not-matter.sock"),
                (CONTROL_TOKEN_ENV, "tok"),
                (SESSION_ID_ENV, "session-1"),
            ],
            || {
                let tool = PointToCodeTool;
                assert!(Tool::should_list(&tool, &ListToolsContext::default()));
            },
        );
    }

    #[tokio::test]
    // Each `#[tokio::test]` gets its own isolated single-threaded runtime, so
    // holding this across `.await` never blocks an unrelated concurrent task.
    #[allow(clippy::await_holding_lock)]
    async fn run_sends_the_documented_wire_shape_and_reports_queued() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("control.sock");
        let listener = UnixListener::bind(&socket_path).unwrap();

        let server = std::thread::spawn(move || {
            let (mut conn, _) = listener.accept().unwrap();
            let mut buf = String::new();
            conn.read_to_string(&mut buf).unwrap();
            let value: serde_json::Value = serde_json::from_str(buf.trim()).unwrap();
            assert_eq!(value["method"], "point-to-code");
            assert_eq!(value["token"], "tok");
            assert_eq!(value["sessionID"], "session-1");
            assert_eq!(value["filePath"], "src/lib.rs");
            assert_eq!(value["lineStart"], 1);
            assert_eq!(value["lineEnd"], 3);
            conn.write_all(br#"{"ok":true,"result":{}}"#).unwrap();
        });

        set_test_env(CONTROL_SOCKET_ENV, socket_path.to_str().unwrap());
        set_test_env(CONTROL_TOKEN_ENV, "tok");
        set_test_env(SESSION_ID_ENV, "session-1");

        let tool = PointToCodeTool;
        let output = Tool::run(
            &tool,
            ToolCallContext::default(),
            PointToCodeInput {
                file_path: "src/lib.rs".to_string(),
                line_start: 1,
                line_end: 3,
                comment: None,
            },
        )
        .await
        .unwrap();
        assert!(output.queued);

        server.join().unwrap();
        clear_test_env(CONTROL_SOCKET_ENV);
        clear_test_env(CONTROL_TOKEN_ENV);
        clear_test_env(SESSION_ID_ENV);
    }

    #[tokio::test]
    // See the identical `#[allow]` above.
    #[allow(clippy::await_holding_lock)]
    async fn run_surfaces_conan_codes_rejection_as_a_tool_error() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("control.sock");
        let listener = UnixListener::bind(&socket_path).unwrap();

        let server = std::thread::spawn(move || {
            let (mut conn, _) = listener.accept().unwrap();
            let mut buf = String::new();
            conn.read_to_string(&mut buf).unwrap();
            conn.write_all(
                br#"{"ok":false,"error":{"code":"session_unavailable","message":"nope"}}"#,
            )
            .unwrap();
        });

        set_test_env(CONTROL_SOCKET_ENV, socket_path.to_str().unwrap());
        set_test_env(CONTROL_TOKEN_ENV, "tok");
        set_test_env(SESSION_ID_ENV, "session-1");

        let tool = PointToCodeTool;
        let err = Tool::run(
            &tool,
            ToolCallContext::default(),
            PointToCodeInput {
                file_path: "src/lib.rs".to_string(),
                line_start: 1,
                line_end: 1,
                comment: None,
            },
        )
        .await
        .unwrap_err();
        assert!(err.detail.contains("nope"));

        server.join().unwrap();
        clear_test_env(CONTROL_SOCKET_ENV);
        clear_test_env(CONTROL_TOKEN_ENV);
        clear_test_env(SESSION_ID_ENV);
    }
}
