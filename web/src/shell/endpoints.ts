// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

// Shared accessors for the shell's runtime configuration, read from the <meta> tags rendered
// into index.html (see web/src/index.html.tera). Centralized here so every element resolves
// the API base / MCP endpoint / markdown capability the same way -- no copy-pasted helpers.

/** A `<meta name>` content value with a trailing slash trimmed, or `fallback` when absent. */
function metaContent(name: string, fallback: string): string {
  const meta = document.querySelector(`meta[name="${name}"]`);
  return (meta?.getAttribute("content") ?? fallback).replace(/\/+$/, "");
}

/** Base URL of the REST API (default `/api`), from the `mcp-ui-api-base` meta tag. */
export function apiBase(): string {
  return metaContent("mcp-ui-api-base", "/api");
}

/** MCP endpoint (Streamable HTTP), from the `mcp-ui-mcp-endpoint` meta tag (default `/mcp`). */
export function mcpEndpoint(): string {
  return metaContent("mcp-ui-mcp-endpoint", "/mcp");
}

/** Whether the shell was built with markdown rendering (the crate's `web-markdown` feature):
 *  the `mcp-ui-markdown` meta tag is present and "true". Off by default, so the `marked`
 *  dependency is only vendored/imported when a server opts in. */
export function markdownEnabled(): boolean {
  return document.querySelector('meta[name="mcp-ui-markdown"]')?.getAttribute("content") === "true";
}

/** Whether a URL is reachable (a 2xx to a GET). Used to show built-in views only when the
 *  server actually backs them; never throws. */
export async function isReachable(url: string): Promise<boolean> {
  try {
    return (await fetch(url)).ok;
  } catch {
    return false;
  }
}
