// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

import { registerRenderer } from "../shell/registry.js";
import { renderJson } from "./json.js";
import { apiBase } from "../shell/endpoints.js";
import type { ContentRef } from "../shell/types.js";

/**
 * Built-in `catalog-item` renderer: fetch an item's structured content from
 * `{api-base}/catalog/items/{id}` and show it via the JSON fallback so the data is always
 * inspectable. Loaded eagerly (not with the lazy Catalog list) because a server's search
 * hits default to a `catalog-item` ref, which can be opened from the Search view before the
 * Catalog view -- and its list module -- is ever loaded. A server can register an override
 * for a richer per-item view.
 */
registerRenderer("catalog-item", {
  async render(ref: ContentRef, host: HTMLElement): Promise<void> {
    const id = (ref as { id?: string }).id ?? "";
    const loading = document.createElement("div");
    loading.className = "p-3 text-muted";
    loading.textContent = "Loading...";
    host.replaceChildren(loading);
    try {
      const res = await fetch(`${apiBase()}/catalog/items/${encodeURIComponent(id)}`);
      if (!res.ok) {
        throw new Error(`HTTP ${res.status}`);
      }
      renderJson(host, await res.json());
    } catch (e) {
      const alert = document.createElement("div");
      alert.className = "alert alert-warning m-3";
      alert.textContent = `Could not load item "${id}": ${String(e)}`;
      host.replaceChildren(alert);
    }
  },
});
