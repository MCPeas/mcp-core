// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Integration-test harness for consumers: spawn your MCP server binary and drive it with a
//! real rmcp client, so the "an MCP client can connect and call my tools" contract is verified
//! end to end over both transports (Streamable HTTP, as Claude Code uses, and stdio).
//!
//! Gated by the `test-harness` feature; enable it as a dev-dependency. The spawned binary must
//! speak [`crate::ServerArgs`]: [`connect_stdio`] injects `--stdio`, [`connect_http`] injects
//! `--mcp --http-port <free>`. Pass data/index and any other flags as `extra_args`.
//!
//! ```ignore
//! let mcp = mcp_core::testing::connect_stdio(
//!     env!("CARGO_BIN_EXE_my-server"),
//!     &["--data", "./data"],
//! )
//! .await?;
//! assert!(mcp.tools().await?.iter().any(|t| t == "search"));
//!
//! // Evaluate the result, not just its existence: most servers return their structured output
//! // as a single JSON text block, so `call_json` extracts it for assertions by key, and
//! // `call_as` deserializes it into a type.
//! let out = mcp.call_json("search", serde_json::json!({ "q": "beta" })).await?;
//! assert!(out["results"].as_array().is_some_and(|r| !r.is_empty()));
//! ```

use std::time::{Duration, Instant};

use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::service::{RoleClient, RunningService};
use rmcp::transport::{ConfigureCommandExt, StreamableHttpClientTransport, TokioChildProcess};
use rmcp::ServiceExt;
use serde::de::DeserializeOwned;
use tokio::process::{Child, Command};

/// Boxed error so the harness pulls in no error-handling dependency of its own.
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// A connected MCP client driving a spawned server. The server process is killed on drop.
pub struct McpTestClient {
    client: RunningService<RoleClient, ()>,
    // Some for the http transport (we spawn and own the process, kill_on_drop); None for stdio,
    // where TokioChildProcess owns the child inside `client`.
    _child: Option<Child>,
}

impl McpTestClient {
    /// The names of all advertised tools.
    pub async fn tools(&self) -> Result<Vec<String>> {
        let tools = self.client.list_all_tools().await?;
        Ok(tools.into_iter().map(|t| t.name.to_string()).collect())
    }

    /// Call a tool with JSON arguments and return the raw result.
    pub async fn call(&self, name: &str, args: serde_json::Value) -> Result<CallToolResult> {
        let mut params = CallToolRequestParams::new(name.to_string());
        if let Some(arguments) = args.as_object() {
            params = params.with_arguments(arguments.clone());
        }
        Ok(self.client.call_tool(params).await?)
    }

    /// Call a tool and return a parsing-aware [`ToolOutcome`] (the raw result plus JSON
    /// conveniences for evaluating it).
    pub async fn call_tool(&self, name: &str, args: serde_json::Value) -> Result<ToolOutcome> {
        Ok(ToolOutcome {
            result: self.call(name, args).await?,
        })
    }

    /// Call a tool and return its payload as a [`serde_json::Value`] -- the common "assert on
    /// fields by key" path. See [`ToolOutcome::json`] for how the payload is sourced.
    pub async fn call_json(
        &self,
        name: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.call_tool(name, args).await?.json()
    }

    /// Call a tool and deserialize its payload into `T`. See [`ToolOutcome::parse`].
    pub async fn call_as<T: DeserializeOwned>(
        &self,
        name: &str,
        args: serde_json::Value,
    ) -> Result<T> {
        self.call_tool(name, args).await?.parse()
    }

    /// The underlying rmcp client, for assertions the convenience methods do not cover.
    pub fn client(&self) -> &RunningService<RoleClient, ()> {
        &self.client
    }
}

/// The outcome of a tool call: the raw [`CallToolResult`] plus conveniences for evaluating it.
///
/// Tools built on this framework return their structured output as a single JSON text block
/// (`Content::json(..)`), so [`json`](Self::json) / [`parse`](Self::parse) extract and deserialize
/// that payload, and [`is_domain_error`](Self::is_domain_error) understands the in-band
/// `{ "found": false, "error": ".." }` convention servers use to report failures. Derefs to the
/// inner [`CallToolResult`] for direct access to `content` / `is_error` / `structured_content`.
pub struct ToolOutcome {
    result: CallToolResult,
}

impl ToolOutcome {
    /// The raw result, by value.
    pub fn into_result(self) -> CallToolResult {
        self.result
    }

    /// The concatenated text of every text content block (usually exactly one).
    pub fn text(&self) -> String {
        CallToolResultExt::text(&self.result)
    }

    /// The tool payload as a [`serde_json::Value`]. Prefers `structured_content` when the tool
    /// sets it; otherwise parses the text block as JSON. Errors (including the raw text) when the
    /// payload is not JSON -- e.g. a plain-text error message.
    pub fn json(&self) -> Result<serde_json::Value> {
        CallToolResultExt::json(&self.result)
    }

    /// The tool payload deserialized into `T`. Same source as [`json`](Self::json).
    pub fn parse<T: DeserializeOwned>(&self) -> Result<T> {
        CallToolResultExt::parse(&self.result)
    }

