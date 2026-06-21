// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Web-server harness: assemble the HTTP router (landing/login, health, robots, REST API, the
//! UI shell) and serve it on one or more listen addresses with gzip + baseline security headers.
//!
//! This module is intentionally MCP-agnostic (no `rmcp`): a consumer composes
//! [`app_router`](shell::app_router) with its own MCP/SSE router, then hands the result to
//! [`serve`].

use std::net::{IpAddr, SocketAddr};

use axum::http::{header, HeaderName, HeaderValue};
use axum::response::Response;
use axum::{routing::get, Router};
use tokio::task::JoinSet;
use tower_http::compression::CompressionLayer;

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

/// Apply token authentication to `router` when `auth_token` is `Some`; otherwise return it
/// unchanged. This is the single place the opt-in auth policy lives — `Some` requires a valid
/// Bearer token or Basic-Auth password (the Basic challenge advertises `realm`), `None` leaves
/// the router open.
///
/// A header-only check ([`TokenAuthLayer`]); [`app_router`](shell::app_router) is the usual
/// entry point and layers session-cookie auth on top.
pub fn protect(router: Router, auth_token: Option<&str>, realm: &str) -> Router {
    match auth_token {
        Some(token) => router.layer(TokenAuthLayer::with_realm(
            token.to_owned(),
            realm.to_owned(),
        )),
        None => router,
    }
}

/// Baseline CSP applied to every response. Under `web-ui` it also pins `script-src` to `'self'`
/// plus the build-time SHA-256 of the inlined import-map script (emitted by `build.rs`), so no
/// other inline script and no cross-origin script can run - the XSS lock that backs the `esm`
/// renderer's same-origin guard. Other resource types are left unconstrained here (the shell
/// loads same-origin assets); untrusted rendered content is contained by the script-free
/// sandboxed iframe (`<mcp-frame>`), not by this policy.
#[cfg(feature = "web-ui")]
const CONTENT_SECURITY_POLICY: &str = concat!(
    "script-src 'self' '",
    env!("MCP_UI_CSP_SCRIPT_HASH"),
    "'; object-src 'none'; base-uri 'self'; frame-ancestors 'none'; form-action 'self'"
);
/// Baseline CSP for non-UI `web` consumers (no shell, so no inline import map to hash).
#[cfg(not(feature = "web-ui"))]
const CONTENT_SECURITY_POLICY: &str =
    "object-src 'none'; base-uri 'self'; frame-ancestors 'none'; form-action 'self'";

/// Disable powerful browser features the shell never uses.
const PERMISSIONS_POLICY: &str = "camera=(), microphone=(), geolocation=(), payment=()";

/// Add baseline security headers to every response.
async fn set_security_headers(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CONTENT_SECURITY_POLICY),
    );
    headers.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static(PERMISSIONS_POLICY),
    );
    response
}

/// Wrap a fully-assembled app with the shared layers: baseline security headers + gzip
/// compression. No CORS layer (the shell is same-origin); a consumer needing cross-origin
/// adds its own.
pub fn with_layers(app: Router) -> Router {
    app.layer(axum::middleware::map_response(set_security_headers))
        .layer(CompressionLayer::new())
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

mod session;
pub use session::{login, logout, require_auth, WebAuth};

mod landing;
pub use landing::{info_page, Landing};

/// serde `skip_serializing_if` helper: omit a `bool` field when it is `false`, so a default
/// `false` (the common case for the catalog action/toggle flags) stays off the wire. Shared by
/// the `catalog` and `data_catalog` submodules.
#[cfg(feature = "web-ui")]
fn is_false(b: &bool) -> bool {
    !*b
}

#[cfg(feature = "web-ui")]
mod shell;
#[cfg(feature = "web-ui")]
pub use shell::{app_router, shell_dir, shell_router};

#[cfg(feature = "web-ui")]
mod catalog;
#[cfg(feature = "web-ui")]
pub use catalog::{catalog_router, CatalogAction, CatalogItem, CatalogProvider};

#[cfg(feature = "web-ui")]
mod data_catalog;
#[cfg(feature = "web-ui")]
pub use data_catalog::{
    data_catalog_router, Cardinality, CatalogPage, CatalogQuery, DataCatalog, EntityAction,
    EntityType, FilterToggle, Relationship, Resource, ResourceRef,
};

#[cfg(feature = "web-ui")]
mod search;
#[cfg(feature = "web-ui")]
pub use search::{search_router, SearchHit, SearchProvider, SearchQuery, SearchResults};

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::get,
    };
    use tower::util::ServiceExt;

    fn guarded(auth_token: Option<&str>) -> Router {
        protect(
            Router::new().route("/x", get(|| async { "OK" })),
            auth_token,
            "test-realm",
        )
    }

    async fn status(app: Router, auth: Option<&str>) -> StatusCode {
        let mut req = Request::builder().uri("/x");
        if let Some(value) = auth {
            req = req.header("Authorization", value);
        }
        app.oneshot(req.body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status()
    }

    #[tokio::test]
    async fn with_layers_adds_security_headers_and_no_wildcard_cors() {
        let app = with_layers(Router::new().route("/x", get(|| async { "OK" })));
        let res = app
            .oneshot(Request::builder().uri("/x").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let h = res.headers();
        assert_eq!(h["x-content-type-options"], "nosniff");
        assert_eq!(h["x-frame-options"], "DENY");
        assert!(h.contains_key("content-security-policy"));
        assert!(h.contains_key("referrer-policy"));
        assert!(h.contains_key("permissions-policy"));
        // The permissive wildcard CORS is gone.
        assert!(h.get("access-control-allow-origin").is_none());
    }

    #[tokio::test]
    async fn none_leaves_router_open() {
        assert_eq!(status(guarded(None), None).await, StatusCode::OK);
    }

    #[tokio::test]
    async fn some_rejects_missing_credentials() {
        assert_eq!(
            status(guarded(Some("secret")), None).await,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn some_accepts_bearer() {
        assert_eq!(
            status(guarded(Some("secret")), Some("Bearer secret")).await,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn some_accepts_basic_with_token_as_password() {
        let creds =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, "anyuser:secret");
        assert_eq!(
            status(guarded(Some("secret")), Some(&format!("Basic {creds}"))).await,
            StatusCode::OK
        );
    }
}
