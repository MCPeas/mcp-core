// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! MCP Core - Shared infrastructure for MCP and web servers.
//!
//! This crate provides common building blocks for MCP servers:
//!
//! - **auth**: Token-based authentication middleware (Bearer and Basic Auth)
//! - **config**: Configuration management with environment variable support
//! - **transport**: HTTP transports for MCP — legacy SSE (`/sse`) and Streamable HTTP (`/mcp`)
//! - **bootstrap**: Tracing initialization utilities
//! - **web**: Hardened axum harness; with `web-ui`, the embedded inspection shell,
//!   typed catalog and search
//! - **cli**: Shared clap flags for server binaries
//! - **testing**: Consumer test harness driving a server binary with a real MCP client
//!
//! # Features
//!
//! See the README's feature table for the complete list. `full` (the default) bundles the
//! headless server pieces: `auth`, `config`, `sse`, `streamable-http`, `stdio` and
//! `bootstrap`. The web harness (`web`, `web-ui`), `cli`, `server` and `test-harness` are
//! separate opt-ins.
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

#[cfg(any(feature = "sse", feature = "streamable-http"))]
pub mod transport;

#[cfg(feature = "bootstrap")]
pub mod bootstrap;

#[cfg(feature = "cli")]
pub mod cli;

#[cfg(feature = "web")]
pub mod web;

#[cfg(feature = "test-harness")]
pub mod testing;

// Char-boundary-safe text slicing — pure, no transport deps.
pub mod text;

// Response-size budget for MCP tool results; needs the rmcp server types that
// the transport features pull in.
#[cfg(any(feature = "sse", feature = "streamable-http", feature = "stdio"))]
pub mod budget;

// Re-exports for convenience
#[cfg(feature = "auth")]
pub use auth::{TokenAuthLayer, TokenAuthService};
#[cfg(any(feature = "sse", feature = "streamable-http", feature = "stdio"))]
pub use budget::{enforce_response_budget, BudgetedHandler, DEFAULT_MAX_RESPONSE_CHARS};

#[cfg(feature = "config")]
pub use config::{generate_random_token, safe_resolve, BaseConfig, SafePathError};

#[cfg(feature = "sse")]
pub use transport::{AuthSseServer, SseTransport};

#[cfg(feature = "streamable-http")]
pub use transport::streamable_http_router;

#[cfg(feature = "bootstrap")]
pub use bootstrap::init_tracing;

#[cfg(feature = "cli")]
pub use cli::{ServerArgs, ServerConfig};

#[cfg(feature = "web-ui")]
pub use web::{
    app_router, catalog_router, data_catalog_router, search_router, shell_dir, shell_router,
    Cardinality, CatalogAction, CatalogItem, CatalogPage, CatalogProvider, CatalogQuery,
    DataCatalog, EntityAction, EntityType, FilterToggle, Relationship, Resource, ResourceRef,
    SearchHit, SearchProvider, SearchQuery, SearchResults,
};
