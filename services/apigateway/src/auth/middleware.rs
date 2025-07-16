use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    error::ErrorUnauthorized,
    http::header,
    Error, HttpMessage,
};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use futures::future::{ok, Ready};
use futures::Future;
use log::debug;
use std::pin::Pin;
use std::rc::Rc;

use super::{check_permission, jwt::JwtManager, Permission};

pub struct JwtAuthMiddleware;

impl<S, B> Transform<S, ServiceRequest> for JwtAuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = JwtAuthMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(JwtAuthMiddlewareService {
            service: Rc::new(service),
        })
    }
}

pub struct JwtAuthMiddlewareService<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for JwtAuthMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = self.service.clone();

        Box::pin(async move {
            // Skip auth for health check endpoints
            let path = req.path();
            if path == "/health" || path.starts_with("/health") {
                return service.call(req).await;
            }

            // Extract token from Authorization header
            let auth_header = req.headers().get(header::AUTHORIZATION);

            if let Some(auth_value) = auth_header {
                if let Ok(auth_str) = auth_value.to_str() {
                    if auth_str.starts_with("Bearer ") {
                        let token = &auth_str[7..];

                        match JwtManager::verify_token(token) {
                            Ok(claims) => {
                                debug!("Token verified for user: {}", claims.username);
                                req.extensions_mut().insert(claims);
                                return service.call(req).await;
                            }
                            Err(e) => {
                                debug!("Token verification failed: {}", e);
                            }
                        }
                    }
                }
            }

            Err(ErrorUnauthorized("Invalid or missing authentication token"))
        })
    }
}

pub struct RequirePermission {
    permission: Permission,
}

impl RequirePermission {
    pub fn new(permission: Permission) -> Self {
        Self { permission }
    }
}

impl<S, B> Transform<S, ServiceRequest> for RequirePermission
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = RequirePermissionService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(RequirePermissionService {
            service: Rc::new(service),
            permission: self.permission.clone(),
        })
    }
}

pub struct RequirePermissionService<S> {
    service: Rc<S>,
    permission: Permission,
}

impl<S, B> Service<ServiceRequest> for RequirePermissionService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = self.service.clone();
        let required_permission = self.permission.clone();

        Box::pin(async move {
            // Get claims from request extensions
            let has_permission = req
                .extensions()
                .get::<super::Claims>()
                .map(|claims| check_permission(&claims.roles, required_permission))
                .unwrap_or(false);

            if has_permission {
                service.call(req).await
            } else {
                Err(ErrorUnauthorized("Insufficient permissions"))
            }
        })
    }
}

pub async fn bearer_auth_validator(
    req: ServiceRequest,
    credentials: BearerAuth,
) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    match JwtManager::verify_token(credentials.token()) {
        Ok(claims) => {
            req.extensions_mut().insert(claims);
            Ok(req)
        }
        Err(_) => Err((ErrorUnauthorized("Invalid token"), req)),
    }
}
