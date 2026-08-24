# Messenger v2 — Project Structure

A real-time messenger application: **Flutter** (client, gRPC) + **Rust** (server, Tonic gRPC) + **PostgreSQL** (users/relational) + **Cassandra** (high-write messaging) + **RustFS** (S3-compatible attachments).

> Last updated: 2026-08-24 — synced with `docker-compose.yml`, `server/Cargo.toml`, `server/src/*`, `.env`, `STEPS.md`

---

## Root Layout

```
messenger_v2/
├── .env                     # Dev env config (committed, safe defaults) — includes PROJECT_VERSION, RUSTFS_*
├── .env.prod                # Prod template (copy to .env, fill secrets)
├── AGENTS.md                # Agent/developer guide
├── STEPS.md                 # Phased build plan (OTP auth, groups, RustFS attachments) — created 2026-08-24
├── PROJECT_STRUCTURE.md     # This file
├── docker-compose.yml       # Dev: hot reload, debug builds, external network rt-messenger
├── docker-compose.prod.yml  # Prod: release builds, nginx for web client, 127.0.0.1 DB/RustFS
├── nginx.conf               # Nginx config for prod Flutter web (referenced, not yet present)
├── client/                  # Flutter app (v3.47.0)
└── server/                  # Rust gRPC backend (v1.97.1)
```

---

## Infrastructure (Docker Compose)

### Dev (`docker-compose.yml`)

| Service | Image | Port | Notes |
|---------|-------|------|-------|
| postgres | postgres:15-alpine | 5432 | Relational store (users, contacts, rooms) — `container_name: postgres-messenger` |
| cassandra | cassandra:5.0.9 | 9042 | High-write message store, keyspace `messenger` — `container_name: cassandra-messenger` |
| rustfs | rustfs/rustfs:1.0.0-rc.3 | 9000 (S3), 9001 (Console) | S3-compatible attachment storage, bucket `messenger-attachments` — `container_name: rustfs-messenger` |
| server | konsin1988/messenger:${PROJECT_VERSION} (build from `./server` target `dev`) | 50051 (gRPC) | Tonic gRPC backend — **image tag frozen via `PROJECT_VERSION` (do not change without command)** |
| flutter (dev) | ghcr.io/cirruslabs/flutter:3.47.0 | 9100→3000 | Currently commented out in `docker-compose.yml:93` |

- Dev server uses `cargo watch -w src -x run`; volumes mount `server/src` and `server/proto`.
- Network: `rt-messenger` (`external: true`, name `rt-messenger`) shared by postgres/cassandra/rustfs/server.
- Volumes: `postgres_dev_data`, `cassandra_dev_data`, `rustfs_dev_data`, `flutter_pub_cache`.
- Healthchecks: `pg_isready`, `cqlsh describe cluster`, `curl http://localhost:9000/health && http://localhost:9001/rustfs/console/health`.
- Startup ordering: `postgres` & `cassandra` & `rustfs` must be `healthy` before `server` starts.

### Prod (`docker-compose.prod.yml`)

| Service | Image | Port | Notes |
|---------|-------|------|-------|
| postgres | postgres:15-alpine | 127.0.0.1:5432 | Same as dev but `restart: always`, mem limit 512M |
| cassandra | cassandra:5.0.9 | 127.0.0.1:9042 | Mem limit 1G |
| rustfs | rustfs/rustfs:1.0.0-rc.3 | 127.0.0.1:9000/9001 | Same env, mem not limited, `restart: always` |
| server | build `./server` target `prod` | 127.0.0.1:50051 | Release binary on `debian:bookworm-slim`, mem 256M |
| nginx | nginx:alpine | 80 | Serves `client/build/web` |

- Prod DB/RustFS ports bound to `127.0.0.1` only.

---

## Server (`server/`)

**Stack:** Tonic 0.12 + sqlx 0.8 (postgres) + scylla 0.15 + `chrono-04` + tokio-stream 0.1 / futures 0.3 + aws-sdk-s3 1 + aws-config 1 + jsonwebtoken 9 + bcrypt 0.16 + tracing.

