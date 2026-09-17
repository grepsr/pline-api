//! Streamable HTTP transport for remote MCP clients (Claude.ai, ChatGPT, Gemini, and any other
//! client that connects to an HTTPS URL).
//!
//! Every request must identify the caller's pline.ai API key. In order of preference:
//!
//! 1. `Authorization: Bearer <key>` header
//! 2. `x-api-key: <key>` header
//! 3. a key segment in the URL, `/<key>/mcp`, for clients whose connector settings accept a URL
//!    but no custom headers
//!
//! The key is stored in the request extensions as [`ApiKey`]; rmcp forwards the request parts into
//! every tool call, where `api_key_for` reads it back. Requests without a key are rejected with
//! `401` before they reach the MCP layer.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, Request};
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use rmcp::transport::streamable_http_server::session::never::NeverSessionManager;
use rmcp::transport::{StreamableHttpServerConfig, StreamableHttpService};
use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::PlineServer;

/// The caller's pline.ai API key for one HTTP request.
#[derive(Clone, Debug)]
pub struct ApiKey(pub String);

fn header_key(request: &Request) -> Option<String> {
    let bearer = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            value
                .strip_prefix("Bearer ")
                .or_else(|| value.strip_prefix("bearer "))
        })
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(key) = bearer {
        return Some(key.to_string());
    }
    request
        .headers()
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

async fn require_api_key(
    Path(params): Path<HashMap<String, String>>,
    mut request: Request,
    next: Next,
) -> Response {
    let key = header_key(&request).or_else(|| {
        params
            .get("api_key")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    });
    match key {
        Some(key) => {
            request.extensions_mut().insert(ApiKey(key));
            next.run(request).await
        }
        None => (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "detail": "missing pline.ai API key: send `Authorization: Bearer <key>` or `x-api-key`, \
                           or connect to /<key>/mcp"
            })),
        )
            .into_response(),
    }
}

async fn health() -> impl IntoResponse {
    Json(json!({"status": "ok", "service": "pline.ai"}))
}

fn allowed_hosts_from_env() -> Vec<String> {
    std::env::var("PLINE_MCP_ALLOWED_HOSTS")
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|host| !host.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::warn!(%error, "failed to install SIGINT handler");
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
            }
            Err(error) => {
                tracing::warn!(%error, "failed to install SIGTERM handler");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("received SIGINT, shutting down"),
        _ = terminate => tracing::info!("received SIGTERM, shutting down"),
    }
}

pub async fn serve(mut server: PlineServer, addr: &str) -> anyhow::Result<()> {
    server.allow_local_files = false;
    let session_cancellation = CancellationToken::new();
    let config = StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_allowed_hosts(allowed_hosts_from_env())
        .with_cancellation_token(session_cancellation.clone());
    let mcp = StreamableHttpService::new(
        move || Ok(server.clone()),
        Arc::new(NeverSessionManager::default()),
        config,
    );
    let app = Router::new()
        .route_service("/mcp", mcp.clone())
        .route_service("/{api_key}/mcp", mcp)
        .layer(middleware::from_fn(require_api_key))
        .route("/health", get(health));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "pline.ai MCP listening (Streamable HTTP at /mcp and /<api-key>/mcp)");
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            session_cancellation.cancel();
        })
        .await?;
    Ok(())
}
