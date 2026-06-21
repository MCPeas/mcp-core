// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The search contract behind the shell's built-in Search view: a [`SearchProvider`] the
//! server implements and [`search_router`] exposing `{api_base}/search`. Mount it in the
//! server's `api_router` next to [`super::catalog_router`].

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

/// A query from the built-in Search view: free text plus consumer-defined `filters` and
/// paging, POSTed as JSON to `{api_base}/search`.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct SearchQuery {
    #[serde(default)]
    pub q: String,
    /// Free-form filters the provider interprets (e.g. a scope or category). The built-in
    /// view sends none in v1; a provider may still accept them from a richer UI.
    #[serde(default)]
    pub filters: BTreeMap<String, String>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
}

/// One search result. `content` is the content the shell opens when the hit is selected
/// (any ContentRef, as JSON); when `None`, the view defaults to the hit's catalog item
/// (`{ "type": "catalog-item", "id": <id> }`).
#[derive(Clone, Debug, serde::Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS), ts(export))]
pub struct SearchHit {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub subtitle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub snippet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional, type = "unknown"))]
    pub content: Option<serde_json::Value>,
}

/// The results of a [`SearchQuery`].
#[derive(Clone, Debug, Default, serde::Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS), ts(export))]
pub struct SearchResults {
    pub hits: Vec<SearchHit>,
    /// Total matches (for paging UIs); `None` if the provider does not count.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub total: Option<usize>,
}

/// A server's search backend, surfaced by the shell's built-in Search view. Implementors
/// write `async fn search(...)`; `+ Send` keeps the future usable from axum handlers.
pub trait SearchProvider: Send + Sync + 'static {
    fn search(&self, query: SearchQuery)
        -> impl std::future::Future<Output = SearchResults> + Send;
}

/// A router exposing `GET /search` (an availability probe, so the shell shows the Search
/// view only when the server has one) and `POST /search` (run a query). Mount it inside
/// your `api_router`; the view posts to `{api_base}/search`.
pub fn search_router<P: SearchProvider>(provider: Arc<P>) -> Router {
    Router::new()
        .route("/search", get(probe).post(run::<P>))
        .with_state(provider)
}

async fn probe() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "search": true }))
}

async fn run<P: SearchProvider>(
    State(provider): State<Arc<P>>,
    Json(query): Json<SearchQuery>,
) -> Json<SearchResults> {
    Json(provider.search(query).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    struct TestSearch;

    impl SearchProvider for TestSearch {
        async fn search(&self, query: SearchQuery) -> SearchResults {
            let hits: Vec<SearchHit> = ["alpha", "beta"]
                .into_iter()
                .filter(|id| query.q.is_empty() || id.contains(query.q.as_str()))
                .map(|id| SearchHit {
                    id: id.to_string(),
                    title: id.to_uppercase(),
                    subtitle: None,
                    snippet: None,
                    content: None,
                })
                .collect();
            let total = hits.len();
            SearchResults {
                hits,
                total: Some(total),
            }
        }
    }

    fn app() -> Router {
        search_router(Arc::new(TestSearch))
    }

    async fn body_string(res: axum::http::Response<Body>) -> String {
        let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[tokio::test]
    async fn probe_reports_available() {
        let res = app()
            .oneshot(
                Request::builder()
                    .uri("/search")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert!(body_string(res).await.contains("\"search\":true"));
    }

    #[tokio::test]
    async fn query_returns_matching_hits() {
        let res = app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/search")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"q":"alph"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let text = body_string(res).await;
        assert!(text.contains("\"id\":\"alpha\""), "alpha present: {text}");
        assert!(
            !text.contains("\"id\":\"beta\""),
            "beta filtered out: {text}"
        );
        assert!(text.contains("\"total\":1"), "total counted: {text}");
    }
}
