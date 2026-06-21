// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Typed, related catalog -- the data-explorer layer. A server declares its entity types
//! (attributes as JSON Schema, plus relations to other types) and serves typed resources
//! JSON:API-style; the shell's generic entity/list/relation rendering explores them. This is
//! additive to the flat [`CatalogProvider`](super::CatalogProvider). See
//! `docs/design/schema-catalog.md`.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::Value;

/// Cardinality of a [`Relationship`].
#[derive(Clone, Copy, Debug, serde::Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "snake_case")]
pub enum Cardinality {
    ToOne,
    ToMany,
}

/// A relation from one entity type to another (rendered by the shell as navigable links).
#[derive(Clone, Debug, serde::Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS), ts(export))]
pub struct Relationship {
    pub name: String,
    pub label: String,
    /// The target [`EntityType`] name.
    pub target: String,
    pub cardinality: Cardinality,
}

impl Relationship {
    /// A relation named `name` (labelled `label`) to the `target` entity type. Saves consumers
    /// the struct-literal boilerplate when declaring a type's relationships.
    pub fn new(
        name: impl Into<String>,
        label: impl Into<String>,
        target: impl Into<String>,
        cardinality: Cardinality,
    ) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            target: target.into(),
            cardinality,
        }
    }
}

/// One entity type in a server's catalog schema. `attributes` is the JSON Schema for the
/// type's fields (typically `schemars::schema_for!(...)`); properties may carry an
/// `x-mcp-kind` hint (markdown/code/table/...) the shell's field renderers read.
#[derive(Clone, Debug, Default, serde::Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS), ts(export))]
pub struct EntityType {
    pub name: String,
    pub title: String,
    /// Which attribute holds an instance's display title (the shell uses it for headers and
    /// list rows). Falls back to the id when unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub title_field: Option<String>,
    /// Whether this type is a top-level browsable list (the shell adds a sidebar view for it);
    /// non-list types are reached only through relations.
    pub list: bool,
    #[cfg_attr(
        feature = "ts-export",
        ts(type = "import('../catalog-types').PropSchema")
    )]
    pub attributes: Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relationships: Vec<Relationship>,
    /// Attribute names usable as facets/filters in the list view.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub facets: Vec<String>,
    /// Actions offered per item (rendered as per-row buttons in the list's edit mode, and on the
    /// entity detail). A `ResourceRef` may narrow which apply via its `actions` field.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub item_actions: Vec<EntityAction>,
    /// Actions offered for the list as a whole (rendered in the list's edit toolbar).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub list_actions: Vec<EntityAction>,
    /// Filter toggles offered in the list's edit toolbar (e.g. "show available laws").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub list_toggles: Vec<FilterToggle>,
}

/// A typed reference to a resource: a JSON:API resource identifier plus a display title.
/// `group` lets a list/relation be rendered under collapsible section headers; `actions`
/// names which of the type's [`EntityType::item_actions`] apply to this particular row
/// (e.g. an installed law offers update/uninstall, an available one offers install) -- `None`
/// means all of them apply.
#[derive(Clone, Debug, Default, serde::Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS), ts(export))]
pub struct ResourceRef {
    #[serde(rename = "type")]
    pub ty: String,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub title: Option<String>,
    /// Optional grouping key -- the shell renders refs sharing a group under one collapsible
    /// header (a law's table of contents grouped by section, say).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub group: Option<String>,
    /// The subset of the type's `item_actions` applicable to this row (`None` = all).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub actions: Option<Vec<String>>,
}

/// A declarative action on an entity (a row or a whole list): the shell renders a button and,
/// when clicked, calls the named MCP `tool` (over the same `/mcp` transport the Operations view
/// uses) with the row's `id`. mcp-core stays domain-agnostic -- it only knows "call this tool";
/// the consumer says which tool a label maps to (e.g. an "Uninstall" button -> `delete_law`).
#[derive(Clone, Debug, Default, serde::Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS), ts(export))]
pub struct EntityAction {
    pub id: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub icon: Option<String>,
    /// The MCP tool to call.
    pub tool: String,
    /// The argument name the row id is passed as (default `id`, e.g. `law_id`).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub arg: Option<String>,
    /// Render as a destructive action (and confirm before firing).
    #[serde(default, skip_serializing_if = "super::is_false")]
    pub danger: bool,
    /// Optional confirmation prompt shown before the call.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub confirm: Option<String>,
}

