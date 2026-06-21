// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Information catalog: a server exposes a list of items and each item's structured
//! content; the shell's built-in Catalog view browses them (a sidebar list + the content
//! pane, with the JSON fallback so the data is always visible). Data flows over REST at
//! `{api_base}/catalog/...` -- the typed half of the hybrid model (operations come from
//! the MCP tool schemas; catalog + search come from the API).

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};

/// An item shown in the catalog's sidebar list.
#[derive(Clone, Debug, Default, serde::Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS), ts(export))]
pub struct CatalogItem {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub subtitle: Option<String>,
    /// Actions offered on the item (e.g. install / uninstall / update). Empty -> none shown.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<CatalogAction>,
}

/// An action on a [`CatalogItem`], wired to an MCP tool the shell invokes over `/mcp`
/// (`tools/call`). The shell merges the item's `id` into the call arguments. Mark mutating
/// actions `danger` so the UI confirms first. mcp-core stays domain-agnostic: it only relays
/// the named tool call; the consumer decides which tools (if any) an item exposes.
#[derive(Clone, Debug, Default, serde::Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS), ts(export))]
pub struct CatalogAction {
    pub id: String,
    pub label: String,
    /// The MCP tool to call.
    pub tool: String,
    /// Extra arguments merged into the call (the item id is added as `id`).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional, type = "Record<string, unknown>"))]
    pub args: Option<serde_json::Value>,
    /// Mutating/destructive: the UI confirms before running.
    #[serde(default, skip_serializing_if = "super::is_false")]
    pub danger: bool,
}

/// A server's information catalog. Implement it to expose the data the shell's built-in
/// Catalog view browses (laws, extracted records, ...). Async so implementations can query
/// a Tantivy index or a database.
pub trait CatalogProvider: Send + Sync + 'static {
    /// The items shown in the sidebar list.
    fn items(&self) -> impl std::future::Future<Output = Vec<CatalogItem>> + Send;

    /// The structured content for `id` as JSON, rendered by the shell's JSON fallback (or
    /// a registered override component). `None` becomes a 404.
    fn item(&self, id: &str)
        -> impl std::future::Future<Output = Option<serde_json::Value>> + Send;
}

/// A router exposing `GET /catalog/items` and `GET /catalog/items/{id}` backed by
/// `provider`. Mount it inside your `api_router` (the Catalog view fetches
/// `{api_base}/catalog/items` and `{api_base}/catalog/items/{id}`).
pub fn catalog_router<P: CatalogProvider>(provider: Arc<P>) -> Router {
    Router::new()
        .route("/catalog/items", get(list_items::<P>))
        .route("/catalog/items/{id}", get(get_item::<P>))
        .with_state(provider)
}

async fn list_items<P: CatalogProvider>(State(provider): State<Arc<P>>) -> Json<Vec<CatalogItem>> {
    Json(provider.items().await)
}

async fn get_item<P: CatalogProvider>(
    State(provider): State<Arc<P>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match provider.item(&id).await {
        Some(value) => Ok(Json(value)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::util::ServiceExt;

    struct TestCatalog;

    impl CatalogProvider for TestCatalog {
        async fn items(&self) -> Vec<CatalogItem> {
            vec![CatalogItem {
                id: "x".to_string(),
                title: "X".to_string(),
                subtitle: None,
                actions: vec![],
            }]
        }

        async fn item(&self, id: &str) -> Option<serde_json::Value> {
            (id == "x").then(|| serde_json::json!({ "id": "x", "ok": true }))
        }
    }

    async fn get(uri: &str) -> (StatusCode, String) {
        let res = catalog_router(Arc::new(TestCatalog))
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
    async fn lists_items() {
        let (status, body) = get("/catalog/items").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains(r#""id":"x""#), "body: {body}");
        // `subtitle: None` is omitted from the wire form.
        assert!(!body.contains("subtitle"), "body: {body}");
    }

    #[tokio::test]
    async fn returns_item_content() {
        let (status, body) = get("/catalog/items/x").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains(r#""ok":true"#), "body: {body}");
    }

    #[tokio::test]
    async fn unknown_item_is_404() {
        let (status, _) = get("/catalog/items/nope").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}
