// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

import { html, type TemplateResult } from "lit";
import { property, state } from "lit/decorators.js";
import { ListElement } from "./list-element.js";
import { McpClient } from "./mcp-client.js";
import { apiBase, mcpEndpoint } from "./endpoints.js";
import { emptyNote, errorAlert, loadingNote, toolResultText, workingNote } from "./ui.js";
import type { CatalogAction, CatalogItem, ContentRef } from "./types.js";

/**
 * Built-in catalog list: the sidebar pane of the shell's Catalog view, and a reusable building
 * block. Fetches items from `{api-base}{endpoint}` (default `/catalog/items`); selecting one
 * points the content pane at `toContent(item)`. Items may carry `actions`, each invoking an MCP
 * tool over `/mcp` (`tools/call`) -- e.g. install / uninstall / update -- after which the list
 * refreshes. `endpoint`/`toContent` are configurable so a consumer can reuse it for a second
 * collection (e.g. a browse-and-install view over available items).
 */
export class CatalogList extends ListElement {
  /** Catalog endpoint, relative to the API base. */
  @property({ type: String }) endpoint = "/catalog/items";
  /** Maps a selected item to the ContentRef shown in the detail pane. */
  toContent: (item: CatalogItem) => ContentRef = (item) => ({
    type: "catalog-item",
    id: item.id,
  });

  @state() items: CatalogItem[] | null = null;
  @state() error = "";
  @state() filter = "";
  /** Id of the item whose action is currently running (disables buttons), else "". */
  @state() acting = "";
  /** Outcome of the last action, shown inline on its item. */
  @state() feedback: { id: string; message: string; isError: boolean } | null = null;
  private readonly client = new McpClient(mcpEndpoint());

  /** Items matching the current filter (case-insensitive, over title + subtitle). */
  private get filtered(): CatalogItem[] {
    const q = this.filter.trim().toLowerCase();
    if (!q || !this.items) {
      return this.items ?? [];
    }
    return this.items.filter(
      (i) => i.title.toLowerCase().includes(q) || (i.subtitle?.toLowerCase().includes(q) ?? false),
    );
  }

  connectedCallback(): void {
    super.connectedCallback();
    void this.load();
  }

  private async load(): Promise<void> {
    try {
      const res = await fetch(`${apiBase()}${this.endpoint}`);
      if (!res.ok) {
        throw new Error(`HTTP ${res.status}`);
      }
      this.items = (await res.json()) as CatalogItem[];
    } catch (e) {
      this.error = String(e);
    }
  }

  /** Run an item's action: call its MCP tool (with the item id merged in), then refresh. */
  private async runAction(action: CatalogAction, item: CatalogItem): Promise<void> {
    if (action.danger && !window.confirm(`${action.label}: ${item.title}?`)) {
      return;
    }
    this.acting = item.id;
    this.feedback = null;
    try {
      const result = await this.client.callTool(action.tool, { ...action.args, id: item.id });
      this.feedback = {
        id: item.id,
        message: toolResultText(result) ?? "Done.",
        isError: result.isError,
      };
      await this.load();
    } catch (e) {
      this.feedback = { id: item.id, message: String(e), isError: true };
    } finally {
      this.acting = "";
    }
  }

  render(): TemplateResult {
    if (this.error) {
      return errorAlert(`Could not load the catalog: ${this.error}`);
    }
    if (!this.items) {
      return loadingNote();
    }
    if (this.items.length === 0) {
      return emptyNote("The catalog is empty.");
    }
    const items = this.filtered;
    return html`
      <div class="p-2 border-bottom bg-body">
        <input
          type="search"
          class="form-control form-control-sm"
          placeholder="Filter..."
          aria-label="Filter catalog"
          .value=${this.filter}
          @input=${(e: Event) => {
            this.filter = (e.target as HTMLInputElement).value;
          }}
        />
      </div>
      <div class="list-group list-group-flush">
        ${items.length === 0
          ? html`<div class="p-3 small text-muted">No matches for "${this.filter}".</div>`
          : items.map((item) => this.row(item))}
      </div>
    `;
  }

  private row(item: CatalogItem): TemplateResult {
    const active = item.id === this.selectedId;
    return html`
      <div
        class="list-group-item list-group-item-action d-flex flex-column gap-1 ${active
          ? "active"
          : ""}"
        role="button"
        style="cursor: pointer"
        @click=${() => this.select(item.id, this.toContent(item))}
      >
        <div class="fw-medium">${item.title}</div>
        ${item.subtitle ? html`<div class="small text-muted">${item.subtitle}</div>` : ""}
        ${item.actions?.length ? this.actions(item) : ""}
        ${this.acting === item.id ? workingNote() : ""}
        ${this.feedback?.id === item.id
          ? html`<div class="small ${this.feedback.isError ? "text-danger" : "text-success"}">
              ${this.feedback.message}
            </div>`
          : ""}
      </div>
    `;
  }

  private actions(item: CatalogItem): TemplateResult {
    return html`<span
      class="btn-group btn-group-sm align-self-start mt-1"
      @click=${(e: Event) => e.stopPropagation()}
    >
      ${(item.actions ?? []).map(
        (action) =>
          html`<button
            type="button"
            class="btn btn-outline-${action.danger ? "danger" : "secondary"}"
            ?disabled=${this.acting !== ""}
            @click=${() => void this.runAction(action, item)}
          >
            ${action.label}
          </button>`,
      )}
    </span>`;
  }
}

customElements.define("mcp-catalog-list", CatalogList);
