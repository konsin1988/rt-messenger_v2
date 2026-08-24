use axum::{extract::{Path, State}, Json};
use uuid::Uuid;

use super::AppState;
use crate::models::{Message, SendMessageRequest};

pub async fn send(
    State(state): State<AppState>,
    Json(req): Json<SendMessageRequest>,
) -> Result<Json<Message>, (StatusCode, String)> {
    let message = Message {
        id: Uuid::new_v4(),
        room_id: req.room_id,
        sender_id: Uuid::new_v4(), // TODO: extract from JWT
        content: req.content,
        created_at: chrono::Utc::now(),
    };

    state.scylla_session
        .query(
            "INSERT INTO messenger.messages (id, room_id, sender_id, content, created_at) VALUES (?, ?, ?, ?, ?)",
            (&message.id, &message.room_id, &message.sender_id, &message.content, &message.created_at),
        )
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(message))
}

pub async fn list(
    State(state): State<AppState>,
    Path(room_id): Path<Uuid>,
) -> Result<Json<Vec<Message>>, (StatusCode, String)> {
    let rows = state.scylla_session
        .query(
            "SELECT id, room_id, sender_id, content, created_at FROM messenger.messages WHERE room_id = ?",
            (room_id,),
        )
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let messages: Vec<Message> = rows
        .rows
        .iter()
        .filter_map(|row| {
            Some(Message {
                id: row.by_name("id").ok()?,
                room_id: row.by_name("room_id").ok()?,
                sender_id: row.by_name("sender_id").ok()?,
                content: row.by_name("content").ok()?,
                created_at: row.by_name("created_at").ok()?,
            })
        })
        .collect();

    Ok(Json(messages))
}

use axum::http::StatusCode;
