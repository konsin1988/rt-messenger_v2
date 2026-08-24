use axum::{extract::State, Json};
use bcrypt::{hash, verify, DEFAULT_COST};
use jsonwebtoken::{encode, Header};
use uuid::Uuid;

use super::AppState;
use crate::models::{AuthResponse, LoginRequest, RegisterRequest, User};

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, String)> {
    let password_hash = hash(&req.password, DEFAULT_COST)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let user = sqlx::query_as::<_, User>(
        "INSERT INTO "user" (id, username, email, password_hash, created_at) VALUES ($1, $2, $3, $4, NOW()) RETURNING *"
    )
    .bind(Uuid::new_v4())
    .bind(&req.username)
    .bind(&req.email)
    .bind(&password_hash)
    .fetch_one(&state.pg_pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let token = encode(
        &Header::default(),
        &serde_json::json!({"sub": user.id.to_string()}),
        &jsonwebtoken::EncodingKey::from_secret(state.jwt_secret.as_bytes()),
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(AuthResponse { token, user }))
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, String)> {
    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM "user" WHERE email = $1"
    )
    .bind(&req.email)
    .fetch_optional(&state.pg_pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or_else(|| (StatusCode::UNAUTHORIZED, "Invalid credentials".into()))?;

    let valid = verify(&req.password, &user.password_hash)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if !valid {
        return Err((StatusCode::UNAUTHORIZED, "Invalid credentials".into()));
    }

    let token = encode(
        &Header::default(),
        &serde_json::json!({"sub": user.id.to_string()}),
        &jsonwebtoken::EncodingKey::from_secret(state.jwt_secret.as_bytes()),
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(AuthResponse { token, user }))
}

use axum::http::StatusCode;
