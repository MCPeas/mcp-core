# Changelog

Notable changes to mcp-core.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); releases are git tags.

## [Unreleased]

### Added
- `budget`: `BudgetedHandler<H>`, a transparent `ServerHandler` wrapper that caps `call_tool` results at a character budget and returns a machine-readable `response_exceeds_budget` error (measured size, budget, recovery hint) when exceeded; plus the free `enforce_response_budget` and `DEFAULT_MAX_RESPONSE_CHARS`.
  Gated on the transport features.
  Wrap once and pass to every transport so oversized responses fail actionably instead of tripping the client's output cap.
- `text`: char-boundary-safe slicing helpers (`floor_char_boundary`, `ceil_char_boundary`, `char_safe_chunk`) for chunking large payloads without splitting UTF-8.
- CI gates: per-feature builds (cargo-hack), dependency and license audit (cargo-deny), REUSE lint.
- `rust-version` (MSRV) 1.95, Dependabot updates, and a security policy (`SECURITY.md`).

### Fixed
- The landing page escapes the reflected `Host`/`X-Forwarded-Proto` origin, and the plain-`web` CSP gains `script-src 'self'`.
- The token-auth middleware builds its 401 challenge without panicking on a header-invalid realm.
- The typed catalog clamps the client-supplied list `limit` to 500.
- The shell redirects to sign-in when the session cookie expires, surfaces renderer failures instead of a blank pane, and no longer shows German strings.

## [0.1.0] - 2026-07-06

Initial release: token/session authentication, environment configuration with safe path resolution, MCP transports (Streamable HTTP `/mcp`, legacy HTTP+SSE, stdio), the hardened web harness, the embedded schema-driven web UI (typed catalog, search, operations console), shared server CLI flags, and the consumer test harness.
