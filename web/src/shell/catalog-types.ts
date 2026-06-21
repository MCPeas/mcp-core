// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

// The catalog model. The wire types are generated from the Rust structs (the single source of
// truth) into ./generated/ by `cargo test --features ts-export`; they are re-exported here so the
// shell's import paths stay stable. PropSchema, Resource and EntityRef are TypeScript-only (no Rust
// struct) and stay hand-written. See docs/design/schema-catalog.md.

export type { Cardinality } from "./generated/Cardinality.js";
export type { Relationship } from "./generated/Relationship.js";
export type { EntityAction } from "./generated/EntityAction.js";
export type { FilterToggle } from "./generated/FilterToggle.js";
export type { EntityType } from "./generated/EntityType.js";
export type { ResourceRef } from "./generated/ResourceRef.js";
// The JSON:API resource object (the `data` of GET {api}/catalog/items/{type}/{id}).
export type { ResourceObject as Resource } from "./generated/ResourceObject.js";
// JSON:API envelopes for the list + single-resource responses.
export type { JsonApiList } from "./generated/JsonApiList.js";
export type { JsonApiResource } from "./generated/JsonApiResource.js";

/** A JSON Schema property (the subset the field renderers consult). */
export interface PropSchema {
  type?: string;
  format?: string;
  title?: string;
  description?: string;
  enum?: unknown[];
  items?: PropSchema;
  properties?: Record<string, PropSchema>;
  /** Render hint that JSON Schema does not carry natively (markdown/code/table/ref/...). */
  "x-mcp-kind"?: string;
  /** Detail-view sort key (lower renders earlier); unset fields keep their layout default. */
  "x-mcp-order"?: number;
  [key: string]: unknown;
}

/** A ContentRef that opens an entity in the explorer (routed to the generic/custom renderer). */
export interface EntityRef {
  type: "entity";
  entityType: string;
  id: string;
}
