// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Demo MCP server on rust-mcp-core's web UI: serves the shell, exposes its MCP tools (the
//! built-in Operations console invokes them over /mcp), backs the built-in Catalog + Search views
//! with in-memory providers, and serves a small consumer frontend at /app.
//!
//! The Catalog is a typed `DataCatalog` -- the shape real consumers (the BMF / gesetze servers)
//! use: one `record` entity type, faceted by `kind`, with a `related` to-one relation and
//! markdown / table attributes (via `x-mcp-kind`), plus a per-item action that calls the `echo`
//! tool over /mcp. Search hits carry an `entity` content ref, so selecting one opens the typed
//! detail view. This keeps the example a faithful contract test for the interfaces consumers
//! depend on (see web/e2e/shell).
//!
//! Run: `cargo run -p mcp-ui-demo`, then open http://127.0.0.1:8080/.

use std::collections::BTreeMap;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use mcp_core::web::{
    app_router, data_catalog_router, search_router, serve, Cardinality, CatalogPage, CatalogQuery,
    DataCatalog, EntityAction, EntityType, Landing, Relationship, Resource, ResourceRef, SearchHit,
    SearchProvider, SearchQuery, SearchResults,
};
use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler};
use schemars::{schema_for, JsonSchema};
use serde::Serialize;
use serde_json::{json, Value};
use tower_http::services::ServeDir;

#[derive(serde::Deserialize, JsonSchema)]
struct EchoParams {
    /// The text to echo back.
    message: String,
}

#[derive(serde::Deserialize, JsonSchema)]
struct AddParams {
    /// First addend.
    a: i64,
    /// Second addend.
    b: i64,
}

#[derive(Clone)]
struct Demo {
    // The conventional rmcp tool-router field, populated by the `#[tool_router]` macro.
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

impl Demo {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl Demo {
    #[tool(description = "Echo a message back to the caller.")]
    async fn echo(
        &self,
        Parameters(p): Parameters<EchoParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(p.message)]))
    }

    #[tool(description = "Add two integers and return the sum.")]
    async fn add(&self, Parameters(p): Parameters<AddParams>) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(
            (p.a + p.b).to_string(),
        )]))
    }
}

#[tool_handler]
impl ServerHandler for Demo {
    fn get_info(&self) -> ServerInfo {
        // ServerInfo is #[non_exhaustive], so build from Default and set fields.
        let mut info = ServerInfo::default();
        info.instructions = Some("rust-mcp-core web UI demo".to_string());
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info
    }
}

/// Attributes of a demo `record`. `summary` (markdown) and `metrics` (a table) carry `x-mcp-kind`
/// hints so the shell's field renderers format them; `kind` is the faceted dropdown.
#[derive(Serialize, JsonSchema)]
struct RecordAttributes {
    title: String,
    kind: String,
    summary: String,
    metrics: Metrics,
}

/// The shell's `{ caption, headers, rows }` table-field shape.
#[derive(Serialize, JsonSchema)]
struct Metrics {
    caption: String,
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

/// One in-memory record plus the id of the record it relates to (the `related` to-one relation).
struct Record {
    id: &'static str,
    title: &'static str,
    kind: &'static str,
    summary: &'static str,
    related: &'static str,
}

const RECORDS: &[Record] = &[
    Record {
        id: "alpha",
        title: "Alpha record",
        kind: "primary",
        summary: "# Alpha\n\nThe **first** record.\n\n- one\n- two",
        related: "beta",
    },
    Record {
        id: "beta",
        title: "Beta record",
        kind: "secondary",
        summary: "# Beta\n\nThe **second** record, related to Alpha.",
        related: "gamma",
    },
    Record {
        id: "gamma",
        title: "Gamma record",
        kind: "primary",
        summary: "# Gamma\n\nThe **third** record.",
        related: "alpha",
    },
];

fn find_record(id: &str) -> Option<&'static Record> {
    RECORDS.iter().find(|r| r.id == id)
}

fn record_ref(r: &Record) -> ResourceRef {
    ResourceRef {
        ty: "record".to_string(),
        id: r.id.to_string(),
        title: Some(r.title.to_string()),
        ..Default::default()
    }
}

/// A typed catalog (`DataCatalog`) over the in-memory demo records -- the shape real consumers use.
struct DemoDataCatalog;

