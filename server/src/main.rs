mod auth;
mod auth_service;
mod config;
mod db;
mod models;
mod storage;

pub mod messenger {
    tonic::include_proto!("messenger");
    pub const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("messenger_descriptor");
}

use messenger::auth_service_server::AuthServiceServer;
use messenger::chat_room_service_server::{ChatRoomService, ChatRoomServiceServer};
use messenger::message_service_server::{MessageService, MessageServiceServer};
use messenger::user_service_server::{UserService, UserServiceServer};
use messenger::*;

use scylla::Session;
use sqlx::PgPool;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::broadcast;
use futures::StreamExt;
use tokio_stream::wrappers::BroadcastStream;
use tonic::{Request, Response, Status};

#[derive(Clone)]
pub struct AppState {
    pub pg_pool: PgPool,
    pub scylla_session: Arc<Session>,
    pub redis_client: ::redis::Client,
    pub jwt_secret: String,
    pub tx: broadcast::Sender<Message>,
    pub s3_client: aws_sdk_s3::Client,
    pub rustfs_bucket: String,
}

fn jwt_for_user(user_id: uuid::Uuid, jwt_secret: &str, exp_secs: i64) -> Result<String, Status> {
    let now = chrono::Utc::now().timestamp() as usize;
    let exp = (chrono::Utc::now() + chrono::Duration::seconds(exp_secs)).timestamp() as usize;
    let claims = auth::Claims {
        sub: user_id.to_string(),
        exp,
        iat: now,
    };
    jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
    .map_err(|e| Status::internal(format!("JWT encode failed: {}", e)))
}

#[derive(Clone)]
pub struct UserServiceImpl {
    pub pg_pool: PgPool,
    pub jwt_secret: String,
    pub jwt_exp_secs: i64,
}

#[tonic::async_trait]
impl UserService for UserServiceImpl {
    async fn register(
        &self,
        request: Request<RegisterRequest>,
    ) -> Result<Response<AuthResponse>, Status> {
        let req = request.into_inner();
        let password_hash = bcrypt::hash(&req.password, bcrypt::DEFAULT_COST)
            .map_err(|e| Status::internal(e.to_string()))?;

        let user = sqlx::query_as::<_, models::User>(
            r#"INSERT INTO "user" (id, username, email, password_hash, created_at) VALUES ($1, $2, $3, $4, NOW()) RETURNING *"#
        )
        .bind(uuid::Uuid::new_v4())
        .bind(&req.username)
        .bind(&req.email)
        .bind(&password_hash)
        .fetch_one(&self.pg_pool)
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

        let token = jwt_for_user(user.id, &self.jwt_secret, self.jwt_exp_secs)?;

        Ok(Response::new(AuthResponse {
            token,
            user: Some(user.into()),
        }))
    }

    async fn login(
        &self,
        request: Request<LoginRequest>,
    ) -> Result<Response<AuthResponse>, Status> {
        let req = request.into_inner();

        let user = sqlx::query_as::<_, models::User>(
            r#"SELECT * FROM "user" WHERE email = $1"#
        )
        .bind(&req.email)
        .fetch_optional(&self.pg_pool)
        .await
        .map_err(|e| Status::internal(e.to_string()))?
        .ok_or_else(|| Status::unauthenticated("Invalid credentials"))?;

        let hash = user
            .password_hash
            .as_deref()
            .ok_or_else(|| Status::unauthenticated("Invalid credentials: no password set, use OTP"))?;
        let valid = bcrypt::verify(&req.password, hash)
            .map_err(|e| Status::internal(e.to_string()))?;

        if !valid {
            return Err(Status::unauthenticated("Invalid credentials"));
        }

        let token = jwt_for_user(user.id, &self.jwt_secret, self.jwt_exp_secs)?;

        Ok(Response::new(AuthResponse {
            token,
            user: Some(user.into()),
        }))
    }

