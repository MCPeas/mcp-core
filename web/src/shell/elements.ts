// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

// The shell's custom elements, typed centrally so tag references are checked.
//
// The `HTMLElementTagNameMap` augmentation makes `document.createElement("mcp-...")` and
// `querySelector("mcp-...")` return the concrete element type (and a typo'd tag a type error).
// The `Known*` unions document the built-in tags / content kinds and type the consumer-facing
// manifest + renderer registry. Importing this file (even type-only) pulls the augmentation in.

import type { AppShell } from "./app-shell.js";
import type { TopNav } from "./top-nav.js";
import type { Sidebar } from "./sidebar.js";
import type { LayoutFull } from "./layout-full.js";
import type { LayoutList } from "./layout-list.js";
import type { ContentRouter } from "./content-router.js";
import type { CatalogList } from "./catalog-list.js";
import type { SearchList } from "./search-list.js";
import type { SchemaForm } from "./schema-form.js";
import type { ToolElement } from "./tool.js";
import type { Operations } from "./operations.js";
import type { Frame } from "./frame.js";
import type { McpMarkdown } from "./markdown.js";
import type { EntityList } from "./entity-list.js";

declare global {
  interface HTMLElementTagNameMap {
    "mcp-app-shell": AppShell;
    "mcp-top-nav": TopNav;
    "mcp-sidebar": Sidebar;
    "mcp-layout-full": LayoutFull;
    "mcp-layout-list": LayoutList;
    "mcp-content-router": ContentRouter;
    "mcp-catalog-list": CatalogList;
    "mcp-search-list": SearchList;
    "mcp-schema-form": SchemaForm;
    "mcp-tool": ToolElement;
    "mcp-operations": Operations;
    "mcp-frame": Frame;
    "mcp-markdown": McpMarkdown;
    "mcp-entity-list": EntityList;
  }
}

/** Every tag the shell registers (a curated union -- `keyof HTMLElementTagNameMap` is every tag). */
export type KnownElementTag =
  | "mcp-app-shell"
  | "mcp-top-nav"
  | "mcp-sidebar"
  | "mcp-layout-full"
  | "mcp-layout-list"
  | "mcp-content-router"
  | "mcp-catalog-list"
  | "mcp-search-list"
  | "mcp-schema-form"
  | "mcp-tool"
  | "mcp-operations"
  | "mcp-frame"
  | "mcp-markdown"
  | "mcp-entity-list";

/** The list elements a {@link ListView} can name (the `ListElement` subclasses). */
export type KnownListTag = "mcp-catalog-list" | "mcp-search-list" | "mcp-entity-list";

/** Content kinds the shell registers a renderer for; consumers add more via `registerRenderer`. */
export type KnownContentKind = "esm" | "html" | "md" | "json" | "catalog-item" | "operations";

/** Accept any string, but keep editor autocomplete for the known literals `T`. */
export type Open<T extends string> = T | (string & {});
