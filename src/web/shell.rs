// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The build-time-baked Lit + Bootstrap UI shell, served as static files under `/ui/`, plus
//! the app composition that ties together the public landing/login page, the shell, the REST
//! API, and an MCP router under one auth policy. See `build.rs` (the shell is compiled into
//! `OUT_DIR` and its path exported as `MCP_UI_DIST`).

use std::path::PathBuf;

use axum::{
    middleware::from_fn_with_state,
    routing::{get, post},
    Router,
};
use tower_http::services::ServeDir;

use super::{info_page, login, logout, public_router, require_auth, Landing, WebAuth};

/// Absolute path to the compiled shell dist baked by `build.rs` into `OUT_DIR`. Contains
/// `index.html` (with the inlined import map), `app.js` + the compiled `shell/` / `renderers/`
/// and `api/mcp-ui.js`, `styles.css`, and the vendored `web_modules/` - all referenced under
/// `/ui/`, where this dist is mounted.
pub fn shell_dir() -> PathBuf {
    PathBuf::from(env!("MCP_UI_DIST"))
}

/// Assemble the whole app and protect it as a single unit:
/// - `/` -> the public landing/info page ([`Landing`]), plus `/login` and `/logout`;
/// - `/ui/*` -> the baked shell (gated);
/// - `{api_base}/*` -> `api_router` (gated);
/// - `extra` -> e.g. your `/mcp` Streamable HTTP router (gated).
///
/// When `auth_token` is `Some`, one check guards `/ui`, the API and MCP together: a valid
/// session cookie (set by `POST /login` after the token check) OR an `Authorization`
/// Bearer/Basic header (so programmatic MCP clients authenticate without a cookie). Browser
/// navigations that fail the check are redirected to `/` (the login page); API/MCP callers get
/// `401`. `/`, `/login`, `/logout`, `/health` and `/robots.txt` stay public; with `None`,
/// everything is open.
pub fn app_router(
    api_router: Router,
    api_base: &str,
    extra: Router,
    auth_token: Option<&str>,
    landing: Landing,
) -> Router {
    let auth = WebAuth::new(auth_token);
    let landing = Landing {
        auth_required: auth.auth_required(),
        ..landing
    };

    // Gated tree: REST API, the MCP/consumer routers, and the shell at /ui/.
    let protected = Router::new()
        .nest(api_base, api_router)
        .merge(extra)
        .nest_service("/ui", ServeDir::new(shell_dir()))
        .layer(from_fn_with_state(auth.clone(), require_auth));

    // Public: the landing/login page + login/logout, plus /health and /robots.txt.
    let public = Router::new()
        .route("/", get(info_page))
        .with_state(landing)
        .merge(
            Router::new()
                .route("/login", post(login))
                .route("/logout", post(logout))
                .with_state(auth),
        )
        .merge(public_router());

    public.merge(protected)
}

/// Convenience: [`app_router`] with no `extra` router - the landing page, the shell, and the
/// REST API, but no MCP endpoint.
pub fn shell_router(
    api_router: Router,
    api_base: &str,
    auth_token: Option<&str>,
    landing: Landing,
) -> Router {
    app_router(api_router, api_base, Router::new(), auth_token, landing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    /// The baked shell dist exists and contains the rendered index.html + the mcp-ui API.
    #[test]
    fn shell_dir_is_populated() {
        let dir = shell_dir();
        assert!(dir.join("index.html").is_file(), "index.html baked");
        assert!(dir.join("api/mcp-ui.js").is_file(), "mcp-ui API baked");
        assert!(dir.join("web_modules/lit").is_dir(), "lit vendored");
    }

    async fn status(app: Router, uri: &str, auth: Option<&str>) -> StatusCode {
        let mut req = Request::builder().uri(uri);
        if let Some(value) = auth {
            req = req.header("Authorization", value);
        }
        app.oneshot(req.body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status()
    }

    fn demo_app(auth: Option<&str>) -> Router {
        use axum::routing::get;
        let mcp = Router::new().route("/mcp", get(|| async { "mcp" }));
        app_router(
            Router::new(),
            "/api",
            mcp,
            auth,
            Landing::new("test").mcp(true),
        )
    }

    /// `/` is the public landing page and advertises the `/mcp` endpoint.
    #[tokio::test]
    async fn landing_is_public_and_lists_mcp() {
        let res = demo_app(Some("secret"))
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = axum::body::to_bytes(res.into_body(), 64 * 1024)
            .await
            .unwrap();
        assert!(String::from_utf8_lossy(&body).contains("/mcp"));
    }

    /// With a token set, `/ui/`, `/mcp` and the API are gated; `/health` stays public; a valid
    /// Bearer header authenticates (the cookie-less programmatic-client path).
    #[tokio::test]
    async fn token_gates_ui_and_mcp_but_not_health() {
        assert_eq!(
            status(demo_app(Some("secret")), "/mcp", None).await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            status(demo_app(Some("secret")), "/mcp", Some("Bearer secret")).await,
            StatusCode::OK
        );
        assert_eq!(
            status(demo_app(Some("secret")), "/ui/", None).await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            status(demo_app(Some("secret")), "/ui/", Some("Bearer secret")).await,
            StatusCode::OK
        );
        assert_eq!(
            status(demo_app(Some("secret")), "/health", None).await,
            StatusCode::OK
        );
    }

    /// With no token, the shell at `/ui/` is open.
    #[tokio::test]
    async fn no_token_leaves_ui_open() {
        assert_eq!(status(demo_app(None), "/ui/", None).await, StatusCode::OK);
    }

    fn login_request(form: &'static str) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri("/login")
            .header("content-type", "application/x-www-form-urlencoded")
            .body(Body::from(form))
            .unwrap()
    }

    /// The session-login flow a consumer relies on when a token is set: `POST /login` with the
    /// token sets a signed cookie that then authenticates the gated shell with no `Authorization`
    /// header (the browser path); a wrong token is rejected.
    #[tokio::test]
    async fn login_sets_session_cookie_that_authenticates_ui() {
        let app = demo_app(Some("secret"));

        // No credentials -> rejected.
        assert_eq!(
            status(app.clone(), "/ui/", None).await,
            StatusCode::UNAUTHORIZED
        );

        // Wrong token -> not logged in.
        let res = app
            .clone()
            .oneshot(login_request("token=wrong"))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        // Right token -> redirect + a session cookie.
        let res = app
            .clone()
            .oneshot(login_request("token=secret"))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        let cookie = res
            .headers()
            .get("set-cookie")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(';').next())
            .expect("login sets a session cookie")
            .to_owned();
        assert!(cookie.starts_with("mcp_session="));

        // The cookie alone authenticates the gated shell (no Authorization header).
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/ui/")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }
}
