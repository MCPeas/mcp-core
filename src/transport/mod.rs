//! HTTP transports for MCP.
//!
//! Two transports live here, independently feature-gated and mountable side-by-side on a
//! single port:
//! - [`sse`] (feature `sse`): the legacy two-endpoint HTTP+SSE transport (`GET /sse` +
//!   `POST /message`), hand-rolled so it can be wrapped with auth middleware.
//! - [`streamable_http`] (feature `streamable-http`): the modern single-endpoint
//!   Streamable HTTP transport (`/mcp`) via rmcp's `StreamableHttpService`.

#[cfg(feature = "sse")]
mod sse;
#[cfg(feature = "sse")]
pub use sse::{AuthSseServer, SseTransport};

#[cfg(feature = "streamable-http")]
mod streamable_http;
#[cfg(feature = "streamable-http")]
pub use streamable_http::streamable_http_router;
