// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

import { LitElement, html, type TemplateResult } from "lit";
import { state } from "lit/decorators.js";
import { registerRenderer } from "./registry.js";
import { McpClient, type ToolDef } from "./mcp-client.js";
import { mcpEndpoint } from "./endpoints.js";
import "./tool.js";
import type { ContentRef } from "./types.js";

/**
 * Built-in "Operations" console: lists the server's MCP tools (`tools/list`) and lets the user
 * invoke each one (`tools/call`) through a single shared {@link McpClient} -- the MCP tool
 * schema is the single source of truth, so there is no parallel REST API. Available on every
 * mcp-core web server started with `--mcp`; degrades to a friendly note when the endpoint is
 * unreachable.
 */
export class Operations extends LitElement {
  @state() tools: ToolDef[] | null = null;
  @state() error = "";
  @state() loading = true;
  private readonly client = new McpClient(mcpEndpoint());

  createRenderRoot(): this {
    return this;
  }

  connectedCallback(): void {
    super.connectedCallback();
    void this.load();
  }

  private async load(): Promise<void> {
    try {
      this.tools = await this.client.listTools();
    } catch (e) {
      this.error = String(e);
    } finally {
      this.loading = false;
    }
  }

  render(): TemplateResult {
    if (this.loading) {
      return html`<div class="p-4 text-muted">Loading operations...</div>`;
    }
    if (this.error) {
      return html`<div class="alert alert-warning m-3">
        Could not reach the MCP endpoint <code>${mcpEndpoint()}</code>: ${this.error}
        <div class="small mt-1">Operations need the server running with <code>--mcp</code>.</div>
      </div>`;
    }
    const tools = this.tools ?? [];
    if (tools.length === 0) {
      return html`<div class="p-4 text-muted">This server exposes no operations.</div>`;
    }
    return html`
      <div class="mcp-operations p-3">
        <h2 class="h4 mb-3">Operations</h2>
        ${tools.map((tool) => this.card(tool))}
      </div>
    `;
  }

  private card(tool: ToolDef): TemplateResult {
    return html`
      <div class="card mb-3 shadow-sm">
        <div class="card-body">
          <h3 class="h6 mb-1"><code>${tool.name}</code></h3>
          ${tool.description ? html`<p class="text-muted small mb-3">${tool.description}</p>` : ""}
          <mcp-tool .tool=${tool} .client=${this.client}></mcp-tool>
        </div>
      </div>
    `;
  }
}

customElements.define("mcp-operations", Operations);

// The built-in "operations" ContentRef renders the console element.
registerRenderer("operations", {
  render(_ref: ContentRef, host: HTMLElement): void {
    host.replaceChildren(document.createElement("mcp-operations"));
  },
});