    /// Whether the call reports an error: the protocol `is_error` flag, OR the in-band convention
    /// (a truthy `"error"` field or `"found": false`), OR a non-JSON text payload (the plain-text
    /// failure path). Useful for "did the user actually reach the information" assertions.
    pub fn is_domain_error(&self) -> bool {
        if self.result.is_error == Some(true) {
            return true;
        }
        match self.json() {
            Ok(value) => {
                value.get("error").is_some_and(|e| !e.is_null())
                    || value.get("found").and_then(serde_json::Value::as_bool) == Some(false)
            }
            // Successful tools return JSON; a non-JSON text payload is a plain-text error.
            Err(_) => true,
        }
    }
}

impl std::ops::Deref for ToolOutcome {
    type Target = CallToolResult;
    fn deref(&self) -> &CallToolResult {
        &self.result
    }
}

/// Parsing conveniences for a raw [`CallToolResult`] (e.g. from [`McpTestClient::call`]), so the
/// escape hatch composes with the same JSON evaluation as [`ToolOutcome`].
pub trait CallToolResultExt {
    /// The concatenated text of every text content block.
    fn text(&self) -> String;
    /// The payload as JSON: prefers `structured_content`, else parses the text block.
    fn json(&self) -> Result<serde_json::Value>;
    /// The payload deserialized into `T`.
    fn parse<T: DeserializeOwned>(&self) -> Result<T>;
}

impl CallToolResultExt for CallToolResult {
    fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|c| c.as_text().map(|t| t.text.as_str()))
            .collect()
    }

    fn json(&self) -> Result<serde_json::Value> {
        if let Some(structured) = &self.structured_content {
            return Ok(structured.clone());
        }
        let text = CallToolResultExt::text(self);
        serde_json::from_str(&text)
            .map_err(|e| format!("tool result was not JSON ({e}); raw text: {text:?}").into())
    }

    fn parse<T: DeserializeOwned>(&self) -> Result<T> {
        let value = CallToolResultExt::json(self)?;
        let rendered = value.to_string();
        serde_json::from_value(value).map_err(|e| {
            format!(
                "could not deserialize tool result into {}: {e}; value: {rendered}",
                std::any::type_name::<T>()
            )
            .into()
        })
    }
}

/// Spawn `bin --stdio <extra_args>` and connect an MCP client over stdio.
pub async fn connect_stdio(bin: &str, extra_args: &[&str]) -> Result<McpTestClient> {
    let transport = TokioChildProcess::new(Command::new(bin).configure(|cmd| {
        cmd.arg("--stdio");
        cmd.args(extra_args);
    }))?;
    let client = ().serve(transport).await?;
    Ok(McpTestClient {
        client,
        _child: None,
    })
}

/// Spawn `bin --mcp --http-port <free> <extra_args>` and connect an MCP client over Streamable
/// HTTP (the same transport Claude Code uses).
pub async fn connect_http(bin: &str, extra_args: &[&str]) -> Result<McpTestClient> {
    let port = free_port()?;
    let child = Command::new(bin)
        .arg("--mcp")
        .arg("--http-port")
        .arg(port.to_string())
        .args(extra_args)
        .kill_on_drop(true)
        .spawn()?;
    wait_for_port(port).await?;
    let transport = StreamableHttpClientTransport::from_uri(format!("http://127.0.0.1:{port}/mcp"));
    let client = ().serve(transport).await?;
    Ok(McpTestClient {
        client,
        _child: Some(child),
    })
}

/// An OS-assigned free TCP port: bind `:0`, read it back, release it for the child to claim.
fn free_port() -> Result<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

/// Wait until `port` accepts connections (the server is listening), or time out.
async fn wait_for_port(port: u16) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match tokio::net::TcpStream::connect(("127.0.0.1", port)).await {
            Ok(_) => return Ok(()),
            Err(e) => {
                if Instant::now() >= deadline {
                    return Err(Box::new(e));
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::Content;

    /// Mirror how servers build a result: a single JSON text block (`Content::json`).
    fn json_result(value: serde_json::Value) -> CallToolResult {
        CallToolResult::success(vec![Content::json(value).expect("serialize json content")])
    }

    #[test]
    fn json_extracts_the_text_block() {
        let result = json_result(serde_json::json!({ "found": true, "count": 2 }));
        assert!(!result.text().is_empty());
        assert_eq!(result.json().unwrap()["count"], 2);
    }

    #[test]
    fn parse_deserializes_into_a_type() {
        #[derive(serde::Deserialize)]
        struct Out {
            count: u32,
        }
        let out: Out = json_result(serde_json::json!({ "count": 7 }))
            .parse()
            .unwrap();
        assert_eq!(out.count, 7);
    }

    #[test]
    fn json_prefers_structured_content() {
        let mut result = CallToolResult::success(vec![Content::text("not json")]);
        result.structured_content = Some(serde_json::json!({ "ok": true }));
        assert_eq!(result.json().unwrap()["ok"], true);
    }

    #[test]
    fn is_domain_error_flags_in_band_failures() {
        let ok = ToolOutcome {
            result: json_result(serde_json::json!({ "found": true })),
        };
        assert!(!ok.is_domain_error());

        let not_found = ToolOutcome {
            result: json_result(serde_json::json!({ "found": false, "error": "missing" })),
        };
        assert!(not_found.is_domain_error());
    }

    #[test]
    fn non_json_text_is_treated_as_an_error() {
        let result = CallToolResult::success(vec![Content::text("Error: bad input")]);
        assert!(result.json().is_err());
        let outcome = ToolOutcome { result };
        assert!(outcome.is_domain_error());
    }
}
