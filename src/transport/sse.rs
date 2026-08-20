// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Custom SSE server for MCP with authentication support.
//!
//! This reimplements rmcp's SSE server logic to allow wrapping with auth middleware.
//! Sessions are cleaned up when the SSE connection drops (client disconnect).

use std::{collections::HashMap, sync::Arc, time::Duration};

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::sse::{Event, Sse},
    routing::{get, post},
    Json, Router,
};
use futures::{Sink, SinkExt, Stream, StreamExt};
use rmcp::{
    model::ClientJsonRpcMessage,
    service::{RxJsonRpcMessage, TxJsonRpcMessage},
    RoleServer,
};
use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::PollSender;

type SessionId = Arc<str>;
type TxStore = Arc<RwLock<HashMap<SessionId, mpsc::Sender<ClientJsonRpcMessage>>>>;

/// Shared application state for SSE server
#[derive(Clone)]
struct SseApp {
    txs: TxStore,
    transport_tx: mpsc::UnboundedSender<SseTransport>,
    post_path: Arc<str>,
}

/// Transport for a single SSE session.
///
/// Implements both `Sink` and `Stream` traits for bidirectional
/// communication with MCP clients.
pub struct SseTransport {
    stream: ReceiverStream<RxJsonRpcMessage<RoleServer>>,
    sink: PollSender<TxJsonRpcMessage<RoleServer>>,
    session_id: SessionId,
    tx_store: TxStore,
}

impl Sink<TxJsonRpcMessage<RoleServer>> for SseTransport {
    type Error = std::io::Error;

    fn poll_ready(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.sink
            .poll_ready_unpin(cx)
            .map_err(std::io::Error::other)
    }

    fn start_send(
        mut self: std::pin::Pin<&mut Self>,
        item: TxJsonRpcMessage<RoleServer>,
    ) -> Result<(), Self::Error> {
        self.sink
            .start_send_unpin(item)
            .map_err(std::io::Error::other)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.sink
            .poll_flush_unpin(cx)
            .map_err(std::io::Error::other)
    }

    fn poll_close(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        let result = self
            .sink
            .poll_close_unpin(cx)
            .map_err(std::io::Error::other);

        if result.is_ready() {
            let session_id = self.session_id.clone();
            let tx_store = self.tx_store.clone();
            tokio::spawn(async move {
                tx_store.write().await.remove(&session_id);
            });
        }
        result
    }
}

impl Stream for SseTransport {
    type Item = RxJsonRpcMessage<RoleServer>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.stream.poll_next_unpin(cx)
    }
}

fn generate_session_id() -> SessionId {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let random: u64 = rand::random();
    Arc::from(format!("{:016x}{:016x}", timestamp, random))
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PostEventQuery {
    session_id: String,
}

async fn post_event_handler(
    State(app): State<SseApp>,
    Query(PostEventQuery { session_id }): Query<PostEventQuery>,
    Json(message): Json<ClientJsonRpcMessage>,
) -> Result<StatusCode, StatusCode> {
    tracing::debug!(session_id, ?message, "received client message");

    let tx = {
        let store = app.txs.read().await;
        store
            .get(session_id.as_str())
            .ok_or(StatusCode::NOT_FOUND)?
            .clone()
    };

    if tx.send(message).await.is_err() {
        // Session dropped — clean up stale entry
        app.txs.write().await.remove(session_id.as_str());
        return Err(StatusCode::GONE);
    }

    Ok(StatusCode::ACCEPTED)
}

/// Guard that removes the session from TxStore when the SSE stream is dropped.
///
/// When a client disconnects (closes the browser, network drop, etc.),
/// Axum drops the `Sse` response stream. This guard runs cleanup on drop,
/// ensuring stale sessions don't accumulate.
struct SessionDropGuard {
    session_id: SessionId,
    tx_store: TxStore,
}

impl Drop for SessionDropGuard {
    fn drop(&mut self) {
        let session_id = self.session_id.clone();
        let tx_store = self.tx_store.clone();
        tracing::debug!(%session_id, "SSE connection dropped, cleaning up session");
        tokio::spawn(async move {
            tx_store.write().await.remove(&session_id);
        });
    }
}

async fn sse_handler(
    State(app): State<SseApp>,
) -> Result<Sse<impl Stream<Item = Result<Event, std::io::Error>>>, (StatusCode, String)> {
    let session_id = generate_session_id();
    tracing::info!(%session_id, "new SSE connection");

    let (from_client_tx, from_client_rx) = mpsc::channel(64);
    let (to_client_tx, to_client_rx) = mpsc::channel(64);

    app.txs
        .write()
        .await
        .insert(session_id.clone(), from_client_tx);

    let stream = ReceiverStream::new(from_client_rx);
    let sink = PollSender::new(to_client_tx);

    let transport = SseTransport {
        stream,
        sink,
        session_id: session_id.clone(),
        tx_store: app.txs.clone(),
    };

    if app.transport_tx.send(transport).is_err() {
        tracing::warn!("failed to send transport - server may be closing");
        app.txs.write().await.remove(&session_id);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "server is closing".to_string(),
        ));
    }

    let post_path = app.post_path.as_ref();
    let endpoint_event = Event::default()
        .event("endpoint")
        .data(format!("{post_path}?sessionId={session_id}"));

    // The drop guard ensures session cleanup when the SSE stream is dropped
    // (client disconnect, network interruption, etc.)
    let drop_guard = SessionDropGuard {
        session_id,
        tx_store: app.txs.clone(),
    };

    let message_stream =
        ReceiverStream::new(to_client_rx).map(|message| match serde_json::to_string(&message) {
            Ok(json) => Ok(Event::default().event("message").data(&json)),
            Err(e) => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
        });

    let stream = futures::stream::once(futures::future::ok(endpoint_event))
        .chain(message_stream)
        // Keep the drop guard alive for the lifetime of the stream.
        // When the stream is dropped (client disconnects), the guard
        // fires and removes the session from TxStore.
        .chain(futures::stream::once(async move {
            drop(drop_guard);
            Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                "stream ended",
            ))
        }));

    Ok(Sse::new(stream)
        .keep_alive(axum::response::sse::KeepAlive::new().interval(Duration::from_secs(30))))
}