impl DemoDataCatalog {
    fn record_type() -> EntityType {
        let mut attributes =
            serde_json::to_value(schema_for!(RecordAttributes)).unwrap_or_else(|_| json!({}));
        if let Some(props) = attributes
            .pointer_mut("/properties")
            .and_then(Value::as_object_mut)
        {
            // `kind` is one of two values -> render the facet as a dropdown.
            if let Some(kind) = props.get_mut("kind").and_then(Value::as_object_mut) {
                kind.insert("enum".to_string(), json!(["primary", "secondary"]));
            }
            if let Some(summary) = props.get_mut("summary").and_then(Value::as_object_mut) {
                summary.insert("x-mcp-kind".to_string(), json!("markdown"));
            }
            if let Some(metrics) = props.get_mut("metrics").and_then(Value::as_object_mut) {
                metrics.insert("x-mcp-kind".to_string(), json!("table"));
            }
        }
        EntityType {
            name: "record".to_string(),
            title: "Record".to_string(),
            title_field: Some("title".to_string()),
            list: true,
            attributes,
            relationships: vec![Relationship::new(
                "related",
                "Related",
                "record",
                Cardinality::ToOne,
            )],
            facets: vec!["kind".to_string()],
            // A per-item action: calls the `echo` MCP tool over /mcp with the row id as `message`.
            item_actions: vec![EntityAction {
                id: "echo".to_string(),
                label: "Echo".to_string(),
                tool: "echo".to_string(),
                arg: Some("message".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        }
    }
}

impl DataCatalog for DemoDataCatalog {
    fn schema(&self) -> Vec<EntityType> {
        vec![Self::record_type()]
    }

    async fn list(&self, ty: &str, query: CatalogQuery) -> CatalogPage {
        if ty != "record" {
            return CatalogPage {
                items: vec![],
                total: Some(0),
            };
        }
        let kind = query.filter.get("kind").cloned();
        let q = query.q.unwrap_or_default().to_lowercase();
        let all: Vec<ResourceRef> = RECORDS
            .iter()
            .filter(|r| match &kind {
                Some(k) => k.as_str() == r.kind,
                None => true,
            })
            .filter(|r| q.is_empty() || r.title.to_lowercase().contains(q.as_str()))
            .map(record_ref)
            .collect();
        let total = all.len();
        let items = all
            .into_iter()
            .skip(query.offset)
            .take(query.limit)
            .collect();
        CatalogPage {
            items,
            total: Some(total),
        }
    }

    async fn get(&self, ty: &str, id: &str) -> Option<Resource> {
        if ty != "record" {
            return None;
        }
        let r = find_record(id)?;
        let attributes = serde_json::to_value(RecordAttributes {
            title: r.title.to_string(),
            kind: r.kind.to_string(),
            summary: r.summary.to_string(),
            metrics: Metrics {
                caption: format!("{} metrics", r.title),
                headers: vec!["Metric".to_string(), "Value".to_string()],
                rows: vec![
                    vec!["score".to_string(), "42".to_string()],
                    vec!["rank".to_string(), "1".to_string()],
                ],
            },
        })
        .ok()?;
        let mut relationships: BTreeMap<String, Vec<ResourceRef>> = BTreeMap::new();
        if let Some(rel) = find_record(r.related) {
            relationships.insert("related".to_string(), vec![record_ref(rel)]);
        }
        Some(Resource {
            ty: "record".to_string(),
            id: id.to_string(),
            attributes,
            relationships,
        })
    }
}

/// A tiny search backend over the same demo records. Each hit's `content` points back at the
/// record entity, so selecting it opens the typed detail view (the search -> entity path).
struct DemoSearch;

impl SearchProvider for DemoSearch {
    async fn search(&self, query: SearchQuery) -> SearchResults {
        let q = query.q.to_lowercase();
        let hits: Vec<SearchHit> = RECORDS
            .iter()
            .filter(|r| {
                q.is_empty() || r.id.contains(q.as_str()) || r.title.to_lowercase().contains(&q)
            })
            .map(|r| SearchHit {
                id: r.id.to_string(),
                title: r.title.to_string(),
                subtitle: Some("record".to_string()),
                snippet: None,
                content: Some(json!({ "type": "entity", "entityType": "record", "id": r.id })),
            })
            .collect();
        let total = hits.len();
        SearchResults {
            hits,
            total: Some(total),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    mcp_core::init_tracing("mcp_ui_demo=debug,mcp_core=info");

    // MCP over Streamable HTTP at /mcp.
    let mcp = mcp_core::streamable_http_router(Demo::new, "/mcp");

    // Assemble the whole app through `app_router` so one auth policy covers the UI, the REST API,
    // AND /mcp together: a public landing/login page at /, the shell at /ui/, this crate's plain-JS
    // consumer content at /app, the typed catalog + search at /api (backing the built-in Catalog +
    // Search views), and /mcp. Set AUTH_TOKEN to require a credential -- the browser logs in at /
    // (a SameSite=Strict session cookie), then reaches /ui + /mcp same-origin; MCP clients send
    // `Authorization: Bearer`. Unset (the default) leaves the demo open. The landing page lists the
    // enabled transports; here only Streamable HTTP /mcp (hence `.mcp(true)`).
    let web_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("web");
    let api =
        data_catalog_router(Arc::new(DemoDataCatalog)).merge(search_router(Arc::new(DemoSearch)));
    let extra = mcp.nest_service("/app", ServeDir::new(web_dir));
    let auth_token = std::env::var("AUTH_TOKEN").ok();
    let app: Router = app_router(
        api,
        "/api",
        extra,
        auth_token.as_deref(),
        Landing::new("mcp-ui-demo").mcp(true),
    );

    let listen: Vec<IpAddr> = vec!["127.0.0.1".parse().unwrap()];
    tracing::info!("landing on http://127.0.0.1:8080/   UI on /ui/   MCP on /mcp");
    serve(app, listen, 8080).await?;
    Ok(())
}
