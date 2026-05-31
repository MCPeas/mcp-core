//! MCP Core - Shared infrastructure for MCP and web servers.
//!
//! This crate provides common building blocks for MCP servers:
//!
//! - **auth**: Token-based authentication middleware (Bearer and Basic Auth)
//! - **config**: Configuration management with environment variable support
//! - **transport**: SSE transport for MCP HTTP mode
//! - **bootstrap**: Tracing initialization utilities
//!
//! # Features
//!
//! - `auth` - Token authentication middleware (enabled by default)
//! - `config` - Configuration utilities (enabled by default)
//! - `bootstrap` - Tracing setup (enabled by default)
//! - `transport` - SSE transport for MCP HTTP mode
//! - `full` - All features
//!
//! # Example
//!
//! ```rust,ignore
//! use mcp_core::{TokenAuthLayer, BaseConfig, init_tracing};
//!
//! fn main() {
//!     init_tracing("myserver=debug");
//!     let config = BaseConfig::from_env();
//!     let (token, _generated) = config.get_or_generate_token();
//!
//!     // Auth middleware
//!     let router = my_routes().layer(TokenAuthLayer::new(token));
//! }
//! ```

#[cfg(feature = "auth")]
pub mod auth;

#[cfg(feature = "config")]
pub mod config;

#[cfg(feature = "sse")]
pub mod transport;

#[cfg(feature = "bootstrap")]
pub mod bootstrap;

#[cfg(feature = "cli")]
pub mod cli;

#[cfg(feature = "web")]
pub mod web;

// Re-exports for convenience
#[cfg(feature = "auth")]
pub use auth::{TokenAuthLayer, TokenAuthService};

#[cfg(feature = "config")]
pub use config::{generate_random_token, safe_resolve, BaseConfig, SafePathError};

#[cfg(feature = "sse")]
pub use transport::{AuthSseServer, SseTransport};

#[cfg(feature = "bootstrap")]
pub use bootstrap::init_tracing;

#[cfg(feature = "cli")]
pub use cli::{ServerArgs, ServerConfig};
