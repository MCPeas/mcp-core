// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

import { LitElement, html, type TemplateResult } from "lit";
import { property, state } from "lit/decorators.js";
// Side-effect import registers <mcp-schema-form>; the type import is erased at build time.
// (A value import used only in a type position would be tree-shaken, dropping the registration.)
import "./schema-form.js";
import type { SchemaForm } from "./schema-form.js";
import type { ContentBlock, McpClient, ToolDef, ToolResult } from "./mcp-client.js";

/**
 * One invocable MCP tool: a schema-driven input form (`mcp-schema-form`) plus a Run button that
 * fires `tools/call` through the {@link McpClient} and renders the result (text blocks +
 * `structuredContent`, JSON pretty-printed like the shell's `json` renderer). The tool's schema
 * is the single source of truth -- there is no parallel API. Destructive tools (the
 * `destructiveHint` annotation, or a delete/remove/uninstall name) confirm before running.
 */
export class ToolElement extends LitElement {
  @property({ attribute: false }) tool: ToolDef | null = null;
  @property({ attribute: false }) client: McpClient | null = null;
  @state() result: ToolResult | null = null;
  @state() error = "";
  @state() running = false;

  createRenderRoot(): this {
    return this;
  }

  private get destructive(): boolean {
    const tool = this.tool;
    if (!tool) {
      return false;
    }
    return (
      tool.annotations?.destructiveHint === true ||
      /(delete|remove|uninstall|drop)/i.test(tool.name)
    );
  }

  private async run(): Promise<void> {
    const tool = this.tool;
    const client = this.client;
    if (!tool || !client) {
      return;
    }
    if (this.destructive && !window.confirm(`Run "${tool.name}"? This may change server state.`)) {
      return;
    }
    const form = this.querySelector<SchemaForm>("mcp-schema-form");
    const args = form?.values ?? {};
    this.running = true;
    this.error = "";
    this.result = null;
    try {
      this.result = await client.callTool(tool.name, args);
    } catch (e) {
      this.error = String(e);
    } finally {
      this.running = false;
    }
  }

  render(): TemplateResult {
    const tool = this.tool;
    if (!tool) {
      return html``;
    }
    // A real <form> so the browser's native `required` validation blocks empty submits and
    // prompts the user, instead of firing a doomed tools/call that the server rejects with a
    // "missing field" deserialize error. The schema-form's inputs are light-DOM descendants of
    // this form, so they associate with it.
    return html`
      <div class="mcp-tool">
        <form @submit=${(e: Event) => this.onSubmit(e)}>
          <mcp-schema-form .schema=${tool.inputSchema ?? null}></mcp-schema-form>
          <div class="mt-3">
            <button
              type="submit"
              class="btn btn-sm ${this.destructive ? "btn-outline-danger" : "btn-primary"}"
              ?disabled=${this.running || !this.client}
            >
              ${this.running ? "Running..." : "Run"}
            </button>
          </div>
        </form>
        ${this.error
          ? html`<div class="alert alert-warning mt-3 mb-0">${this.error}</div>`
          : this.renderResult()}
      </div>
    `;
  }

  private onSubmit(e: Event): void {
    e.preventDefault();
    void this.run();
  }

  private renderResult(): TemplateResult {
    const result = this.result;
    if (!result) {
      return html``;
    }
    const frame = result.isError ? "alert alert-warning" : "border rounded bg-body-tertiary";
    return html`<div class="mt-3 ${frame} overflow-auto" style="max-height: 24rem">
      ${result.content.map((block) => this.renderBlock(block))}
      ${result.structuredContent !== undefined ? json(result.structuredContent) : ""}
    </div>`;
  }

  private renderBlock(block: ContentBlock): TemplateResult {
    if (block.type === "text" && typeof block.text === "string") {
      const parsed = tryParseJson(block.text);
      return parsed !== undefined
        ? json(parsed)
        : html`<pre class="mcp-json p-3 m-0 small">${block.text}</pre>`;
    }
    return json(block);
  }
}

/** Pretty-print as JSON in the same markup the `json` renderer uses, so output looks consistent. */
function json(data: unknown): TemplateResult {
  return html`<pre class="mcp-json p-3 m-0 small">${JSON.stringify(data, null, 2)}</pre>`;
}

/** Parse `text` only when it looks like a JSON object/array, so prose stays prose. */
function tryParseJson(text: string): unknown {
  const trimmed = text.trim();
  if (!trimmed.startsWith("{") && !trimmed.startsWith("[")) {
    return undefined;
  }
  try {
    return JSON.parse(trimmed);
  } catch {
    return undefined;
  }
}

customElements.define("mcp-tool", ToolElement);
