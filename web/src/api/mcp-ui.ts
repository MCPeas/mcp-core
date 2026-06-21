// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

// The public API a consumer MCP imports as the bare specifier `mcp-ui` (mapped to
// /api/mcp-ui.js by the baked import map). A reusable toolkit: runtime entry points, the
// browser MCP client, the shell's elements (importing this registers their custom-element
// tags, so a consumer can compose `<mcp-catalog-list>`, `<mcp-tool>`, `<mcp-layout-list>`,
// ... in its own views), plus the (erased) authoring types.

import type { ViewDescriptor } from "../shell/types.js";

// Primitives.
export { ListElement } from "../shell/list-element.js";
export { registerRenderer, getRenderer } from "../shell/registry.js";
export { navigate } from "../shell/navigate.js";
export { apiBase, mcpEndpoint, markdownEnabled } from "../shell/endpoints.js";

// Browser MCP client (Streamable HTTP): tools/list + tools/call.
export { McpClient } from "../shell/mcp-client.js";

// Reusable elements (the import side effect defines each custom element).
export { SchemaForm } from "../shell/schema-form.js";
export { ToolElement } from "../shell/tool.js";
export { Operations } from "../shell/operations.js";
export { CatalogList } from "../shell/catalog-list.js";
export { SearchList } from "../shell/search-list.js";
export { LayoutList } from "../shell/layout-list.js";
export { LayoutFull } from "../shell/layout-full.js";
export { ContentRouter } from "../shell/content-router.js";
// Safe sandboxed-iframe primitive + markdown rendering (gated by the `web-markdown` feature).
export { Frame } from "../shell/frame.js";
export { McpMarkdown, markdownToHtml } from "../shell/markdown.js";

// Typed-catalog explorer: a generic, schema-driven entity view + faceted list, plus hooks to
// override rendering per entity type or field kind (a Lit element or a bare render method).
export { Entity, registerEntityRenderer } from "../shell/entity.js";
export type { EntityRenderer } from "../shell/entity.js";
export { EntityList, registerListToolbar } from "../shell/entity-list.js";
export type { ListToolbarRenderer, ListToolbarContext } from "../shell/entity-list.js";
export {
  registerFieldRenderer,
  fieldKind,
  fieldLayout,
  renderField,
} from "../shell/catalog-fields.js";
export type { FieldRenderer, FieldCtx } from "../shell/catalog-fields.js";
export { entitySchema, entityType, loadEntity } from "../shell/catalog-schema.js";

// Authoring + data types (erased at runtime).
export type { Renderer } from "../shell/registry.js";
export type {
  JsonSchema,
  ToolDef,
  ToolResult,
  ToolAnnotations,
  ContentBlock,
} from "../shell/mcp-client.js";
export type { SearchHit } from "../shell/search-list.js";
export type {
  EntityType,
  EntityAction,
  FilterToggle,
  Relationship,
  Cardinality,
  ResourceRef,
  Resource,
  PropSchema,
  EntityRef,
} from "../shell/catalog-types.js";
export type {
  Manifest,
  ViewDescriptor,
  FullView,
  ListView,
  ContentRef,
  EsmRef,
  HtmlRef,
  MdRef,
  JsonRef,
  CatalogItem,
  CatalogAction,
} from "../shell/types.js";

/** Identity helper for typed view authoring in TypeScript consumers. */
export function defineView(view: ViewDescriptor): ViewDescriptor {
  return view;
}
