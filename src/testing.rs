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
//! let out = mcp.call("search", serde_json::json!({ "q": "x" })).await?;
//! assert!(!out.content.is_empty());
//! ```

use std::time::{Duration, Instant};

use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::service::{RoleClient, RunningService};
use rmcp::transport::{ConfigureCommandExt, StreamableHttpClientTransport, TokioChildProcess};
use rmcp::ServiceExt;
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

    /// The underlying rmcp client, for assertions the convenience methods do not cover.
    pub fn client(&self) -> &RunningService<RoleClient, ()> {
        &self.client
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
