// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The Lit + Bootstrap UI shell, **embedded into the binary** and served under `/ui/`, plus the
//! app composition that ties together the public landing/login page, the shell, the REST API, and
//! an MCP router under one auth policy. `build.rs` bakes the shell into `$OUT_DIR/web-ui-dist` (via
//! the `web_modules` toolchain); `include_dir!` then embeds that dist into the binary, so a release
//! image carrying only the binary still serves the UI — no external static dir, no `--static-dir`.
//! A `web-ui-dev` debug build instead live-compiles the shell from `web/src` (browser live-reload).

use std::path::PathBuf;

use axum::{
    middleware::from_fn_with_state,
    routing::{get, post},
    Router,
};
use include_dir::{include_dir, Dir};
use web_modules::Frontend;

/// The shell dist (`build.rs` → `$OUT_DIR/web-ui-dist`) embedded into the binary at compile time.
/// Served from memory by [`ui_router`] — nothing needs to ship beside the binary.
static SHELL_DIST: Dir<'static> = include_dir!("$OUT_DIR/web-ui-dist");

use super::{info_page, login, logout, public_router, require_auth, Landing, WebAuth};

/// Absolute path to the compiled shell dist baked by `build.rs` into `OUT_DIR`. Contains
/// `index.html` (with the inlined import map), `app.js` + the compiled `shell/` / `renderers/`
/// and `api/mcp-ui.js`, `styles.css`, and the vendored `web_modules/` - all referenced under
/// `/ui/`. This is the on-disk bake that [`SHELL_DIST`] embeds into the binary.
pub fn shell_dir() -> PathBuf {
    PathBuf::from(env!("MCP_UI_DIST"))
}

/// The shell as a nestable axum service. Release/CI (`web-ui`): served statically from the
/// in-binary [`SHELL_DIST`]. A `web-ui-dev` debug build: live-compiled from `web/src` with browser
/// live-reload (web_modules' dev server), falling back to the embedded dist. Mounted under `/ui`;
/// the shell's assets resolve at `/ui/...` because the import map is baked with that prefix.
fn ui_router() -> Router {
    Frontend::embedded(&SHELL_DIST)
        .source(concat!(env!("CARGO_MANIFEST_DIR"), "/web/src"))
        .auto()
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
        .nest_service("/ui", ui_router())
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

    /// The shell is embedded **into the binary** (not served from a build-time path), so a slim
    /// release image carrying only the binary still serves `/ui`. This is the regression guard for
    /// the 404 that the old `ServeDir`-from-`$OUT_DIR` serving produced in a multi-stage image.
    #[test]
    fn shell_is_embedded_in_the_binary() {
        assert!(
            SHELL_DIST.get_file("index.html").is_some(),
            "index.html embedded"
        );
        assert!(
            SHELL_DIST.get_file("api/mcp-ui.js").is_some(),
            "mcp-ui API embedded"
        );
        assert!(
            SHELL_DIST.get_dir("web_modules/lit").is_some(),
            "lit vendored + embedded"
        );
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
