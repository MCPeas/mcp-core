// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

import { html, type TemplateResult } from "lit";
import { property, state } from "lit/decorators.js";
import { ListElement } from "./list-element.js";
import { apiBase } from "./endpoints.js";
import { emptyNote, errorAlert, loadingNote } from "./ui.js";
import type { ContentRef } from "./types.js";
import type { SearchHit } from "./generated/SearchHit.js";

// SearchHit + SearchResults are generated from the Rust types -- see web/src/shell/generated/.
export type { SearchHit };
export type { SearchResults } from "./generated/SearchResults.js";

/**
 * Built-in Search view (the list pane of a list/detail layout): a debounced query box whose
 * results drive the content pane. Posts `{ q }` to `{api-base}/search`; selecting a hit opens
 * its `content` ContentRef (or, by default, the hit's catalog item) via the navigation event.
 */
export class SearchList extends ListElement {
  /** Search endpoint, relative to the API base. */
  @property({ type: String }) endpoint = "/search";
  /** Maps a hit to the ContentRef opened on selection. */
  toContent: (hit: SearchHit) => ContentRef = (hit) =>
    // content is an opaque JSON value on the wire (Rust `Value`); the shell treats it as a ContentRef.
    (hit.content as ContentRef | undefined) ?? { type: "catalog-item", id: hit.id };
  @state() query = "";
  @state() hits: SearchHit[] = [];
  @state() loading = false;
  @state() error = "";
  @state() searched = false;
  private timer = 0;

  private onInput(value: string): void {
    this.query = value;
    clearTimeout(this.timer);
    this.timer = window.setTimeout(() => void this.run(), 250);
  }

  private async run(): Promise<void> {
    const q = this.query.trim();
    if (!q) {
      this.hits = [];
      this.searched = false;
      return;
    }
    this.loading = true;
    this.error = "";
    try {
      const res = await fetch(`${apiBase()}${this.endpoint}`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ q }),
      });
      if (!res.ok) {
        throw new Error(`HTTP ${res.status}`);
      }
      const data = (await res.json()) as { hits?: SearchHit[] };
      this.hits = data.hits ?? [];
      this.searched = true;
    } catch (e) {
      this.error = String(e);
    } finally {
      this.loading = false;
    }
  }

  private open(hit: SearchHit): void {
    this.select(hit.id, this.toContent(hit));
  }

  private results(): TemplateResult {
    if (this.error) {
      return errorAlert(`Search failed: ${this.error}`);
    }
    if (this.loading) {
      return loadingNote("Searching...");
    }
    if (this.searched && this.hits.length === 0) {
      return emptyNote(`No results for "${this.query}".`);
    }
    return html`
      <div class="list-group list-group-flush">
        ${this.hits.map(
          (hit) => html`
            <button
              type="button"
              class="list-group-item list-group-item-action ${hit.id === this.selectedId
                ? "active"
                : ""}"
              @click=${() => this.open(hit)}
            >
              <div class="fw-medium">${hit.title}</div>
              ${hit.subtitle ? html`<div class="small text-muted">${hit.subtitle}</div>` : ""}
              ${hit.snippet
                ? html`<div class="small text-body-secondary">${hit.snippet}</div>`
                : ""}
            </button>
          `,
        )}
      </div>
    `;
  }

  render() {
    return html`
      <div class="p-2 border-bottom bg-body">
        <input
          type="search"
          class="form-control form-control-sm"
          placeholder="Search..."
          aria-label="Search"
          .value=${this.query}
          @input=${(e: Event) => this.onInput((e.target as HTMLInputElement).value)}
        />
      </div>
      ${this.results()}
    `;
  }
}

customElements.define("mcp-search-list", SearchList);
