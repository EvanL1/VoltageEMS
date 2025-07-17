use axum::{
    extract::Request,
    http::header,
    middleware::Next,
    response::{IntoResponse, Response},
};
use futures::future::BoxFuture;
use log::debug;
use std::task::{Context, Poll};
use tower::{Layer, Service};

use super::{check_permission, jwt::JwtManager, Claims, Permission};
use crate::error::ApiError;

// JWT Authentication Layer
pub fn jwt_auth_layer() -> JwtAuthLayer {
    JwtAuthLayer
}

#[derive(Clone)]
pub struct JwtAuthLayer;

impl<S> Layer<S> for JwtAuthLayer {
    type Service = JwtAuthMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        JwtAuthMiddleware { inner }
    }
}

#[derive(Clone)]
pub struct JwtAuthMiddleware<S> {
    inner: S,
}

impl<S> Service<Request> for JwtAuthMiddleware<S>
where
    S: Service<Request, Response = Response> + Send + 'static + Clone,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut request: Request) -> Self::Future {
        let mut inner = self.inner.clone();

        Box::pin(async move {
            // Skip auth for health check endpoints
            let path = request.uri().path();
            if path == "/health" || path.starts_with("/health") {
                return inner.call(request).await;
            }

            // Extract token from Authorization header
            let auth_header = request.headers().get(header::AUTHORIZATION);

            if let Some(auth_value) = auth_header {
                if let Ok(auth_str) = auth_value.to_str() {
                    if auth_str.starts_with("Bearer ") {
                        let token = &auth_str[7..];

                        match JwtManager::verify_token(token) {
                            Ok(claims) => {
                                debug!("Token verified for user: {}", claims.username);
                                request.extensions_mut().insert(claims);
                                return inner.call(request).await;
                            }
                            Err(e) => {
                                debug!("Token verification failed: {}", e);
                            }
                        }
                    }
                }
            }

            // Try to extract token from query parameters
            if let Some(query) = request.uri().query() {
                for param in query.split('&') {
                    if let Some(token) = param.strip_prefix("token=") {
                        match JwtManager::verify_token(token) {
                            Ok(claims) => {
                                debug!("Token verified for user (from query): {}", claims.username);
                                request.extensions_mut().insert(claims);
                                return inner.call(request).await;
                            }
                            Err(e) => {
                                debug!("Token verification failed (from query): {}", e);
                            }
                        }
                    }
                }
            }

            // Try to extract token from Cookie header
            if let Some(cookie_header) = request.headers().get(header::COOKIE) {
                if let Ok(cookie_str) = cookie_header.to_str() {
                    for cookie in cookie_str.split(';') {
                        let cookie = cookie.trim();
                        if let Some(token) = cookie.strip_prefix("token=") {
                            match JwtManager::verify_token(token) {
                                Ok(claims) => {
                                    debug!("Token verified for user (from cookie): {}", claims.username);
                                    request.extensions_mut().insert(claims);
                                    return inner.call(request).await;
                                }
                                Err(e) => {
                                    debug!("Token verification failed (from cookie): {}", e);
                                }
                            }
                        }
                    }
                }
            }

            // Return unauthorized error
            Ok(ApiError::Unauthorized.into_response())
        })
    }
}

// Permission middleware function
pub async fn require_permission(
    permission: Permission,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    // Get claims from request extensions
    let has_permission = request
        .extensions()
        .get::<Claims>()
        .map(|claims| check_permission(&claims.roles, permission))
        .unwrap_or(false);

    if has_permission {
        Ok(next.run(request).await)
    } else {
        Err(ApiError::Forbidden)
    }
}

// Helper function to extract claims from request
pub fn extract_claims(request: &Request) -> Option<&Claims> {
    request.extensions().get::<Claims>()
}