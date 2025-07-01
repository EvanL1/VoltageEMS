use actix_web::{web, HttpRequest, HttpResponse};
use std::sync::Arc;

use crate::config::Config;
use crate::error::ApiResult;
use crate::handlers::proxy_request;

#[actix_web::route("{path:.*}", method = "GET", method = "POST", method = "PUT", method = "DELETE", method = "PATCH")]
pub async fn proxy_handler(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
    config: web::Data<Config>,
    client: web::Data<Arc<reqwest::Client>>,
) -> ApiResult<HttpResponse> {
    let method = req.method().as_str();
    let path_str = format!("/{}", path.as_str());
    
    proxy_request(
        "alarmsrv",
        &path_str,
        method,
        &req,
        Some(body),
        &config,
        &client,
    )
    .await
}