// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

import { LitElement, html } from "lit";
import { property, state } from "lit/decorators.js";
import type { PropertyValues } from "lit";
import "./content-router.js";
import type { ContentRef, ListView } from "./types.js";

/**
 * List/detail layout: a list pane on the left (a {@link ListElement} subclass defined by an
 * imported module) and a content router on the right that renders the
 * {@link ContentRef} of the selected item.
 *
 * The detail pane keeps a navigation history: choosing an item in the list pane starts a
 * fresh history at that item, while drilling in *within* the content (a paragraph link, a
 * cross-reference, prev/next) pushes onto it -- so a Back button can walk back out. Both
 * paths arrive as the same bubbling `mcp-navigate` event; they are told apart by whether the
 * event originated inside the list pane.
 */
export class LayoutList extends LitElement {
  @property({ attribute: false }) view: ListView | null = null;
  /** Detail-pane history; the last entry is shown. Empty until something is selected. */
  @state() stack: ContentRef[] = [];
  @state() listEl: HTMLElement | null = null;

  createRenderRoot(): this {
    return this;
  }

  // The list element's defining module is imported statically (by whoever registers the view),
  // so its tag is already defined -- just instantiate it when the view changes.
  updated(changed: PropertyValues): void {
    if (changed.has("view")) {
      this.stack = [];
      this.listEl = null;
      if (this.view) {
        const el = document.createElement(this.view.element);
        if (this.view.props) {
          Object.assign(el, this.view.props);
        }
        this.listEl = el;
      }
    }
  }

  // Navigation from anywhere in the view. A selection in the list pane starts a fresh history;
  // a drill-in from the content pane (a "next paragraph" link, a cross-reference) pushes onto
  // it. The two are told apart by whether the event came from inside the list pane.
  private onNavigate(e: Event): void {
    const content = (e as CustomEvent<{ content: ContentRef }>).detail.content;
    const fromList = !!this.listEl && this.listEl.contains(e.target as Node);
    this.stack = fromList ? [content] : [...this.stack, content];
  }

  private back(): void {
    if (this.stack.length > 1) {
      this.stack = this.stack.slice(0, -1);
    }
  }

  render() {
    const current = this.stack[this.stack.length - 1] ?? null;
    return html`
      <div class="mcp-layout-list row g-0 h-100" @mcp-navigate=${(e: Event) => this.onNavigate(e)}>
        <div class="mcp-list-pane col-12 col-md-5 col-lg-4 border-end overflow-auto">
          ${this.listEl}
        </div>
        <div class="mcp-detail-pane col overflow-auto">
          ${this.stack.length > 1
            ? html`<div class="sticky-top bg-body border-bottom px-3 py-2">
                <button
                  type="button"
                  class="btn btn-sm btn-link p-0 text-decoration-none"
                  @click=${() => this.back()}
                >
                  &lsaquo; Back
                </button>
              </div>`
            : ""}
          <mcp-content-router .ref=${current}></mcp-content-router>
        </div>
      </div>
    `;
  }
}

customElements.define("mcp-layout-list", LayoutList);
