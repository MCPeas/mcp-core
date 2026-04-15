//! Custom SSE server for MCP with authentication support.
//!
//! This reimplements rmcp's SSE server logic to allow wrapping with auth middleware.
//! Sessions are cleaned up when the SSE connection drops (client disconnect).

use std::{collections::HashMap, sync::Arc, time::Duration};

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{
        sse::{Event, Sse},
        Response,
    },
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
) -> Result<Sse<impl Stream<Item = Result<Event, std::io::Error>>>, Response<String>> {
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
        let mut response = Response::new("server is closing".to_string());
        *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
        return Err(response);
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
    /// Create a new SSE server and return the router that can be wrapped with middleware.
    ///
    /// Returns a tuple of `(server, router)` where:
    /// - `server` is used to accept new transports via `next_transport()`
    /// - `router` contains the SSE endpoints and can be layered with middleware
    pub fn new() -> (Self, Router) {
        let (transport_tx, transport_rx) = mpsc::unbounded_channel();

        let app = SseApp {
            txs: Arc::new(RwLock::new(HashMap::new())),
            transport_tx,
            post_path: Arc::from("/message"),
        };

        let router = Router::new()
            .route("/sse", get(sse_handler))
            .route("/message", post(post_event_handler))
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