/// SSE server that can be wrapped with authentication middleware.
///
/// # Example
///
/// ```rust,ignore
/// use mcp_core::{AuthSseServer, TokenAuthLayer};
///
/// let (mut sse_server, sse_router) = AuthSseServer::new();
///
/// // Wrap with auth middleware
/// let protected_router = sse_router.layer(TokenAuthLayer::new("secret".to_string()));
///
/// // Accept connections in a loop
/// while let Some(transport) = sse_server.next_transport().await {
///     // Handle the transport...
/// }
/// ```
pub struct AuthSseServer {
    transport_rx: mpsc::UnboundedReceiver<SseTransport>,
}

impl AuthSseServer {
    /// Create a new SSE server with routes at `/sse` and `/message`.
    ///
    /// Returns a tuple of `(server, router)` where:
    /// - `server` is used to accept new transports via `next_transport()`
    /// - `router` contains the SSE endpoints and can be layered with middleware
    ///
    /// Equivalent to [`AuthSseServer::with_base_path`] with an empty base.
    pub fn new() -> (Self, Router) {
        Self::with_base_path("")
    }

    /// Create a new SSE server whose routes are mounted under `base_path`.
    ///
    /// For example `with_base_path("/mcp")` serves the event stream at
    /// `GET /mcp/sse` and the client POST endpoint at `POST /mcp/message`, and
    /// advertises `/mcp/message` in the SSE `endpoint` event so the router can be
    /// merged directly into a larger app without axum nesting. An empty
    /// `base_path` is equivalent to [`AuthSseServer::new`] (`/sse` + `/message`).
    pub fn with_base_path(base_path: &str) -> (Self, Router) {
        let (transport_tx, transport_rx) = mpsc::unbounded_channel();

        let base = base_path.trim_end_matches('/');
        let sse_path = format!("{base}/sse");
        let message_path = format!("{base}/message");

        let app = SseApp {
            txs: Arc::new(RwLock::new(HashMap::new())),
            transport_tx,
            // Advertise the same path the POST handler is mounted at, so clients
            // POST to the correct place regardless of the configured base.
            post_path: Arc::from(message_path.as_str()),
        };

        let router = Router::new()
            .route(&sse_path, get(sse_handler))
            .route(&message_path, post(post_event_handler))
            .with_state(app);

        (Self { transport_rx }, router)
    }

    /// Wait for the next transport (new SSE connection).
    ///
    /// Returns `None` when all router clones have been dropped.
    pub async fn next_transport(&mut self) -> Option<SseTransport> {
        self.transport_rx.recv().await
    }
}

impl Default for AuthSseServer {
    fn default() -> Self {
        Self::new().0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, BodyDataStream};
    use axum::http::Request;
    use tower::util::ServiceExt;

    /// Reads SSE events (`...\n\n`-terminated) off a response body, buffering across chunks.
    struct EventReader {
        stream: BodyDataStream,
        buf: String,
    }

    impl EventReader {
        fn new(body: Body) -> Self {
            Self {
                stream: body.into_data_stream(),
                buf: String::new(),
            }
        }

        async fn next_event(&mut self) -> String {
            loop {
                if let Some(end) = self.buf.find("\n\n") {
                    let event = self.buf[..end].to_string();
                    self.buf.drain(..end + 2);
                    return event;
                }
                let chunk = self
                    .stream
                    .next()
                    .await
                    .expect("SSE stream ended without a full event")
                    .expect("SSE stream errored");
                self.buf.push_str(std::str::from_utf8(&chunk).unwrap());
            }
        }
    }

