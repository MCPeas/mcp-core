// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

// The contract a consumer MCP's `/components.js` is written against. These are
// type-only declarations (erased at build time); the runtime entry points live in
// `mcp-ui` (../api/mcp-ui.ts).

import type { KnownListTag, Open } from "./elements.js";

/** A consumer's default-exported registration manifest. */
export interface Manifest {
  /** Optional application title shown in the top navigation. */
  title?: string;
  /** The views shown in the sidebar, in order. */
  views: ViewDescriptor[];
}

/** A single view: either a full-page layout or a list/detail layout. */
export type ViewDescriptor = FullView | ListView;

interface ViewBase {
  /** Stable id, used for routing/selection. */
  id: string;
  /** Label shown in the sidebar. */
  title: string;
  /** Optional sidebar glyph (any short string / emoji for v1). */
  icon?: string;
}

/** Content fills the page (minus the sidebar). */
export interface FullView extends ViewBase {
  layout: "full";
  content: ContentRef;
}

/**
 * A list (left) drives content (right). `element` is the tag name of the list custom element
 * (a {@link ListElement} subclass); import the module that defines it (a side-effect import)
 * so the tag is registered before the view is shown.
 */
export interface ListView extends ViewBase {
  layout: "list";
  element: Open<KnownListTag>;
  /** Properties assigned onto the created list element (e.g. a typed-catalog `entityType`). */
  props?: Record<string, unknown>;
}

/**
 * A pointer to a piece of content the content-router knows how to render. The built-in
 * kinds are below; a consumer may register more via `registerRenderer` and use
 * `{ type: "their-kind", ... }`. Any unknown type falls back to the `json` renderer.
 */
export type ContentRef =
  | EsmRef
  | HtmlRef
  | MdRef
  | JsonRef
  | { type: string; [key: string]: unknown };

/** A dynamically-imported ES module that defines a custom element. */
export interface EsmRef {
  type: "esm";
  /** Module URL (or bare specifier) to import. */
  module: string;
  /** Custom-element tag the module defines. */
  element: string;
  /** Properties assigned onto the created element. */
  props?: Record<string, unknown>;
  /** The current selection (set as the element's `selection` property). */
  selection?: unknown;
}

/** An HTML document shown in a sandboxed iframe. Provide one of `url` / `srcdoc`. */
export interface HtmlRef {
  type: "html";
  url?: string;
  srcdoc?: string;
  /** Allow scripts in the sandbox (off by default). */
  allowScripts?: boolean;
}

/** Markdown converted to HTML and shown in a (script-free) sandbox. One of `url`/`text`. */
export interface MdRef {
  type: "md";
  url?: string;
  text?: string;
}

/**
 * Built-in fallback: pretty-print `data` (or, when omitted, the ref itself) as JSON in
 * a `<pre>`. The content-router uses this for any item whose `type` has no registered
 * renderer, so data is always inspectable.
 */
export interface JsonRef {
  type: "json";
  data?: unknown;
}

// CatalogItem + CatalogAction are generated from the Rust types -- see web/src/shell/generated/.
export type { CatalogItem } from "./generated/CatalogItem.js";
export type { CatalogAction } from "./generated/CatalogAction.js";
