# AGENTS.md

> Skeleton file — fill in as the project takes shape.

## Project

- client - flutter v3.47.0
- server - rust v1.97.1
- cassandra v5.0.9
- postgres v15

## Commands

| Task | Command |
|------|---------|
| Dev (all) | `docker compose up` |
| Dev (detached) | `docker compose up -d` |
| Stop dev | `docker compose down` |
| Rebuild dev | `docker compose up --build` |
| Logs | `docker compose logs -f [service]` |
| Production | `docker compose -f docker-compose.prod.yml up -d` |
| Production rebuild | `docker compose -f docker-compose.prod.yml up -d --build` |
| Shell into server | `docker compose exec server sh` |
| Postgres CLI | `docker compose exec postgres psql -U ${POSTGRES_USER} -d ${POSTGRES_DB}` |
| Cassandra CLI | `docker compose exec cassandra cqlsh` |

## Architecture

| Service | Port | Notes |
|---------|------|-------|
| PostgreSQL | 5432 | Main relational store |
| Cassandra | 9042 | High-write messaging store |
| Rust Server | 50051 | gRPC backend (tonic) |
| Flutter Dev | 3000 | Web dev server with hot reload |

### Server (`/server`)

- **Framework:** Tonic (gRPC) + Axum
- **DB split:** PostgreSQL for users/auth, Cassandra for messages (high-write)
- **Auth:** JWT via `jsonwebtoken` + bcrypt password hashing
- **Entry:** `src/main.rs` → gRPC services → db modules
- **Proto:** `server/proto/messenger.proto` (UserService, MessageService, ChatRoomService)
- **Migrations:** `server/migrations/` (sqlx)
- **Dockerfile:** Multi-stage (`dev` target = hot reload via cargo-watch, `prod` target = release binary)

Key server commands (inside container or locally):
| Task | Command |
|------|---------|
| Dev run | `cargo watch -w src -x run` |
| Build release | `cargo build --release` |
| Run migrations | `cargo sqlx migrate run` |
| Add dependency | `cargo add <pkg>` |

### Client (`/client`)

- **Framework:** Flutter v3.47.0 + Riverpod
- **gRPC:** `grpc` + `protobuf` packages
- **State:** Riverpod with code generation
- **Navigation:** go_router
- **Proto files:** Copy from `server/proto/` to `client/lib/core/api/generated/`

### Project structure

- `docker-compose.yml` — development (hot reload, debug builds, exposed ports)
- `docker-compose.prod.yml` — production (release builds, nginx for Flutter web, localhost-only DB ports)
- `.env` — dev defaults (committed)
- `.env.prod` — prod template (copy to `.env` and fill secrets before deploy)

## Conventions

- `.env` has safe dev defaults; never put real secrets there
- `.env.prod` is the template for production; deploy by copying to `.env`
- Cassandra keyspace is `messenger`

## Gotchas

- Cassandra takes ~30s to bootstrap on first start; server healthcheck waits for it
- Flutter container uses `flutter run --debug` for dev; use `flutter build web` + nginx for prod
- Production DB ports are bound to `127.0.0.1` only (not exposed to host network)