```
server/
├── Cargo.toml              # tonic 0.12, prost 0.13, sqlx 0.8 + postgres/uuid/chrono, scylla 0.15 + chrono-04, tokio full, tokio-stream sync, futures 0.3, aws-config 1 + aws-sdk-s3 1 + aws-credential-types 1, chrono serde
├── Cargo.lock              # Generated (now present, 386 crates locked)
├── build.rs                # tonic_build compiles proto/messenger.proto
├── Dockerfile              # multi-stage: base (rust:1.97.1-bookworm + protobuf-compiler cmake clang pkg-config libssl-dev) → dependencies → dev (cargo watch, cargo build) | build (cargo build --release) → prod (debian:bookworm-slim)
├── proto/
│   └── messenger.proto     # gRPC contract (UserService, MessageService, ChatRoomService)
├── migrations/
│   ├── 001_init.sql        # Postgres schema (sqlx migrations)
│   └── cassandra/          # CQL scripts 001–007 (applied manually via cqlsh)
└── src/
    ├── main.rs             # gRPC service impls, AppState { PgPool, Arc<Session>, jwt_secret, broadcast::Sender<Message>, s3_client, rustfs_bucket }, entrypoint with RustFS bucket ensure
    ├── config.rs           # Config::from_env() — server_addr, database_url, cassandra_url, jwt_secret, rustfs_endpoint, rustfs_bucket, rustfs_access_key, rustfs_secret_key
    ├── storage.rs          # NEW: create_s3_client(endpoint, key, secret) + ensure_bucket_exists (HeadBucket → CreateBucket, force_path_style, us-east-1)
    ├── db/
    │   ├── mod.rs          # pub mod postgres; pub mod cassandra
    │   ├── postgres.rs     # create_pool() → PgPool
    │   └── cassandra.rs    # create_session() → scylla Session
    ├── models/
    │   ├── mod.rs          # User & Message structs (also request/response DTOs)
    │   ├── user.rs         # User, CreateUser
    │   └── message.rs      # Message, CreateMessage
    └── routes/             # Axum REST handlers (NOT wired into main.rs — dead/legacy code)
        ├── mod.rs          # axum AppState + health()
        ├── auth.rs         # register, login (REST versions of gRPC RPCs)
        ├── users.rs        # list, get
        └── messages.rs     # send, list
```

### gRPC Services (`main.rs`)

| Service | RPCs | Status |
|---------|------|--------|
| UserService | Register, Login, GetUser, ListUsers | Implemented — `UserServiceImpl` now holds `pg_pool` + `jwt_secret` (fixed `r#"SELECT * FROM "user""#` quoting) |
| MessageService | SendMessage (`query_unpaged`), StreamMessages (`BroadcastStream` + `Pin<Box<dyn Stream>>`), GetHistory (`query_unpaged` → `into_rows_result().rows::<(Uuid,Uuid,Uuid,String,DateTime<Utc>)>`) | Implemented — fixed private `Session::query` → `query_unpaged`, chrono `SerializeValue/DeserializeValue` via `scylla chrono-04` |
| ChatRoomService | CreateRoom, JoinRoom, LeaveRoom, ListRooms | All return `UNIMPLEMENTED` (TODO) |

- `AppState` holds `PgPool`, `Arc<Session>` (scylla), `jwt_secret`, `broadcast::Sender<Message>`, `s3_client` (`aws_sdk_s3::Client`), `rustfs_bucket`.
- On startup `main.rs:312` creates S3 client via `storage::create_s3_client(&config.rustfs_endpoint, ...)` and `ensure_bucket_exists` (fail-fast if bucket cannot be created).
- Messages streamed via `tokio_stream::wrappers::BroadcastStream` with `filter_map` (lagged messages dropped).
- `sender_id` still stubbed `Uuid::new_v4()` — TODO: extract from JWT.

### Postgres schema (`migrations/001_init.sql`)

`user`, `user_profile`, `device`, `contact`, `block`, `conversation`, `conversation_participant`, `conversation_setting`, `attachment`.

### Cassandra schema (`migrations/cassandra/`)

| File | Table | Purpose |
|------|-------|---------|
| 001_init_keyspace.cql | keyspace `messenger` | SimpleStrategy, RF=1 |
| 002_messages.cql | messages | PK((room_id), created_at, id), newest-first; history |
| 003_message_delivery.cql | message_delivery | Per-message per-recipient delivery state |
| 004_read_receipts.cql | read_receipts | Per-message read tracking |
| 005_messages_by_user.cql | messages_by_user | User's sent messages, newest-first |
| 006_unread_counters.cql | unread_counters | Counter table for unread badges |
| 007_conversation_preview.cql | conversation_preview | Denormalized last-message per room for chat list |

---

## Client (`client/`)

**Stack:** Flutter v3.47.0 + Riverpod (codegen) + go_router + grpc/protobuf + shared_preferences + flutter_secure_storage.