    async fn get_user(
        &self,
        request: Request<GetUserRequest>,
    ) -> Result<Response<User>, Status> {
        let req = request.into_inner();
        let id = uuid::Uuid::parse_str(&req.user_id)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let user = sqlx::query_as::<_, models::User>(r#"SELECT * FROM "user" WHERE id = $1"#)
            .bind(id)
            .fetch_optional(&self.pg_pool)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("User not found"))?;

        Ok(Response::new(user.into()))
    }

    async fn list_users(
        &self,
        _request: Request<ListUsersRequest>,
    ) -> Result<Response<ListUsersResponse>, Status> {
        let users = sqlx::query_as::<_, models::User>(r#"SELECT * FROM "user""#)
            .fetch_all(&self.pg_pool)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(ListUsersResponse {
            users: users.into_iter().map(|u| u.into()).collect(),
        }))
    }
}

#[derive(Clone)]
pub struct MessageServiceImpl {
    state: AppState,
}

#[tonic::async_trait]
impl MessageService for MessageServiceImpl {
    async fn send_message(
        &self,
        request: Request<SendMessageRequest>,
    ) -> Result<Response<Message>, Status> {
        let sender_id = request
            .extensions()
            .get::<uuid::Uuid>()
            .copied()
            .ok_or_else(|| Status::unauthenticated("Missing user ID"))?;

        let req = request.into_inner();
        let msg = models::Message {
            id: uuid::Uuid::new_v4(),
            room_id: uuid::Uuid::parse_str(&req.room_id)
                .map_err(|e| Status::invalid_argument(e.to_string()))?,
            sender_id,
            content: req.content,
            created_at: chrono::Utc::now(),
        };

        self.state.scylla_session
            .query_unpaged(
                "INSERT INTO messenger.messages (id, room_id, sender_id, content, created_at) VALUES (?, ?, ?, ?, ?)",
                (&msg.id, &msg.room_id, &msg.sender_id, &msg.content, &msg.created_at),
            )
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let _ = self.state.tx.send(msg.clone().into());

        Ok(Response::new(msg.into()))
    }

    type StreamMessagesStream =
        Pin<Box<dyn futures::Stream<Item = Result<Message, Status>> + Send>>;

    async fn stream_messages(
        &self,
        request: Request<StreamMessagesRequest>,
    ) -> Result<Response<Self::StreamMessagesStream>, Status> {
        let _req = request.into_inner();
        let rx = self.state.tx.subscribe();
        let stream = BroadcastStream::new(rx).filter_map(|res| async move {
            match res {
                Ok(msg) => Some(Ok(msg)),
                Err(_) => None,
            }
        });
        Ok(Response::new(Box::pin(stream)))
    }

    async fn get_history(
        &self,
        request: Request<GetHistoryRequest>,
    ) -> Result<Response<GetHistoryResponse>, Status> {
        let req = request.into_inner();
        let room_id = uuid::Uuid::parse_str(&req.room_id)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let result = self.state.scylla_session
            .query_unpaged(
                "SELECT id, room_id, sender_id, content, created_at FROM messenger.messages WHERE room_id = ?",
                (room_id,),
            )
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let rows_result = result
            .into_rows_result()
            .map_err(|e| Status::internal(e.to_string()))?;

        let mut messages: Vec<Message> = Vec::new();
        for row in rows_result
            .rows::<(uuid::Uuid, uuid::Uuid, uuid::Uuid, String, chrono::DateTime<chrono::Utc>)>()
            .map_err(|e| Status::internal(e.to_string()))?
        {
            let (id, room_id_val, sender_id, content, created_at) =
                row.map_err(|e| Status::internal(e.to_string()))?;
            messages.push(Message {
                id: id.to_string(),
                room_id: room_id_val.to_string(),
                sender_id: sender_id.to_string(),
                content,
                timestamp: created_at.timestamp(),
            });
            if messages.len() >= req.limit as usize {
                break;
            }
        }

        Ok(Response::new(GetHistoryResponse { messages }))
    }
}

#[derive(Default)]
pub struct ChatRoomServiceImpl;

