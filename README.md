# Messenger v2

Real-time messenger — **Flutter 3.47 (Riverpod, go_router, gRPC)** + **Rust 1.97 (Tonic gRPC, sqlx, scylla, aws-sdk-s3)** + **PostgreSQL 15** + **Cassandra 5.0.9** + **RustFS 1.0.0-rc.3 (S3)**. Group chats, 1:1 messaging, attachments, OTP phone auth (planned), Android-first.

> Docs: `PROJECT_STRUCTURE.md` (architecture), `STEPS.md` (phased build plan), `AGENTS.md` (commands).

---

## Architecture

| Service | Image | Port | Notes |
|---------|-------|------|-------|
| PostgreSQL | `postgres:15-alpine` | `5432` (`POSTGRES_PORT`) | `messenger` DB, `container_name: postgres-messenger` |
| Cassandra | `cassandra:5.0.9` | `9042` | keyspace `messenger`, `container_name: cassandra-messenger` |
| RustFS | `rustfs/rustfs:1.0.0-rc.3` | `9000` (S3), `9001` (Console) | bucket `messenger-attachments`, auto-created on server start (`server/src/storage.rs:25`) |
| Rust Server | `konsin1988/messenger:${PROJECT_VERSION}` (build `target: dev`) | `50051` (gRPC) | `Tonic` — `UserService`, `MessageService`, `ChatRoomService` (`server/proto/messenger.proto:9`) |

Network: `rt-messenger` (`external: true`) — all services share it. Server `depends_on: postgres, cassandra, rustfs` `healthy`.

**DB split:** Postgres for `user`, `user_profile`, `contact`, `conversation`, `attachment` etc. (`server/migrations/001_init.sql:6`); Cassandra for high-write `messages`, `message_delivery`, `unread_counters`, `conversation_preview` (`server/migrations/cassandra/001-007`).

---

## Prerequisites

- Docker + Compose v2
- For local client: Flutter 3.47
- For local server: Rust 1.97, `protobuf-compiler`, `cmake`, `clang`, `pkg-config`, `libssl-dev` (`server/Dockerfile:5` — required for `prost-build` + `aws-lc-sys`)
- External network: `docker network create rt-messenger` (required by `docker-compose.yml:114`)

---

## Quick Start (Dev)

```bash
# 1. Create shared network (once)
docker network create rt-messenger

# 2. Start infra + server (first build compiles 386 crates — 5-10 min)
docker compose up --build

# 3. Or detached
docker compose up -d
docker compose logs -f server

# 4. Check health
curl -f http://localhost:9000/health                # RustFS S3
curl -f http://localhost:9001/rustfs/console/health # RustFS console
grpcurl -plaintext localhost:50051 list             # gRPC services
```

Server runs at `0.0.0.0:50051`, RustFS at `localhost:9000/9001`, Postgres `5432`, Cassandra `9042`. Server creates `messenger-attachments` bucket on boot (`server/src/main.rs:312`).

**Stop / rebuild:**

```bash
docker compose down
docker compose up --build          # rebuild after Cargo.toml / Dockerfile changes
docker compose logs -f [service]   # logs
docker compose exec server sh      # shell
docker compose exec postgres psql -U ${POSTGRES_USER} -d ${POSTGRES_DB}
docker compose exec cassandra cqlsh
```

---

## Environment

`.env` (dev defaults, committed) and `.env.prod` (template, copy to `.env` for prod).

| Variable | Required | Default | Notes |
|----------|----------|---------|-------|
| `PROJECT_VERSION` | no | `0.0.1` | **Frozen** — `image: konsin1988/messenger:${PROJECT_VERSION}` (`docker-compose.yml:70`), do not change without command |
| `POSTGRES_DB/USER/PASSWORD/PORT` | via `.env` | `messenger` / `messenger_user` / `dev_password_change_me` / `5432` | |
| `CASSANDRA_CLUSTER_NAME/DC/PORT` | no | `MessengerCluster` / `dc1` / `9042` | |
| `GRPC_PORT` | no | `50051` | |
| `RUSTFS_ACCESS_KEY/SECRET_KEY` | no | `rustfsadmin` | |
| `RUSTFS_S3_PORT/CONSOLE_PORT` | no | `9000` / `9001` | |
| `RUSTFS_BUCKET` | no | `messenger-attachments` | |
| `RUSTFS_ENDPOINT` | no | `http://rustfs:9000` | internal (docker network) |
| `RUSTFS_PUBLIC_ENDPOINT` | no | `http://localhost:9000` | presigned URLs |
| `DATABASE_URL` | yes | `postgres://...@postgres:5432/messenger` | via `.env` |
| `CASSANDRA_URL` | yes | `cassandra://cassandra:9042` | via `.env` |
| `JWT_SECRET` | yes | `dev-jwt-secret-change-in-production` | override in prod |
| `RUST_LOG` | no | `debug` (dev) / `info` (prod) | |

