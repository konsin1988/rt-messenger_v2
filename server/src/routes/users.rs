use axum::{extract::{Path, State}, Json};
use uuid::Uuid;

use super::AppState;
use crate::models::User;

pub async fn list(
    State(state): State<AppState>,
) -> Result<Json<Vec<User>>, (StatusCode, String)> {
    let users = sqlx::query_as::<_, User>("SELECT * FROM "user"")
        .fetch_all(&state.pg_pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(users))
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<User>, (StatusCode, String)> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM "user" WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pg_pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "User not found".into()))?;

    Ok(Json(user))
}

use axum::http::StatusCode;
