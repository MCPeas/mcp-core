// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

import { LitElement, html } from "lit";
import { property, state } from "lit/decorators.js";
import "./top-nav.js";
import "./sidebar.js";
import "./layout-full.js";
import "./layout-list.js";
import "./operations.js";
// The built-in list views' elements are imported here so their tags are defined up front; the
// shell is bundled and minified for production, so there is nothing to gain from lazy-loading.
import "./catalog-list.js";
import "./search-list.js";
import type { Manifest, ViewDescriptor } from "./types.js";

/**
 * Built-in "Operations" view, always appended to the sidebar so every mcp-core web
 * server exposes its MCP operations explorer with no per-server work.
 */
const OPERATIONS_VIEW: ViewDescriptor = {
  id: "operations",
  title: "Operations",
  layout: "full",
  content: { type: "operations" },
};

/**
 * Built-in "Catalog" view: a list/detail browser over the server's information catalog
 * (a `CatalogProvider` on the Rust side). Added only when `{api-base}/catalog/items` is
 * reachable, so servers without a catalog show no empty nav entry.
 */
const CATALOG_VIEW: ViewDescriptor = {
  id: "catalog",
  title: "Catalog",
  layout: "list",
  element: "mcp-catalog-list",
};

/**
 * Built-in "Search" view: a query box + results (the list pane) drive the content pane (a
 * `SearchProvider` on the Rust side). Added only when `{api-base}/search` is reachable.
 */
const SEARCH_VIEW: ViewDescriptor = {
  id: "search",
  title: "Search",
  layout: "list",
  element: "mcp-search-list",
};

/**
 * The application shell: top navigation, sidebar, and the active view's layout. Fed a
 * {@link Manifest} via `setManifest` (the entry point loads the consumer's manifest).
 */
export class AppShell extends LitElement {
  @property({ attribute: false }) manifest: Manifest | null = null;
  @state() activeId = "";
  /** Effective sidebar views: the consumer's views plus the built-in Operations view. */
  views: ViewDescriptor[] = [];

  createRenderRoot(): this {
    return this;
  }

  /** Install the manifest and select the first view. The built-in Operations view is always
   * appended (and Search / Catalog views when the server has them), so even a server with no
   * consumer frontend is browsable. */
  setManifest(
    manifest: Manifest,
    opts?: { catalog?: boolean; search?: boolean; typedCatalog?: ViewDescriptor[] },
  ): void {
    this.manifest = manifest;
    const builtins = [
      ...(opts?.search ? [SEARCH_VIEW] : []),
      ...(opts?.typedCatalog ?? []),
      ...(opts?.catalog ? [CATALOG_VIEW] : []),
      OPERATIONS_VIEW,
    ];
    this.views = [...manifest.views, ...builtins];
    this.activeId = manifest.views[0]?.id ?? builtins[0].id;
  }

  private get active(): ViewDescriptor | null {
    return this.views.find((v) => v.id === this.activeId) ?? null;
  }

  private onNav(e: Event): void {
    this.activeId = (e as CustomEvent<{ id: string }>).detail.id;
  }

  render() {
    const manifest = this.manifest;
    if (!manifest) {
      return html`<div class="p-5 text-center text-muted">Loading...</div>`;
    }
    const view = this.active;
    return html`
      <div class="mcp-shell d-flex flex-column vh-100">
        <mcp-top-nav .heading=${manifest.title ?? "MCP"}></mcp-top-nav>
        <div class="d-flex flex-grow-1 overflow-hidden">
          <mcp-sidebar
            .views=${this.views}
            .activeId=${this.activeId}
            @mcp-nav-select=${(e: Event) => this.onNav(e)}
          ></mcp-sidebar>
          <main class="mcp-content flex-grow-1 overflow-hidden d-flex flex-column">
            ${this.renderView(view)}
          </main>
        </div>
      </div>
    `;
  }

  private renderView(view: ViewDescriptor | null) {
    if (!view) {
      return html`<div class="p-5 text-center text-muted">No views registered.</div>`;
    }
    if (view.layout === "list") {
      return html`<mcp-layout-list .view=${view}></mcp-layout-list>`;
    }
    return html`<mcp-layout-full .content=${view.content}></mcp-layout-full>`;
  }
}

customElements.define("mcp-app-shell", AppShell);
