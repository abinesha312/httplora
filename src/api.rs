use crate::db::Database;
use crate::radio::{RadioBackend, SharedRadio, LORA_MAX_PAYLOAD};
use axum::{
    extract::{Path, Query, State},
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub radio: SharedRadio,
    pub auth_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub to: String,
    pub payload: String,
}

#[derive(Debug, Serialize)]
pub struct SendMessageResponse {
    pub id: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct MessageStatusResponse {
    pub id: String,
    pub status: String,
    pub attempts: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub radio: String,
}

#[derive(Debug, Deserialize)]
pub struct InboxQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    100
}

pub fn create_router(state: AppState) -> Router {
    let mut router = Router::new()
        .route("/v1/messages", post(send_message))
        .route("/v1/messages/:id", get(get_message_status))
        .route("/v1/inbox", get(get_inbox))
        .route("/health", get(health));
    
    if state.auth_token.is_some() {
        router = router.layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));
    }
    
    router.with_state(state)
}

async fn auth_middleware<B>(
    State(state): State<AppState>,
    req: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    if req.uri().path() == "/health" {
        return Ok(next.run(req).await);
    }
    
    let auth_token = state.auth_token.as_ref().unwrap();
    
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok());
    
    match auth_header {
        Some(header) if header == format!("Bearer {}", auth_token) => {
            Ok(next.run(req).await)
        }
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

async fn send_message(
    State(state): State<AppState>,
    Json(payload): Json<SendMessageRequest>,
) -> Result<Json<SendMessageResponse>, StatusCode> {
    if payload.payload.len() > LORA_MAX_PAYLOAD {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    let id = state
        .db
        .insert_message(&payload.to, &payload.payload)
        .await
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    Ok(Json(SendMessageResponse {
        id,
        status: "queued".to_string(),
    }))
}

async fn get_message_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<MessageStatusResponse>, StatusCode> {
    let msg = state
        .db
        .get_message(&id)
        .await
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;
    
    Ok(Json(MessageStatusResponse {
        id: msg.id,
        status: msg.status,
        attempts: msg.attempts,
        created_at: msg.created_at,
        updated_at: msg.updated_at,
    }))
}

async fn get_inbox(
    State(state): State<AppState>,
    Query(query): Query<InboxQuery>,
) -> Result<Json<Vec<crate::db::InboxMessage>>, StatusCode> {
    let limit = query.limit.min(1000).max(1);
    
    let messages = state
        .db
        .get_inbox_messages(limit)
        .await
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    Ok(Json(messages))
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let radio = state.radio.lock().await;
    
    let radio_status = if !radio.is_connected() {
        "missing"
    } else {
        match radio.backend_type() {
            RadioBackend::MOCK => "mock",
            RadioBackend::REAL_SERIAL => "up",
            _ => "unknown",
        }
    };
    
    Json(HealthResponse {
        status: "ok".to_string(),
        radio: radio_status.to_string(),
    })
}
