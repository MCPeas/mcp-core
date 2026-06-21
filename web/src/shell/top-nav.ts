// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

import { LitElement, html } from "lit";
import { property } from "lit/decorators.js";

/** The top navigation bar. Light DOM so Bootstrap's navbar styles apply. */
export class TopNav extends LitElement {
  @property({ type: String }) heading = "MCP";

  createRenderRoot(): this {
    return this;
  }

  render() {
    return html`
      <nav class="navbar bg-dark" data-bs-theme="dark">
        <div class="container-fluid">
          <span class="navbar-brand mb-0 h1">${this.heading}</span>
        </div>
      </nav>
    `;
  }
}

customElements.define("mcp-top-nav", TopNav);