/// A declarative filter toggle for a list view's edit toolbar: clicking it flips
/// `filter[key] = value` on the list query (which the consumer's [`DataCatalog::list`]
/// interprets) -- e.g. "show available (uninstalled) laws" -> `filter[installed] = "false"`.
#[derive(Clone, Debug, Default, serde::Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS), ts(export))]
pub struct FilterToggle {
    pub id: String,
    pub label: String,
    pub key: String,
    pub value: String,
}

/// A full resource: attributes (matching its type's schema) + resolved relationships.
#[derive(Clone, Debug, Default)]
pub struct Resource {
    pub ty: String,
    pub id: String,
    pub attributes: Value,
    /// Relationship name -> the referenced resources.
    pub relationships: BTreeMap<String, Vec<ResourceRef>>,
}

/// A page of resource refs for the list view.
#[derive(Clone, Debug, Default)]
pub struct CatalogPage {
    pub items: Vec<ResourceRef>,
    pub total: Option<usize>,
}

/// JSON:API list envelope: `{ "data": [...], "meta": { "total": <n|null> } }`.
#[derive(Clone, Debug, serde::Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS), ts(export))]
pub struct JsonApiList<T> {
    pub data: Vec<T>,
    pub meta: ListMeta,
}

/// Pagination metadata for a [`JsonApiList`].
#[derive(Clone, Debug, Default, serde::Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS), ts(export))]
pub struct ListMeta {
    /// Total matches, or `null` when the provider does not count.
    pub total: Option<usize>,
}

/// JSON:API single-resource envelope: `{ "data": { ... } }`.
#[derive(Clone, Debug, serde::Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS), ts(export))]
pub struct JsonApiResource {
    pub data: ResourceObject,
}

/// A JSON:API resource object: attributes (its type's schema) + to-many relationships by name.
#[derive(Clone, Debug, serde::Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS), ts(export))]
pub struct ResourceObject {
    #[serde(rename = "type")]
    pub ty: String,
    pub id: String,
    #[cfg_attr(feature = "ts-export", ts(type = "Record<string, unknown>"))]
    pub attributes: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub relationships: Option<BTreeMap<String, RelData>>,
}

/// A JSON:API to-many relationship value: `{ "data": [refs] }`.
#[derive(Clone, Debug, serde::Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS), ts(export))]
pub struct RelData {
    pub data: Vec<ResourceRef>,
}

/// A list query: facet filters + free text + pagination.
#[derive(Clone, Debug, Default)]
pub struct CatalogQuery {
    pub filter: BTreeMap<String, String>,
    pub q: Option<String>,
    pub limit: usize,
    pub offset: usize,
}

/// A server's typed, related catalog. Implement it to expose data the shell explores
/// generically. Async so implementations can query a store / Tantivy index.
pub trait DataCatalog: Send + Sync + 'static {
    /// The entity types and their relations.
    fn schema(&self) -> Vec<EntityType>;

    /// A page of items for `ty`, honoring the query's facet filters / text / pagination.
    fn list(
        &self,
        ty: &str,
        query: CatalogQuery,
    ) -> impl std::future::Future<Output = CatalogPage> + Send;

    /// One resource (attributes + relationships), or `None` -> 404.
    fn get(&self, ty: &str, id: &str)
        -> impl std::future::Future<Output = Option<Resource>> + Send;
}

/// A router exposing the typed catalog, JSON:API-shaped:
/// `GET /catalog/schema`, `GET /catalog/items/{type}`, `GET /catalog/items/{type}/{id}`
/// (the `{id}` is a wildcard, so hierarchical/slash ids work). Mount inside `api_router`.
pub fn data_catalog_router<P: DataCatalog>(provider: Arc<P>) -> Router {
    Router::new()
        .route("/catalog/schema", get(schema::<P>))
        .route("/catalog/items/{type}", get(list::<P>))
        .route("/catalog/items/{type}/{*id}", get(get_one::<P>))
        .with_state(provider)
}

async fn schema<P: DataCatalog>(State(provider): State<Arc<P>>) -> Json<Vec<EntityType>> {
    Json(provider.schema())
}

