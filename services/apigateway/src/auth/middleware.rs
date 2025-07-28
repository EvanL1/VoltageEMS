use axum::{
    extract::Request,
    http::header,
    response::{IntoResponse, Response},
};
use futures::future::BoxFuture;
use log::debug;
use std::task::{Context, Poll};
use tower::{Layer, Service};

use super::jwt::JwtManager;
use crate::error::ApiError;

// JWT Authentication Layer
pub fn auth_layer(jwt_secret: String) -> JwtAuthLayer {
    JwtAuthLayer { jwt_secret }
}

#[derive(Clone)]
pub struct JwtAuthLayer {
    jwt_secret: String,
}

impl<S> Layer<S> for JwtAuthLayer {
    type Service = JwtAuthMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        JwtAuthMiddleware {
            inner,
            jwt_secret: self.jwt_secret.clone(),
        }
    }
}

#[derive(Clone)]
pub struct JwtAuthMiddleware<S> {
    inner: S,
    #[allow(dead_code)]
    jwt_secret: String,
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
                    if let Some(token) = auth_str.strip_prefix("Bearer ") {
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
                                    debug!(
                                        "Token verified for user (from cookie): {}",
                                        claims.username
                                    );
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
