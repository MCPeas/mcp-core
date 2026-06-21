// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

import { registerRenderer } from "../shell/registry.js";
import type { ContentRef } from "../shell/types.js";

/**
 * Render arbitrary data as pretty-printed JSON in a `<pre>`. This is the universal
 * fallback so users can always "see the data" and verify correctness, even when a
 * server provides no custom renderer for an item.
 */
export function renderJson(host: HTMLElement, data: unknown): void {
  const pre = document.createElement("pre");
  pre.className = "mcp-json p-3 m-0 small";
  pre.textContent = JSON.stringify(data, null, 2);
  host.replaceChildren(pre);
}

registerRenderer("json", {
  render(ref: ContentRef, host: HTMLElement): void {
    // Render `data` when present, else the ref itself (so any unknown ref is inspectable).
    const data = (ref as { data?: unknown }).data ?? ref;
    renderJson(host, data);
  },
});