Prod overrides: `SERVER_PORT` default `8080`, secrets must be replaced, DB/RustFS bound to `127.0.0.1`.

---

## Project Structure

```
messenger_v2/
├── .env / .env.prod
├── docker-compose.yml / docker-compose.prod.yml
├── STEPS.md / PROJECT_STRUCTURE.md / AGENTS.md
├── server/  # Rust 1.97, Tonic 0.12, sqlx 0.8, scylla 0.15 + chrono-04, tokio-stream, aws-sdk-s3
│   ├── Cargo.toml / Cargo.lock
│   ├── Dockerfile  # base + protobuf-compiler cmake clang
│   ├── build.rs    # tonic_build proto
│   ├── proto/messenger.proto
│   ├── migrations/ (001_init.sql + cassandra 001-007) 
│   └── src/{main.rs, config.rs, storage.rs, db/, models/, routes/}
└── client/  # Flutter 3.47, Riverpod, go_router, grpc/protobuf
    ├── pubspec.yaml
    └── lib/{main.dart, core/{api,router,theme}, features/{auth,chat,profile}}
```

See `PROJECT_STRUCTURE.md:59` for full server tree and `PROJECT_STRUCTURE.md:122` for client.

---

## Server

```bash
# Inside container or locally (from ./server)
cargo watch -w src -x run                # dev hot reload
cargo build                              # debug build (needs protoc)
cargo build --release                    # prod
cargo sqlx migrate run                   # run postgres migrations
cargo check                              # verify (currently 0 errors, 10 warnings)
```

**gRPC contract** `server/proto/messenger.proto:9`:
- `UserService: Register, Login, GetUser, ListUsers`
- `MessageService: SendMessage, StreamMessages (BroadcastStream), GetHistory`
- `ChatRoomService: CreateRoom, JoinRoom, LeaveRoom, ListRooms` (UNIMPLEMENTED)

**Key fixes (2026-08-24):** `UserServiceImpl` holds `PgPool` + `jwt_secret`, `r#"SELECT * FROM "user""#` quoting, `query`→`query_unpaged`, `BroadcastStream`, `storage.rs` bucket ensure, `scylla chrono-04`.

---

## Client

```bash
cd client
flutter pub get
flutter run --debug --web-port=3000
flutter build web   # prod → nginx
flutter test
```

Proto workflow (`PROJECT_STRUCTURE.md:166`):
1. Edit `server/proto/messenger.proto`
2. Copy to `client/lib/core/api/generated/`
3. Regenerate (`build_runner` / `proto_generator`)

`core/api/grpc_client.dart:21` uses insecure `localhost:50051` (prod needs `ChannelCredentials.secure`).

---

## Production

```bash
cp .env.prod .env   # fill secrets
docker compose -f docker-compose.prod.yml up -d --build
# or detached build
docker compose -f docker-compose.prod.yml up -d
```

- Server is `release` binary on `debian:bookworm-slim` (`server/Dockerfile:46`)
- DB/RustFS ports `127.0.0.1`-only, nginx serves `client/build/web`
- See `docker-compose.prod.yml:69` for resource limits (postgres 512M, cassandra 1G, server 256M)

---

## Migrations

```bash
# Postgres (sqlx)
docker compose exec server cargo sqlx migrate run
# or locally
cargo sqlx migrate run  # from ./server

# Cassandra (manual)
docker compose exec cassandra cqlsh -f /path/to/001_init_keyspace.cql
# or
cqlsh localhost 9042 -f server/migrations/cassandra/001_init_keyspace.cql
```

---

## Troubleshooting

- **Cassandra bootstrap ~30s** — server healthcheck waits (`docker-compose.yml:60` `start_period: 20s`, `interval: 15s`).
- **`Could not find protoc`** — install `protobuf-compiler` (`server/Dockerfile:6`), or `apt-get install protobuf-compiler`.
- **`SerializeValue` for `DateTime<Utc>`** — enable `scylla` `chrono-04` (`server/Cargo.toml:19`).
- **`query` private** — use `query_unpaged` (scylla 0.15 API).
- **`BroadcastStream` lagged** — stream filters lagged messages (`server/src/main.rs:185`); client should refetch history.
- **Build slow** — first `aws-sdk-s3` compile is 386 crates; use `docker compose build --no-cache server` only when `Cargo.toml` changes.
- **Network missing** — `docker network create rt-messenger` (`external: true`).

---

## Roadmap

See `STEPS.md:23` — Phase 0 foundation fixes (done) → Phone OTP auth → Simple groups (50 members) → Real-time messaging → RustFS attachments → Read receipts → Hardening → QA. E2E, international SMS deferred.

## Contributing

- Do not change `PROJECT_VERSION` or `server` image tag without maintainer command.
- Keep `PROJECT_STRUCTURE.md` in sync after infra changes.
- Run `cargo check` and `docker compose config` before pushing.
