# Design: the schema + relations catalog (typed data-explorer layer)

Status: accepted; part of the single web-UI shell PR (#1) -- one consolidated feature. This
layer builds on the same PR's browser-side invocation + reusable element toolkit.

## 1. Problem and goal

Today the catalog is a flat list of `{ id, title, subtitle }` whose per-item content is an
opaque JSON blob, rendered either by the generic JSON fallback or by a renderer the consumer
hand-writes in `components.js`. Every data-oriented MCP server therefore re-implements a
bespoke renderer -- a modest one for a simple server, a large one for a rich document model.
It duplicates work and does not scale, and "references between items" are not modelled at all.

Goal: make the catalog a **typed, self-describing, related data model**. An MCP server
(built on mcp-core) declares its entity types, their fields, and the relations between them
**once, as Rust types**. The app-shell then *explores* that model generically -- list/detail,
faceting, and clickable relations -- with the server (or mcp-core's own component library)
supplying a Lit component or a bare render method only where the generic rendering is not
enough. The catalog becomes the data-explorer foundation data-oriented servers sit on.

Design tenets:
- Declared in Rust, in the server, via mcp-core types (single source of truth, compile-checked).
- Standards-based wire format (below), so the data is legible to other clients/tools too.
- Generic by default; custom only for the genuinely unusual. Customization should shrink to
  near zero for a simple server and to a handful of field renderers for a rich one.
- References are first-class and explorable (navigate the graph).

## 2. Standards

- **Field/attribute shapes: JSON Schema (Draft 2020-12).** Same vocabulary MCP tool inputs
  already use; generated from the server's Rust attribute structs via `schemars`. One schema
  vocabulary spans tool I/O and catalog data.
- **Identity, relationships, links: JSON:API resource conventions.** A resource is
  `{ type, id, attributes, relationships, links }`; a relationship carries typed refs
  (`data: [{ type, id }]`) plus a `related` link. This is the off-the-shelf standard for
  "typed resources with navigable relationships," and maps 1:1 onto what we need.
- **OpenAPI: optional.** A generated description of the catalog HTTP endpoints (Swagger UI,
  client codegen), reusing the JSON Schema models. Not the source of the entity-relation model.

Render hints that JSON Schema does not natively carry (markdown vs plain text, code, table,
"render as a reference") are expressed with a custom keyword `x-mcp-kind` on a property schema
(JSON Schema permits custom keywords); absent it, the renderer falls back to `type`/`format`.

## 3. Rust model (mcp-core)

A new `catalog` module (the existing flat `CatalogProvider` stays; see 7). Sketch:

```rust
/// One entity type in a server's catalog schema.
pub struct EntityType {
    pub name: String,            // machine name, e.g. "document"
    pub title: String,           // human label, e.g. "Dokument"
    /// JSON Schema (Draft 2020-12) for this type's attributes -- typically
    /// `schemars::schema_for!(DocumentAttributes)`. Properties may carry `x-mcp-kind`.
    pub attributes: serde_json::Value,
    pub relationships: Vec<Relationship>,
    /// Attribute names usable as facets/filters in the list view.
    pub facets: Vec<String>,
}

pub enum Cardinality { ToOne, ToMany }

pub struct Relationship {
    pub name: String,            // "tables"
    pub label: String,           // "Tabellen"
    pub target: String,          // an EntityType name
    pub cardinality: Cardinality,
}

/// A typed reference to another resource (the `data` of a JSON:API relationship).
pub struct ResourceRef { pub r#type: String, pub id: String, pub title: Option<String> }

/// A full resource: attributes (matching the type's JSON Schema) + resolved relationships.
pub struct Resource {
    pub r#type: String,
    pub id: String,
    pub attributes: serde_json::Value,
    pub relationships: BTreeMap<String, Vec<ResourceRef>>,
}

pub struct CatalogQuery {
    pub filter: BTreeMap<String, String>,   // facet field -> value
    pub q: Option<String>,                  // free text (optional)
    pub limit: usize,
    pub offset: usize,
}
pub struct CatalogPage { pub items: Vec<ResourceRef>, pub total: Option<usize> }

/// Implemented by a data-oriented MCP server. Async so it can hit a Tantivy index / store.
pub trait DataCatalog: Send + Sync + 'static {
    fn schema(&self) -> Vec<EntityType>;
    fn list(&self, ty: &str, query: CatalogQuery)
        -> impl Future<Output = CatalogPage> + Send;
    fn get(&self, ty: &str, id: &str)
        -> impl Future<Output = Option<Resource>> + Send;
    /// Optional: lazily resolve a to-many relationship for large sets (see 8).
    fn related(&self, ty: &str, id: &str, rel: &str, query: CatalogQuery)
        -> impl Future<Output = CatalogPage> + Send { /* default: read from get().relationships */ }
}
```

The server writes a plain `#[derive(Serialize, JsonSchema)]` struct per entity's attributes;
mcp-core derives the JSON Schema and assembles the JSON:API wire form. Relationships are
declared explicitly (they are the part JSON Schema cannot express).

## 4. HTTP API (JSON:API-shaped)

- `GET {api}/catalog/schema` -> `[EntityType]` (the meta-description: per type, its JSON
  Schema attributes + relationships + facets). Drives the entire generic UI.
- `GET {api}/catalog/items/{type}?filter[f]=v&q=&page[limit]=&page[offset]=`
  -> `{ data: [ResourceRef], meta: { total } }`.
- `GET {api}/catalog/items/{type}/{id}`
  -> `{ data: { type, id, attributes, relationships: { name: { data: [ref], links:{related} } }, links:{self} } }`.
- `GET {api}/catalog/items/{type}/{id}/{rel}` -> lazy to-many (optional; large sets).

Note: hierarchical/slash ids (e.g. document paths) are carried as the trailing `{id}` segment
(wildcard route), which the current single-segment `/catalog/items/{id}` route cannot do --
that limitation is one of the concrete drivers for this layer.

## 5. Frontend (generic, schema-driven) -- in the shell

New content kind `{ type: "entity", entityType, id }`, routed by the content-router.

- **`mcp-entity`**: fetches the resource + its `EntityType` (from `/catalog/schema`, cached),
  renders attributes by JSON Schema (`x-mcp-kind` / `type` / `format`) and a "Relations"
  section where each relationship's refs are buttons -> `navigate({type:"entity",...})`. This
  is the whole explorer: click a reference, land on that entity, repeat.
- **`mcp-entity-list`**: a typed, faceted list for one entity type (facet controls built from
  `EntityType.facets`; pagination from `meta.total`). Selecting an item navigates to it. A
  hierarchy relationship renders as a collapsible tree.
- **Field renderers by kind** (mcp-core ships a reusable set): `text`, `markdown`, `date`,
  `number`, `bool`, `enum`, `code`, `table`, `ref`. This library is the main lever that
  "reduces the customization needed for every server."
- **Custom hooks** ("Lit WebComponents or bare render methods"):
  - `registerEntityRenderer(entityType, ElementTagOrRenderFn)` -- override a whole type.
  - `registerFieldRenderer(kind, ElementTagOrRenderFn)` -- override/add a field kind.
  A `Renderer` may be a custom-element tag or a bare `(data, host) => void` function.
  Generic is the default; custom overrides apply only where registered.

Built-in Catalog/Search views become typed: the Catalog lists entity types (or a default
type); Search hits are `ResourceRef`s that open the entity; facets come from the schema.

## 6. How this reduces customization

- A server that declares schema in Rust gets a complete explorable UI with **zero** frontend
  code (generic entity/list/field/relation rendering + faceting + graph navigation).
- Custom code is needed only to register a renderer for a field kind or entity type the generic
  set does not handle well -- and mcp-core's field-renderer library already covers the common
  kinds, so this is the exception, not the rule.

## 7. Backward compatibility

The existing flat `CatalogProvider` (`{id,title,subtitle}` + opaque `item(id)` JSON) stays and
keeps working (the demo relies on it). `DataCatalog` is additive: a flat catalog is just a
single entity type with no relationships and a JSON-fallback attribute. Servers migrate to
`DataCatalog` when they want typing/relations; nothing breaks meanwhile.

## 8. Open decisions for the build

- `x-mcp-kind` keyword set and how a server annotates it via `schemars` (attribute macro vs a
  post-process step).
- To-many delivery: inline `ResourceRef`s (with title) for small sets vs the lazy
  `/{id}/{rel}` endpoint for large ones (a document's examples / a year's documents can be
  large). Proposal: inline when small; the schema marks a relationship `lazy` otherwise.
- Pagination + facet param shape (lean on JSON:API `page[...]` / `filter[...]`).
- Typed search: fold `SearchProvider` into this (a hit is a `ResourceRef`; facets from schema)
  vs keep search separate for now.
- Module/feature placement in mcp-core (`catalog` under `web-ui`, or its own feature that
  `web-ui` depends on).
- Relationship to MCP tools: the catalog data often mirrors tool outputs. Kept separate now
  (schema is server Rust types, per the decision); deriving one from the other is a later idea.

## 9. Sequencing

Everything lands in the single web-UI shell PR (#1) -- one big feature. Build order within the
PR:
   a. Rust `catalog` model + `DataCatalog` trait + JSON:API endpoints + schema endpoint.
   b. Frontend: `mcp-entity` / `mcp-entity-list`, the field-renderer library, the custom-hook
      registry, `entity` content kind + relation navigation.
   c. Migrate a simple data server onto `DataCatalog`; confirm near-zero custom code.
   d. Migrate a rich document server; confirm custom code reduces to ~one field renderer.
   e. Iterate the field-renderer library + open decisions from what (c)/(d) surface.
