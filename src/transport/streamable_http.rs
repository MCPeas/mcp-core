//! Streamable HTTP transport for MCP (rmcp 1.7+).
//!
//! Unlike the legacy two-endpoint SSE transport ([`super::sse`]), Streamable HTTP is a
//! single, content-negotiated endpoint (conventionally `/mcp`): a client `POST`s a
//! JSON-RPC message and receives either a single JSON reply or an SSE stream, and a `GET`
//! opens a server→client SSE stream. rmcp's [`StreamableHttpService`] performs that
//! negotiation and manages the session lifecycle, so — unlike [`super::sse::AuthSseServer`]
//! — there is no transport accept-loop to drive: the returned [`Router`] is self-contained.

use axum::Router;
use rmcp::handler::server::ServerHandler;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};

/// Build an axum [`Router`] serving Streamable HTTP MCP under `base` (e.g. `"/mcp"`).
///
/// `factory` is invoked once per MCP session to mint a fresh handler, so per-session state
/// stays isolated (mirroring how [`super::sse::AuthSseServer`] spawns a handler per SSE
/// connection). The returned router can be wrapped with [`crate::web::protect`] for token
/// auth and `merge`d into the application router alongside the web/SSE routes — all on one
/// port.
///
/// Host-header (DNS-rebinding) validation is disabled here: this crate's security model is
/// bind-address + optional `AUTH_TOKEN`, and these servers commonly run behind a reverse
/// proxy or on an internal network where the `Host` is a service name (e.g.
/// `gesetze-mcp:8080`) that rmcp's default allow-list would reject with `403`. Re-enable
/// (via a custom [`StreamableHttpServerConfig`]) if `/mcp` is ever exposed publicly.
pub fn streamable_http_router<H, F>(factory: F, base: &str) -> Router
where
    F: Fn() -> H + Send + Sync + 'static,
    H: ServerHandler + Send + 'static,
{
    // `stateful_mode` (the default) issues an `Mcp-Session-Id` on initialize and tracks
    // per-session state — the behaviour OpenWebUI's native MCP client expects.
    let config = StreamableHttpServerConfig::default().disable_allowed_hosts();

    let service = StreamableHttpService::new(
        move || Ok(factory()),
        LocalSessionManager::default().into(),
        config,
    );

    Router::new().nest_service(base, service)
}
