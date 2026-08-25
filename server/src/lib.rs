pub mod auth;
pub mod auth_service;
pub mod config;
pub mod db;
pub mod models;
pub mod storage;

pub mod messenger {
    tonic::include_proto!("messenger");
    pub const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("messenger_descriptor");
}

impl From<models::User> for messenger::User {
    fn from(u: models::User) -> Self {
        Self {
            id: u.id.to_string(),
            username: u.username,
            email: u.email.unwrap_or_default(),
            created_at: u.created_at.to_rfc3339(),
            phone: u.phone.unwrap_or_default(),
        }
    }
}

impl From<models::Message> for messenger::Message {
    fn from(m: models::Message) -> Self {
        Self {
            id: m.id.to_string(),
            room_id: m.room_id.to_string(),
            sender_id: m.sender_id.to_string(),
            content: m.content,
            timestamp: m.created_at.timestamp(),
        }
    }
}