async fn list<P: DataCatalog>(
    State(provider): State<Arc<P>>,
    Path(ty): Path<String>,
    Query(mut params): Query<BTreeMap<String, String>>,
) -> Json<JsonApiList<ResourceRef>> {
    // Reserved params steer the query; everything else is a facet filter.
    let q = params.remove("q").filter(|s| !s.is_empty());
    let limit = params
        .remove("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(50);
    let offset = params
        .remove("offset")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let page = provider
        .list(
            &ty,
            CatalogQuery {
                filter: params,
                q,
                limit,
                offset,
            },
        )
        .await;
    Json(JsonApiList {
        data: page.items,
        meta: ListMeta { total: page.total },
    })
}

async fn get_one<P: DataCatalog>(
    State(provider): State<Arc<P>>,
    Path((ty, id)): Path<(String, String)>,
) -> Result<Json<JsonApiResource>, StatusCode> {
    let resource = provider.get(&ty, &id).await.ok_or(StatusCode::NOT_FOUND)?;
    // Each relationship is wrapped JSON:API-style as `{ "data": [refs] }`; omitted when empty.
    let relationships = if resource.relationships.is_empty() {
        None
    } else {
        Some(
            resource
                .relationships
                .into_iter()
                .map(|(name, refs)| (name, RelData { data: refs }))
                .collect(),
        )
    };
    Ok(Json(JsonApiResource {
        data: ResourceObject {
            ty: resource.ty,
            id: resource.id,
            attributes: resource.attributes,
            relationships,
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use serde_json::json;
    use tower::util::ServiceExt;

    struct TestCatalog;

    impl DataCatalog for TestCatalog {
        fn schema(&self) -> Vec<EntityType> {
            vec![EntityType {
                name: "doc".to_string(),
                title: "Doc".to_string(),
                title_field: Some("title".to_string()),
                list: true,
                attributes: json!({ "type": "object", "properties": { "title": { "type": "string" } } }),
                relationships: vec![Relationship {
                    name: "refs".to_string(),
                    label: "References".to_string(),
                    target: "doc".to_string(),
                    cardinality: Cardinality::ToMany,
                }],
                facets: vec!["year".to_string()],
                ..Default::default()
            }]
        }

        async fn list(&self, ty: &str, _query: CatalogQuery) -> CatalogPage {
            assert_eq!(ty, "doc");
            CatalogPage {
                items: vec![ResourceRef {
                    ty: "doc".to_string(),
                    id: "a/b".to_string(),
                    title: Some("A/B".to_string()),
                    ..Default::default()
                }],
                total: Some(1),
            }
        }

        async fn get(&self, ty: &str, id: &str) -> Option<Resource> {
            (ty == "doc" && id == "a/b").then(|| Resource {
                ty: "doc".to_string(),
                id: "a/b".to_string(),
                attributes: json!({ "title": "A/B" }),
                relationships: BTreeMap::from([(
                    "refs".to_string(),
                    vec![ResourceRef {
                        ty: "doc".to_string(),
                        id: "c/d".to_string(),
                        title: Some("C/D".to_string()),
                        ..Default::default()
                    }],
                )]),
            })
        }
    }

    async fn get(uri: &str) -> (StatusCode, String) {
        let res = data_catalog_router(Arc::new(TestCatalog))
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(bytes.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn serves_schema() {
        let (status, body) = get("/catalog/schema").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains(r#""name":"doc""#), "body: {body}");
        assert!(body.contains(r#""cardinality":"to_many""#), "body: {body}");
    }

    #[tokio::test]
    async fn lists_items() {
        let (status, body) = get("/catalog/items/doc").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains(r#""total":1"#), "body: {body}");
        assert!(body.contains(r#""id":"a/b""#), "body: {body}");
    }

    #[tokio::test]
    async fn gets_resource_with_slash_id_and_relationships() {
        // The wildcard id route carries the slash.
        let (status, body) = get("/catalog/items/doc/a/b").await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            body.contains(r#""attributes":{"title":"A/B"}"#),
            "body: {body}"
        );
        assert!(body.contains(r#""relationships""#), "body: {body}");
        assert!(body.contains(r#""id":"c/d""#), "body: {body}");
    }

    #[tokio::test]
    async fn unknown_resource_is_404() {
        let (status, _) = get("/catalog/items/doc/nope").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}
