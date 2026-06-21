// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

import { LitElement, html } from "lit";
import { property } from "lit/decorators.js";
import "./content-router.js";
import type { ContentRef } from "./types.js";

/** Full-page layout: a single content router filling the content area. */
export class LayoutFull extends LitElement {
  @property({ attribute: false }) content: ContentRef | null = null;

  createRenderRoot(): this {
    return this;
  }

  render() {
    return html`
      <div class="mcp-layout-full h-100">
        <mcp-content-router .ref=${this.content}></mcp-content-router>
      </div>
    `;
  }
}

customElements.define("mcp-layout-full", LayoutFull);
