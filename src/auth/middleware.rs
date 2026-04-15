//! Token authentication middleware implementation.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    response::Response,
};
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tower::{Layer, Service};

/// Layer that adds token authentication to a service.
///
/// # Example
///
/// ```rust,ignore
/// use mcp_core::TokenAuthLayer;
///
/// let router = Router::new()
///     .route("/api", get(handler))
///     .layer(TokenAuthLayer::new("my-secret-token".to_string()));
/// ```
#[derive(Clone)]
pub struct TokenAuthLayer {
    token: Arc<str>,
    realm: Arc<str>,
}

impl TokenAuthLayer {
    /// Create a new token auth layer with the given token.
    pub fn new(token: String) -> Self {
        Self {
            token: Arc::from(token),
            realm: Arc::from("mcp-core"),
        }
    }

    /// Create a new token auth layer with a custom realm.
    pub fn with_realm(token: String, realm: String) -> Self {
        Self {
            token: Arc::from(token),
            realm: Arc::from(realm),
        }
    }
}

impl<S> Layer<S> for TokenAuthLayer {
    type Service = TokenAuthService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TokenAuthService {
            inner,
            token: self.token.clone(),
            realm: self.realm.clone(),
        }
    }
}

/// Service that validates token authentication.
///
/// Accepts authentication via:
/// - Bearer token: `Authorization: Bearer <token>`
/// - Basic Auth: Any username with token as password
#[derive(Clone)]
pub struct TokenAuthService<S> {
    inner: S,
    token: Arc<str>,
    realm: Arc<str>,
}

impl<S> Service<Request<Body>> for TokenAuthService<S>
where
    S: Service<Request<Body>, Response = Response> + Clone + Send + 'static,
    S::Future: Send,
{
    type Response = Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let token = self.token.clone();
        let realm = self.realm.clone();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            // Check Authorization header
            if let Some(auth_header) = req.headers().get(header::AUTHORIZATION) {
                if let Ok(auth_str) = auth_header.to_str() {
                    // Check Bearer token
                    if let Some(bearer_token) = auth_str.strip_prefix("Bearer ") {
                        if bearer_token == token.as_ref() {
                            return inner.call(req).await;
                        }
                    }

                    // Check Basic Auth (any username, token as password)
                    if let Some(basic_creds) = auth_str.strip_prefix("Basic ") {
                        if let Ok(decoded) = base64_decode(basic_creds) {
                            if let Some((_username, password)) = decoded.split_once(':') {
                                if password == token.as_ref() {
                                    return inner.call(req).await;
                                }
                            }
                        }
                    }
                }
            }

            // No valid auth - return 401
            let response = Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header(
                    header::WWW_AUTHENTICATE,
                    format!("Bearer, Basic realm=\"{}\"", realm),
                )
                .body(Body::from("Unauthorized"))
                .unwrap();

            Ok(response)
        })
    }
}

fn base64_decode(input: &str) -> Result<String, ()> {
    use std::io::Read;
    let mut decoder = base64::read::DecoderReader::new(
        input.as_bytes(),
        &base64::engine::general_purpose::STANDARD,
    );
    let mut decoded = String::new();
    decoder.read_to_string(&mut decoded).map_err(|_| ())?;
    Ok(decoded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, routing::get, Router};
    use tower::util::ServiceExt;

    async fn test_handler() -> &'static str {
        "OK"
    }

    fn create_test_router(token: &str) -> Router {
        Router::new()
            .route("/test", get(test_handler))
            .layer(TokenAuthLayer::new(token.to_string()))
    }

    #[tokio::test]
    async fn test_valid_bearer_token() {
        let app = create_test_router("test-token-not-a-real-secret");

        let request = Request::builder()
            .uri("/test")
            .header("Authorization", "Bearer test-token-not-a-real-secret")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_invalid_bearer_token() {
        let app = create_test_router("test-token-not-a-real-secret");

        let request = Request::builder()
            .uri("/test")
            .header("Authorization", "Bearer wrongtoken")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_valid_basic_auth() {
        let app = create_test_router("test-token-not-a-real-secret");

        // "user:test-token-not-a-real-secret" in base64
        let credentials = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            "user:test-token-not-a-real-secret",
        );

        let request = Request::builder()
            .uri("/test")
            .header("Authorization", format!("Basic {}", credentials))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_invalid_basic_auth() {
        let app = create_test_router("test-token-not-a-real-secret");

        // "user:wrong-token" in base64
        let credentials = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            "user:wrong-token",
        );

        let request = Request::builder()
            .uri("/test")
            .header("Authorization", format!("Basic {}", credentials))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_no_auth_header() {
        let app = create_test_router("test-token-not-a-real-secret");

        let request = Request::builder().uri("/test").body(Body::empty()).unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_custom_realm() {
        let app =
            Router::new()
                .route("/test", get(test_handler))
                .layer(TokenAuthLayer::with_realm(
                    "secret".to_string(),
                    "my-custom-realm".to_string(),
                ));

        let request = Request::builder().uri("/test").body(Body::empty()).unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let www_auth = response
            .headers()
            .get(header::WWW_AUTHENTICATE)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(www_auth.contains("my-custom-realm"));
    }
}
