pub mod auth;
pub mod messages;
pub mod users;

use axum::{extract::State, http::StatusCode, Json};
use scylla::Session;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub pg_pool: PgPool,
    pub scylla_session: Session,
    pub jwt_secret: String,
}

pub async fn health() -> StatusCode {
    StatusCode::OK
}

use axum::response::IntoResponse;
