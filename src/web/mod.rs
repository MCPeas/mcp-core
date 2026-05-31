//! Web-server harness: assemble the HTTP router (health, robots, REST API, static
//! files) and serve it on one or more listen addresses with CORS + compression.
//!
//! This module is intentionally MCP-agnostic (no `rmcp`): a consumer composes
//! [`public_router`] with [`build_web_router`] and/or its own MCP/SSE router, then
//! hands the result to [`serve`].

use std::net::{IpAddr, SocketAddr};
use std::path::Path;

use axum::{routing::get, Router};
use tokio::task::JoinSet;
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    services::ServeDir,
};

use crate::auth::TokenAuthLayer;

/// robots.txt body — deny all crawlers.
async fn robots_txt() -> &'static str {
    "User-agent: *\nDisallow: /\n"
}

/// Always-public routes: `/health` and `/robots.txt`. These never require auth and are
/// served regardless of which features (`--web`/`--sse`) are enabled.
pub fn public_router() -> Router {
    Router::new()
        .route("/health", get(|| async { "OK" }))
        .route("/robots.txt", get(robots_txt))
}

/// Build the web router: the REST API mounted at `api_base` plus static files served as
/// the fallback. When `auth_token` is set, both require Basic-Auth with the given `realm`.
///
/// Does **not** include `/health`/`/robots.txt` — merge [`public_router`] for those, so
/// they stay public and available even in an `--sse`-only server.
pub fn build_web_router(
    static_dir: &Path,
    api_router: Router,
    api_base: &str,
    auth_token: Option<&str>,
    realm: &str,
) -> Router {
    let web = Router::new()
        .nest(api_base, api_router)
        .fallback_service(ServeDir::new(static_dir));
    match auth_token {
        Some(token) => web.layer(TokenAuthLayer::with_realm(
            token.to_owned(),
            realm.to_owned(),
        )),
        None => web,
    }
}

/// Wrap a fully-assembled app with the shared layers (gzip compression, permissive CORS).
pub fn with_layers(app: Router) -> Router {
    app.layer(CompressionLayer::new()).layer(
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any),
    )
}

/// Serve `app` on every `addr:port` in `listen` concurrently, returning when the first
/// listener stops (error or shutdown). Applies [`with_layers`] and binds one TCP
/// listener per address — loopback v4+v6 by default; external addresses are an explicit
/// opt-in by the caller. Takes an owned `listen` so it can be `tokio::spawn`ed.
pub async fn serve(app: Router, listen: Vec<IpAddr>, port: u16) -> std::io::Result<()> {
    let app = with_layers(app);
    let mut tasks: JoinSet<std::io::Result<()>> = JoinSet::new();

    for ip in listen {
        let addr = SocketAddr::new(ip, port);
        match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => {
                tracing::info!("listening on http://{addr}");
                let app = app.clone();
                tasks.spawn(async move { axum::serve(listener, app).await });
            }
            // Tolerate e.g. a missing IPv6 loopback — bind what we can.
            Err(e) => tracing::warn!("could not bind {addr}: {e}"),
        }
    }

    if tasks.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AddrNotAvailable,
            "no listen address could be bound",
        ));
    }

    match tasks.join_next().await {
        Some(Ok(serve_result)) => serve_result,
        Some(Err(join_err)) => Err(std::io::Error::other(join_err)),
        None => Ok(()),
    }
}