#[tonic::async_trait]
impl ChatRoomService for ChatRoomServiceImpl {
    async fn create_room(
        &self,
        _request: Request<CreateRoomRequest>,
    ) -> Result<Response<Room>, Status> {
        Err(Status::unimplemented("TODO"))
    }

    async fn join_room(
        &self,
        _request: Request<JoinRoomRequest>,
    ) -> Result<Response<JoinRoomResponse>, Status> {
        Err(Status::unimplemented("TODO"))
    }

    async fn leave_room(
        &self,
        _request: Request<LeaveRoomRequest>,
    ) -> Result<Response<LeaveRoomResponse>, Status> {
        Err(Status::unimplemented("TODO"))
    }

    async fn list_rooms(
        &self,
        _request: Request<ListRoomsRequest>,
    ) -> Result<Response<ListRoomsResponse>, Status> {
        Err(Status::unimplemented("TODO"))
    }
}

impl From<models::User> for User {
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

impl From<models::Message> for Message {
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "messenger_server=debug".into()),
        )
        .init();

    let config = config::Config::from_env()?;

    let pg_pool = db::postgres::create_pool(&config.database_url).await?;
    sqlx::migrate!("./migrations").run(&pg_pool).await?;

    let scylla_session = Arc::new(db::cassandra::create_session(&config.cassandra_url).await?);

    // ── Redis client + ping (fail-fast, caching) ─────────────────
    let redis_client = db::redis::create_client(&config.redis_url)?;
    if let Err(e) = db::redis::ping(&redis_client).await {
        tracing::warn!("Redis ping failed (will retry on use): {e:?}");
        // Fail-fast alternative: return Err(e);
    }

    // ── RustFS S3 client + ensure bucket exists ─────────────────
    let s3_client = storage::create_s3_client(
        &config.rustfs_endpoint,
        &config.rustfs_access_key,
        &config.rustfs_secret_key,
    );
    // Fail fast if bucket can't be created — server can't handle attachments without it
    storage::ensure_bucket_exists(&s3_client, &config.rustfs_bucket)
        .await
        .map_err(|e| {
            tracing::error!("RustFS bucket setup failed: {e:?}");
            e
        })?;

    let (tx, _) = broadcast::channel::<Message>(100);

    let state = AppState {
        pg_pool,
        scylla_session,
        redis_client,
        jwt_secret: config.jwt_secret.clone(),
        tx,
        s3_client,
        rustfs_bucket: config.rustfs_bucket.clone(),
    };

    let addr = config.server_addr.parse()?;
    let user_svc = UserServiceImpl {
        pg_pool: state.pg_pool.clone(),
        jwt_secret: state.jwt_secret.clone(),
        jwt_exp_secs: config.jwt_exp_secs,
    };
    let msg_svc = MessageServiceImpl { state: state.clone() };
    let room_svc = ChatRoomServiceImpl;
    let auth_svc = auth_service::AuthServiceImpl::new(
        state.pg_pool.clone(),
        state.jwt_secret.clone(),
        config.jwt_exp_secs,
        config.sms_mock,
        config.otp_ttl_secs,
        config.otp_country_prefix.clone(),
    );

    // MessageService protected; AuthService is public
    let message_auth_interceptor = auth::AuthInterceptor::new(state.jwt_secret.clone());

    tracing::info!("gRPC server listening on {}", addr);
    tracing::info!(
        "Auth OTP: sms_mock={}, otp_ttl={}s, jwt_exp={}s",
        config.sms_mock,
        config.otp_ttl_secs,
        config.jwt_exp_secs
    );

    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(messenger::FILE_DESCRIPTOR_SET)
        .build_v1()?;

    tonic::transport::Server::builder()
        .add_service(reflection)
        .add_service(UserServiceServer::new(user_svc))
        .add_service(AuthServiceServer::new(auth_svc))
        .add_service(MessageServiceServer::with_interceptor(
            msg_svc,
            message_auth_interceptor,
        ))
        .add_service(ChatRoomServiceServer::new(room_svc))
        .serve(addr)
        .await?;

    Ok(())
}