```
client/
├── pubspec.yaml            # flutter_riverpod, go_router, grpc, protobuf, riverpod_generator
├── proto/                  # proto_generator dev dep (directory referenced, not yet present)
└── lib/
    ├── main.dart           # ProviderScope + MessengerApp (MaterialApp.router, dark theme)
    ├── core/
    │   ├── api/
    │   │   ├── grpc_client.dart   # GrpcClient provider (insecure channel, localhost:50051)
    │   │   └── generated/         # .pb.dart/.pbgrpc.dart from server/proto (not yet generated)
    │   ├── router/
    │   │   └── app_router.dart    # GoRouter + ShellRoute w/ bottom nav (Chats, Profile)
    │   └── theme/
    │       └── app_theme.dart     # AppTheme.dark (M3, indigo #6366F1, dark surfaces)
    └── features/
        ├── auth/
        │   └── screens/
        │       ├── login_screen.dart     # Form UI only — gRPC call is TODO
        │       └── register_screen.dart  # Form UI only — gRPC call is TODO
        ├── chat/
        │   └── screens/
        │       ├── chat_list_screen.dart # Empty list — TODO: load rooms via gRPC
        │       └── chat_room_screen.dart # Local-only messages — TODO: send/stream via gRPC
        └── profile/
            └── screens/
                └── profile_screen.dart   # Static placeholders, logout TODO
```

### Routes

| Path | Screen |
|------|--------|
| `/login` | LoginScreen (initial) |
| `/register` | RegisterScreen |
| `/chats` | ChatListScreen (bottom nav tab 0) |
| `/chats/:roomId` | ChatRoomScreen |
| `/profile` | ProfileScreen (bottom nav tab 1) |

### Workflow for proto changes

1. Edit `server/proto/messenger.proto`.
2. Copy it to `client/lib/core/api/generated/`.
3. Regenerate Dart stubs (proto_generator / build_runner).

---

## Env vars (`server/src/config.rs`)

| Variable | Required | Default | Notes |
|----------|----------|---------|-------|
| PROJECT_VERSION | no | `0.0.1` | **Frozen** — used as `image: konsin1988/messenger:${PROJECT_VERSION}` in `docker-compose.yml:70` (do not change without command) |
| SERVER_HOST | no | `0.0.0.0` | |
| SERVER_PORT | no | `8080` | `server_addr = SERVER_HOST:SERVER_PORT` |
| DATABASE_URL | **yes** | — | `postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@postgres:5432/${POSTGRES_DB}` |
| CASSANDRA_URL | **yes** | — | `cassandra://cassandra:9042` |
| JWT_SECRET | **yes** | — | |
| RUSTFS_ENDPOINT | no | `http://rustfs:9000` | S3 endpoint for server (docker network) |
| RUSTFS_PUBLIC_ENDPOINT | no | `http://localhost:9000` | Public presigned URL host |
| RUSTFS_BUCKET | no | `messenger-attachments` | Auto-created on server start via `storage.rs` |
| RUSTFS_ACCESS_KEY | no | `rustfsadmin` | |
| RUSTFS_SECRET_KEY | no | `rustfsadmin` | |
| RUSTFS_S3_PORT | no | `9000` | |
| RUSTFS_CONSOLE_PORT | no | `9001` | |
| POSTGRES_* | via `.env` | — | `POSTGRES_DB/USER/PASSWORD/PORT` |
| CASSANDRA_* | via `.env` | — | `CASSANDRA_CLUSTER_NAME/DC/PORT/KEYSPACE` |
| RUST_LOG | no | `debug` (dev) / `info` (prod) | |

---

## Build & Dockerfile

- **Base** `rust:1.97.1-bookworm` now installs `protobuf-compiler cmake clang pkg-config libssl-dev` for `prost-build` + `aws-lc-sys`.
- **Dev** `cargo build` (not `cargo watch` in image) then `cargo watch -w src -x run`.
- **Prod** `cargo build --release` → `debian:bookworm-slim` with `ca-certificates`.

---

## Known TODOs / gaps

- ChatRoomService RPCs unimplemented (return `UNIMPLEMENTED`).
- `sender_id` hardcoded as random UUID (no JWT auth middleware).
- Client screens are UI-only; no gRPC calls wired yet.
- `client/lib/core/api/generated/` stubs not yet generated.
- `routes/` (Axum) duplicated logic of gRPC services and is not wired into `main.rs`.
- `nginx.conf` referenced but not yet present (prod).
- E2E encryption deferred, international SMS not supported, groups max 50 simplest (see `STEPS.md`).

## Recent Changes (2026-08-24)

- Added `rustfs` service `1.0.0-rc.3` to both compose files, network `rt-messenger`, volumes `rustfs_*_data`, server `depends_on: rustfs: healthy`.
- Added `PROJECT_VERSION=0.0.1` + `image: konsin1988/messenger:${PROJECT_VERSION}` (frozen).
- Added `server/src/storage.rs` + S3 bucket creation, `server/src/config.rs` RustFS fields, `server/Cargo.toml` `chrono-04` + `tokio-stream` + `aws-sdk-s3`.
- Fixed `server/src/main.rs` SQL quoting, `query`→`query_unpaged`, `BroadcastStream`, `UserServiceImpl` fields.
- Added `STEPS.md` roadmap.