    /// Opens the SSE stream and returns the advertised session id, the endpoint-event data
    /// line, and the reader positioned after the endpoint event.
    async fn open_sse(router: &Router, sse_path: &str) -> (String, String, EventReader) {
        let res = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(sse_path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let mut reader = EventReader::new(res.into_body());
        let event = reader.next_event().await;
        assert!(event.contains("event: endpoint"), "event: {event}");
        let data = event
            .lines()
            .find_map(|l| l.strip_prefix("data: "))
            .expect("endpoint event carries a data line")
            .to_string();
        let session_id = data
            .split_once("sessionId=")
            .expect("endpoint data carries the session id")
            .1
            .to_string();
        (session_id, data, reader)
    }

    async fn post_message(router: &Router, path_query: &str) -> StatusCode {
        router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path_query)
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#))
                    .unwrap(),
            )
            .await
            .unwrap()
            .status()
    }

    /// The cleanup paths remove sessions from a spawned task; poll until the store agrees.
    async fn eventually(router: &Router, path_query: &str, want: StatusCode) {
        for _ in 0..500 {
            if post_message(router, path_query).await == want {
                return;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        panic!("POST {path_query} never returned {want}");
    }

    #[test]
    fn session_ids_are_32_hex_chars_and_unique() {
        let a = generate_session_id();
        let b = generate_session_id();
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[tokio::test]
    async fn endpoint_event_advertises_the_post_path() {
        let (_server, router) = AuthSseServer::new();
        let (session_id, data, _reader) = open_sse(&router, "/sse").await;
        assert_eq!(data, format!("/message?sessionId={session_id}"));
    }

    #[tokio::test]
    async fn base_path_prefixes_routes_and_advertisement() {
        let (_server, router) = AuthSseServer::with_base_path("/mcp");
        let (session_id, data, _reader) = open_sse(&router, "/mcp/sse").await;
        assert_eq!(data, format!("/mcp/message?sessionId={session_id}"));
    }

    #[tokio::test]
    async fn post_to_unknown_session_is_404() {
        let (_server, router) = AuthSseServer::new();
        assert_eq!(
            post_message(&router, "/message?sessionId=nope").await,
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn post_routes_the_message_to_the_transport() {
        let (mut server, router) = AuthSseServer::new();
        let (session_id, _, _reader) = open_sse(&router, "/sse").await;
        let mut transport = server.next_transport().await.expect("one transport");

        let status = post_message(&router, &format!("/message?sessionId={session_id}")).await;
        assert_eq!(status, StatusCode::ACCEPTED);

        let received = transport.next().await.expect("client message arrives");
        let value = serde_json::to_value(&received).unwrap();
        assert_eq!(value["method"], "ping");
    }

    #[tokio::test]
    async fn transport_sink_streams_messages_to_the_client() {
        let (mut server, router) = AuthSseServer::new();
        let (_, _, mut reader) = open_sse(&router, "/sse").await;
        let mut transport = server.next_transport().await.expect("one transport");

        let message: TxJsonRpcMessage<RoleServer> =
            serde_json::from_value(serde_json::json!({"jsonrpc":"2.0","id":1,"method":"ping"}))
                .unwrap();
        futures::SinkExt::send(&mut transport, message)
            .await
            .unwrap();

        let event = reader.next_event().await;
        assert!(event.contains("event: message"), "event: {event}");
        assert!(event.contains(r#""method":"ping""#), "event: {event}");
    }

    #[tokio::test]
    async fn client_disconnect_cleans_up_the_session() {
        let (_server, router) = AuthSseServer::new();
        let (session_id, _, reader) = open_sse(&router, "/sse").await;
        let uri = format!("/message?sessionId={session_id}");
        assert_eq!(post_message(&router, &uri).await, StatusCode::ACCEPTED);

        // Dropping the response body is the client disconnect; the drop guard fires.
        drop(reader);
        eventually(&router, &uri, StatusCode::NOT_FOUND).await;
    }

    #[tokio::test]
    async fn transport_close_cleans_up_the_session() {
        let (mut server, router) = AuthSseServer::new();
        let (session_id, _, _reader) = open_sse(&router, "/sse").await;
        let mut transport = server.next_transport().await.expect("one transport");
        let uri = format!("/message?sessionId={session_id}");
        assert_eq!(post_message(&router, &uri).await, StatusCode::ACCEPTED);

        futures::SinkExt::close(&mut transport).await.unwrap();
        eventually(&router, &uri, StatusCode::NOT_FOUND).await;
    }

    #[tokio::test]
    async fn next_transport_ends_when_the_router_is_dropped() {
        let (mut server, router) = AuthSseServer::new();
        drop(router);
        assert!(server.next_transport().await.is_none());
    }
}
