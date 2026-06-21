// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

import { LitElement, html } from "lit";
import { property, state } from "lit/decorators.js";
import type { PropertyValues } from "lit";
import { getRenderer } from "./registry.js";
import { renderJson } from "../renderers/json.js";
import type { ContentRef } from "./types.js";

/** A fresh content host node. */
function newHost(): HTMLElement {
  const el = document.createElement("div");
  el.className = "mcp-content-host h-100";
  return el;
}

/**
 * Renders a {@link ContentRef} by delegating to the registered renderer for its `type`
 * (or the JSON fallback). Used by both layouts (a full view's single ref, or a list/search
 * selection's ref).
 *
 * Each ref gets a FRESH host node. Renderers populate `host` however they like -- imperative
 * DOM (iframes, dynamically-imported elements) or Lit's own `render(template, host)` -- and a
 * fresh node per ref means a renderer never inherits stale state (e.g. Lit part markers left
 * by the previously shown content, which would otherwise crash the next `render()`). Between
 * ref changes the host is stable, so a renderer's imperatively-managed children are untouched.
 */
export class ContentRouter extends LitElement {
  @property({ attribute: false }) ref: ContentRef | null = null;
  @state() host: HTMLElement = newHost();

  createRenderRoot(): this {
    return this;
  }

  updated(changed: PropertyValues): void {
    if (changed.has("ref")) {
      void this.renderContent();
    }
  }

  private async renderContent(): Promise<void> {
    // Swap in a clean host (reactive: schedules a re-render that places it), then fill it.
    const host = newHost();
    this.host = host;
    if (!this.ref) {
      return;
    }
    const renderer = getRenderer(this.ref.type);
    if (!renderer) {
      // No override for this type: always let users "see the data" as pretty-printed JSON.
      const data = (this.ref as { data?: unknown }).data ?? this.ref;
      renderJson(host, data);
      return;
    }
    await renderer.render(this.ref, host);
  }

  render() {
    return html`${this.host}`;
  }
}

customElements.define("mcp-content-router", ContentRouter);
