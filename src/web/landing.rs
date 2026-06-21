// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The public landing/info page served at `/`.
//!
//! It advertises the MCP transport endpoints the server actually mounted (decided at runtime
//! by CLI/env, described by [`Landing`]), shows the credential to send when auth is required,
//! and offers either a login form (auth required) or a button into the UI at `/ui/`.

use axum::{extract::State, http::HeaderMap, response::Html};

/// What the landing page advertises: a display name and which transports are mounted.
/// `auth_required` is filled in by [`super::app_router`] from the configured token.
#[derive(Clone)]
pub struct Landing {
    /// Server display name.
    pub name: String,
    /// Streamable HTTP `/mcp` is mounted.
    pub mcp: bool,
    /// Legacy HTTP+SSE (`/sse` + `/message`) is mounted.
    pub sse: bool,
    /// Whether a token is required (drives the login form vs the open button).
    pub auth_required: bool,
}

impl Landing {
    /// A descriptor for a server called `name`, advertising no transports yet.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            mcp: false,
            sse: false,
            auth_required: false,
        }
    }

    /// Advertise the Streamable HTTP `/mcp` endpoint.
    pub fn mcp(mut self, on: bool) -> Self {
        self.mcp = on;
        self
    }

    /// Advertise the legacy `/sse` + `/message` endpoints.
    pub fn sse(mut self, on: bool) -> Self {
        self.sse = on;
        self
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Best-effort public origin from the request (honours `X-Forwarded-Proto` behind a proxy).
fn origin(headers: &HeaderMap) -> String {
    let host = headers
        .get(axum::http::header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost");
    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("http");
    format!("{scheme}://{host}")
}

/// The landing/login page stylesheet, compiled from `landing.scss` by `build.rs` (web_modules) and
/// inlined into the page below -- so `/` stays self-contained, never loading the `/ui/` shell bundle.
const STYLE: &str = include_str!(env!("MCP_LANDING_CSS"));

/// `GET /`: render the landing/info page.
pub async fn info_page(State(landing): State<Landing>, headers: HeaderMap) -> Html<String> {
    let o = origin(&headers);
    let name = escape(&landing.name);

    let mut transports = String::new();
    if landing.mcp {
        let header_arg = if landing.auth_required {
            " \\\n  --header \"Authorization: Bearer $AUTH_TOKEN\""
        } else {
            ""
        };
        transports.push_str(&format!(
            "<h2>MCP over Streamable HTTP</h2><p>Endpoint: <code>{o}/mcp</code></p>\
             <pre>claude mcp add --transport http {name} {o}/mcp{header_arg}</pre>"
        ));
    }
    if landing.sse {
        transports.push_str(&format!(
            "<h2>MCP over legacy HTTP+SSE</h2><p>Stream: <code>{o}/sse</code><br>\
             Messages: <code>{o}/message</code></p>"
        ));
    }
    if !landing.mcp && !landing.sse {
        transports.push_str("<p class=\"muted\">No MCP transport is enabled on this server.</p>");
    }

    let access = if landing.auth_required {
        "<h2>Sign in</h2><p>This server requires a token (its <code>AUTH_TOKEN</code>). \
         MCP clients send it as <code>Authorization: Bearer &lt;token&gt;</code>; in the \
         browser, sign in to open the UI:</p>\
         <form method=\"post\" action=\"/login\">\
         <input type=\"password\" name=\"token\" placeholder=\"token\" autofocus>\
         <button type=\"submit\">Sign in</button></form>"
            .to_string()
    } else {
        "<h2>Web UI</h2><p><a class=\"btn\" href=\"/ui/\">Open the UI</a></p>".to_string()
    };

    Html(format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>{name}</title><style>{STYLE}</style></head><body>\
         <h1>{name}</h1><p class=\"muted\">MCP server</p>{transports}{access}</body></html>"
    ))
}
