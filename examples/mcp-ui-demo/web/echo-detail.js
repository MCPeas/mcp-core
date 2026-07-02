// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

import { LitElement, html } from "lit";

// A custom override component (registered via an `esm` ContentRef). Receives the selected
// item through the `selection` property set by the shell's esm renderer.
class DemoEchoDetail extends LitElement {
  static properties = {
    selection: { attribute: false },
  };

  selection = null;

  createRenderRoot() {
    return this;
  }

  render() {
    const s = this.selection ?? {};
    return html`
      <div class="p-4">
        <h3>Echo detail</h3>
        <p class="text-muted">A custom override component, not the JSON fallback.</p>
        <p>Message: <strong>${s.message ?? ""}</strong></p>
      </div>
    `;
  }
}

customElements.define("demo-echo-detail", DemoEchoDetail);
