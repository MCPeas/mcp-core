// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

import { LitElement, html } from "lit";
import { property } from "lit/decorators.js";
import type { ViewDescriptor } from "./types.js";

/**
 * The sidebar: one nav entry per view, permanently visible on large screens
 * (`d-lg-block`). Emits `mcp-nav-select` with the chosen view id.
 */
export class Sidebar extends LitElement {
  @property({ attribute: false }) views: ViewDescriptor[] = [];
  @property({ type: String }) activeId = "";

  createRenderRoot(): this {
    return this;
  }

  private select(id: string): void {
    this.dispatchEvent(
      new CustomEvent("mcp-nav-select", {
        detail: { id },
        bubbles: true,
        composed: true,
      }),
    );
  }

  render() {
    return html`
      <nav
        class="mcp-sidebar bg-body-tertiary border-end p-2 d-none d-lg-block"
        style="width: 14rem; flex: 0 0 auto"
      >
        <ul class="nav nav-pills flex-column gap-1">
          ${this.views.map(
            (v) => html`
              <li class="nav-item">
                <a
                  class="nav-link ${v.id === this.activeId ? "active" : ""}"
                  href="#"
                  @click=${(e: Event) => {
                    e.preventDefault();
                    this.select(v.id);
                  }}
                >
                  ${v.icon ? html`<span class="me-2">${v.icon}</span>` : ""}${v.title}
                </a>
              </li>
            `,
          )}
        </ul>
      </nav>
    `;
  }
}

customElements.define("mcp-sidebar", Sidebar);
