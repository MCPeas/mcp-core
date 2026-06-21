// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Web login: a SameSite=Strict signed session cookie set by `POST /login` after a
//! constant-time token check. The gating middleware accepts the cookie OR an `Authorization`
//! Bearer/Basic header, so programmatic MCP clients authenticate without a cookie.

use std::sync::Arc;

use axum::{
    body::Body,
    extract::{FromRef, State},
    http::{header, HeaderMap, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
    Form,
};
use axum_extra::extract::cookie::{Cookie, Key, SameSite, SignedCookieJar};
use subtle::ConstantTimeEq;

const COOKIE_NAME: &str = "mcp_session";

/// Shared web-auth state: the optional token plus the per-process cookie signing key.
///
/// `token` `None` leaves the app open (no login required). The signing key is random per
/// process, so a restart invalidates existing sessions (clients re-login).
#[derive(Clone)]
pub struct WebAuth {
    token: Option<Arc<str>>,
    key: Key,
}

impl WebAuth {
    /// Build web-auth from the optional configured token.
    pub fn new(token: Option<&str>) -> Self {
        Self {
            token: token.map(Arc::from),
            key: Key::generate(),
        }
    }

    /// Whether a token is configured (i.e. login is required).
    pub fn auth_required(&self) -> bool {
        self.token.is_some()
    }
}

impl FromRef<WebAuth> for Key {
    fn from_ref(auth: &WebAuth) -> Self {
        auth.key.clone()
    }
}

/// Constant-time token comparison (avoids leaking the token via timing).
fn token_eq(presented: &str, token: &str) -> bool {
    presented.as_bytes().ct_eq(token.as_bytes()).into()
}

/// Whether `headers` carries a valid `Authorization: Bearer <token>` or Basic `<user:token>`.
fn header_authorized(headers: &HeaderMap, token: &str) -> bool {
    let Some(value) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };
    if let Some(bearer) = value.strip_prefix("Bearer ") {
        return token_eq(bearer, token);
    }
    if let Some(basic) = value.strip_prefix("Basic ") {
        if let Some(decoded) = decode_basic(basic) {
            if let Some((_user, pass)) = decoded.split_once(':') {
                return token_eq(pass, token);
            }
        }
    }
    false
}

fn decode_basic(input: &str) -> Option<String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(input)
        .ok()?;
    String::from_utf8(bytes).ok()
}

/// A top-level browser navigation (gets a redirect to the login page on failure) vs an
/// API/XHR call (gets a 401).
fn is_navigation(headers: &HeaderMap) -> bool {
    if let Some(mode) = headers.get("sec-fetch-mode").and_then(|v| v.to_str().ok()) {
        return mode == "navigate";
    }
    headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|a| a.contains("text/html"))
}

/// `POST /login` body (`application/x-www-form-urlencoded`).
#[derive(serde::Deserialize)]
pub struct LoginForm {
    token: String,
}

/// `POST /login`: set the session cookie when the token matches; otherwise `401`.
pub async fn login(
    State(auth): State<WebAuth>,
    jar: SignedCookieJar,
    Form(form): Form<LoginForm>,
) -> Response {
    let Some(token) = auth.token.as_deref() else {
        return Redirect::to("/ui/").into_response();
    };
    if token_eq(&form.token, token) {
        let cookie = Cookie::build((COOKIE_NAME, "ok"))
            .http_only(true)
            .secure(true)
            .same_site(SameSite::Strict)
            .path("/")
            .build();
        (jar.add(cookie), Redirect::to("/ui/")).into_response()
    } else {
        (StatusCode::UNAUTHORIZED, "invalid token").into_response()
    }
}

/// `POST /logout`: clear the session cookie.
pub async fn logout(jar: SignedCookieJar) -> Response {
    let removal = Cookie::build(COOKIE_NAME).path("/").build();
    (jar.remove(removal), Redirect::to("/")).into_response()
}

/// Gating middleware: allow when no token is configured, when a valid session cookie is
/// present, or when a valid `Authorization` header is present. On failure, redirect browser
/// navigations to `/` (the login page) and return `401` to API/MCP callers.
pub async fn require_auth(
    State(auth): State<WebAuth>,
    jar: SignedCookieJar,
    req: Request<Body>,
    next: Next,
) -> Response {
    let Some(token) = auth.token.as_deref() else {
        return next.run(req).await;
    };
    if jar.get(COOKIE_NAME).is_some() || header_authorized(req.headers(), token) {
        return next.run(req).await;
    }
    if is_navigation(req.headers()) {
        Redirect::to("/").into_response()
    } else {
        StatusCode::UNAUTHORIZED.into_response()
    }
}
