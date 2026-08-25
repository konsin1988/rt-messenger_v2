# Messenger v2 — Build Steps

> Goal: production-grade messenger (Flutter Android + Rust Tonic gRPC + Postgres + Cassandra + RustFS) with phone-number OTP auth, 1:1 and simple group chats, real-time messaging, attachments. Scope locked per 2026-08-24 clarifications: no international numbers, simplest groups, RustFS local, no E2E in v1, Android only, minimal feature set.

## Decisions (Clarified)
| # | Question | Decision |
|---|----------|----------|
| 1 | International numbers | No — local numbers only, E.164 validation limited to one country code (configurable). Cost irrelevant in dev — use mock SMS provider with log-to-console; Twilio/SNS behind feature flag for later. |
| 2 | Group limit | Simplest: max 50 members, roles `owner`/`member` only (no `admin` hierarchy yet), no invite links. Uses existing `conversation.is_group` + `conversation_participant.role` `server/migrations/001_init.sql:77`. |
| 3 | Attachment storage | RustFS (open-source S3-compatible) run locally via Docker Compose, S3 API via `aws-sdk-s3` / `rust-s3`. Presigned URLs. |
| 4 | E2E encryption | Difficult (key exchange, double ratchet, recovery). Deferred to Phase 10. Messages stored plaintext in Cassandra for v1. |
| 5 | Platform | Android only for v1. FCM via `device` table `001_init.sql:33` (platform=`android`). iOS/web deferred. |
| 6 | Feature scope | Simplest viable: OTP login, profile, 1:1 + group create/join/leave/list, send/receive/history/stream, RustFS attachments, read receipts + unread counters only. No reactions/reply/edit/forward/search in v1. |

## Prerequisites
- `docker compose up` brings `postgres:15`, `cassandra:5.0.9`, `server` (Tonic 50051), `flutter` dev, `rustfs` (new)
- Proto workflow: edit `server/proto/messenger.proto` → `server/build.rs` → `cargo build` → copy proto to `client/proto/` or `client/lib/core/api/generated/` → `dart run build_runner` / `proto_generator`
- Env: add `SMS_MOCK=true`, `RUSTFS_ENDPOINT`, `RUSTFS_BUCKET`, `RUSTFS_ACCESS_KEY`, `RUSTFS_SECRET_KEY` to `.env` `server/src/config.rs:11`
- Migrations: `sqlx migrate run` (Postgres) + `cqlsh -f` for `server/migrations/cassandra/*.cql`

---

## Phase 0 — Foundation Fix (1-2 days)
**Why first:** current code has gaps blocking any feature.
- [ ] 0.1 Fix `server/src/main.rs:29` — `UserServiceImpl` holds no state (`pg_pool`, `jwt_secret`) → compile error; make it `struct UserServiceImpl { pg_pool: PgPool, jwt_secret: String }` matching `MessageServiceImpl`
- [ ] 0.2 Fix `server/src/main.rs:149` stub `sender_id = Uuid::new_v4()` — add gRPC JWT interceptor, extract `sub` from metadata `authorization: Bearer <token>` (use `tonic::Status::unauthenticated`)
- [ ] 0.3 Add missing deps `server/Cargo.toml:7` — `tokio-stream` (for `StreamMessages`), `regex` (phone validation), `aws-sdk-s3` or `s3` crate for RustFS
- [ ] 0.4 Add RustFS service to `docker-compose.yml:1` and `docker-compose.prod.yml` — `image: rustfs/rustfs:latest`, port 9000:9000, bucket `messenger-attachments`, healthcheck, volume `rustfs_data`
- [ ] 0.5 Generate Dart stubs — populate `client/lib/core/api/generated/` (currently empty `PROJECT_STRUCTURE.md:113`)
- [ ] Verify: `docker compose up --build` boots, `grpcurl -plaintext localhost:50051 list` shows 3 services, Flutter `grpc_client.dart:21` connects

## Phase 1 — Phone OTP Auth (3-5 days)
Replaces `email+password` `server/proto/messenger.proto:9` with mobile flow.
- [ ] 1.1 Postgres migration `server/migrations/002_phone_auth.sql`:
  ```sql
  ALTER TABLE "user" ADD COLUMN phone VARCHAR(20) UNIQUE;
  ALTER TABLE "user" ALTER COLUMN email DROP NOT NULL; -- keep optional for fallback
  ALTER TABLE "user" ALTER COLUMN password_hash DROP NOT NULL; -- OTP users have no password
  CREATE TABLE phone_verification (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    phone VARCHAR(20) NOT NULL,
    otp_hash VARCHAR(255) NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    attempts INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
  );
  CREATE INDEX idx_pv_phone ON phone_verification(phone);
  ```
- [ ] 1.2 Proto `server/proto/messenger.proto:54` — add:
  ```proto
  service AuthService {
    rpc RequestOTP(RequestOTPRequest) returns (RequestOTPResponse); // mock SMS
    rpc VerifyOTP(VerifyOTPRequest) returns (AuthResponse);
    rpc RefreshToken(RefreshTokenRequest) returns (AuthResponse);
  }
  message RequestOTPRequest { string phone = 1; }
  message RequestOTPResponse { bool success = 1; string debug_otp = 2; } // debug_otp only when SMS_MOCK=true
  message VerifyOTPRequest { string phone = 1; string code = 2; string username = 3; } // username optional on first verify -> register
  ```
  Extend `User { string phone = 5; }`, deprecate `RegisterRequest/LoginRequest` but keep for compat.
- [ ] 1.3 Server `server/src/routes/auth.rs` or `server/src/services/auth.rs`:
  - Validate phone: `^\+?[0-9]{10,15}$` limited to configured country prefix, rate limit 3 req/min per phone+IP (in-memory `DashMap` or Postgres)
  - `RequestOTP`: generate 6-digit `rand`, bcrypt hash, insert `phone_verification` with `expires_at = now+5min`, if `SMS_MOCK=true` log OTP and return `debug_otp`, else call Twilio stub
  - `VerifyOTP`: fetch latest non-expired row, `bcrypt::verify`, check `attempts<5`, delete row on success, upsert `user` (find by `phone` else `INSERT`), create `user_profile` if new, issue JWT `server/src/main.rs:52` with `exp=7d`
  - Config `server/src/config.rs:11` add `sms_mock: bool`, `otp_ttl_secs`
- [ ] 1.4 Client `client/lib/features/auth/screens/`:
  - Replace `login_screen.dart`/`register_screen.dart` with `phone_login_screen.dart` (phone input + country mask) and `otp_screen.dart` (6-digit pin, resend timer 60s)
  - Riverpod `auth_provider.dart` — store JWT in `flutter_secure_storage`, attach to `grpc_client.dart:24` via `CallOptions(metadata: {'authorization': 'Bearer $token'})`, `go_router` guard: if token null → `/login` else `/chats`
- [ ] 1.5 Verify: `cargo test` OTP hash/verify/expiry, `grpcurl` RequestOTP → VerifyOTP → JWT, Flutter Android emulator full flow, check `psql` `phone_verification` cleanup

### Phase 1 Tests — Autotest (Option A)

> Autotest covers `cargo test` (unit + integration with Postgres) + `grpcurl` E2E + `psql` cleanup. Run via `cargo test` and `scripts/test_phase1.sh`. See `server/src/auth_service.rs:330` `#[cfg(test)]` and `server/tests/auth_phase1.rs` + `server/tests/common.rs`.

**How to run:**
```bash
# unit only (no DB)
cargo test --manifest-path server/Cargo.toml --lib -- --nocapture

# unit + integration (needs postgres, uses .env DATABASE_URL)
docker compose up -d postgres redis
cargo sqlx migrate run   # applies server/migrations/001_init.sql
cargo test --manifest-path server/Cargo.toml -- --nocapture --test-threads=1

# autotest script (spins postgres+redis, migrates, runs tests, grpcurl E2E if server running)
bash scripts/test_phase1.sh

# manual E2E
grpcurl -plaintext -d '{"phone":"+79990001122"}' localhost:50051 messenger.AuthService/RequestOTP
grpcurl -plaintext -d '{"phone":"+79990001122","code":"<debug_otp>","username":"alice"}' localhost:50051 messenger.AuthService/VerifyOTP
grpcurl -plaintext -d '{"token":"<jwt>"}' localhost:50051 messenger.AuthService/RefreshToken
psql "postgres://messenger_user:1234@localhost:5432/messenger" -c "SELECT phone,attempts,expires_at FROM phone_verification ORDER BY created_at DESC LIMIT 5"
```

**Unit tests (`server/src/auth_service.rs:47` `validate_phone`, `auth_service.rs:68` `check_rate_limit`, `auth_service.rs:82` `issue_jwt`, `server/src/auth.rs:55` `verify_token`):**

| # | Test | File:line | Input → Expected |
|---|------|-----------|------------------|
| T1 | `test_validate_phone_ok` | `auth_service.rs:47` | `+79990001122`, `79990001122`, `+12345678901` → `Ok` |
| T2 | `test_validate_phone_invalid` | `auth_service.rs:47` | `""`, `123`, `+1234567890123456` (16), `abc`, `+7 999 000` → `InvalidArgument` |
| T3 | `test_validate_phone_country_prefix` | `auth_service.rs:54` | `OTP_COUNTRY_PREFIX=+7`, `+7999...` → Ok, `+1212...` → `InvalidArgument` contains "country prefix"; empty prefix allows all |
| T4 | `test_rate_limit_allow_3` | `auth_service.rs:68` | 3× `check_rate_limit(phone)` → Ok |
| T5 | `test_rate_limit_block_4th` | `auth_service.rs:68` | 4th within 60s → `ResourceExhausted` "3 OTP requests per minute" |
| T6 | `test_rate_limit_ip_isolation` | `auth_service.rs:68` | key `phone:ip1` vs `phone:ip2` isolated (DashMap) |
| T7 | `test_bcrypt_otp_roundtrip` | `auth_service.rs:127` | `hash("123456")` then `verify("123456") == true`, `verify("654321")==false` |
| T8 | `test_issue_jwt_claims` | `auth_service.rs:82` | `issue_jwt(uuid)` → `verify_token` ok, `claims.sub==uuid`, `exp - iat ≈ JWT_EXP_SECS (604800)` ±5s, wrong secret → `Unauthenticated` |
| T9 | `test_verify_token_expired` | `auth.rs:55` | token `exp=now-10` → `Invalid token: ExpiredSignature` |
| T10 | `test_verify_token_wrong_secret` | `auth.rs:55` | `verify_token("wrong")` → `Unauthenticated` |
| T11 | `test_username_validation` | `auth_service.rs:237` | `alice`, `a1_-` (3-32) ok; `ab`, `a*`, 33-char → `InvalidArgument` |

**Integration tests (`server/tests/auth_phase1.rs`, `tokio::test`, `PgPool`, `TRUNCATE` `server/tests/common.rs:10`):**

| # | Test | RPC / SQL | Asserts |
|---|------|-----------|---------|
| I1 | `test_request_otp_mock` | `RequestOTP {phone} sms_mock=true` (`auth_service.rs:101`) | `success=true debug_otp=6 digits`, `SELECT phone,otp_hash,expires_at,attempts` 1 row, `otp_hash` bcrypt verifies, `expires_at ≈ now+OTP_TTL_SECS (300)` ±5s, `attempts=0` |
| I2 | `test_request_otp_no_mock` | `sms_mock=false` | `debug_otp==""` but still inserted |
| I3 | `test_request_otp_invalid_phone` | `phone=123` | → `InvalidArgument`, `0` rows |
| I4 | `test_request_otp_rate_limit_db` | 4× same phone | 4th → `ResourceExhausted` |
| I5 | `test_verify_otp_new_user` | `VerifyOTP {debug_otp}` first time | `DELETE phone_verification` 0 rows after, `SELECT "user" WHERE phone` 1 row `username` auto `user_<last4>_xxxx`, `user_profile` row exists, `token` verifies `sub==user.id`, `User.phone` via `main.rs:292` correct |
| I6 | `test_verify_otp_existing_user_reuse` | 2nd `RequestOTP→VerifyOTP` same phone | same `user.id` reused, no duplicate |
| I7 | `test_verify_otp_with_username` | `username=alice` | `user.username=="alice"`; duplicate `alice` → retry suffix `alice_xxxx` (`auth_service.rs:272`) |
| I8 | `test_verify_otp_wrong_code_increments` | `code=000000` | `attempts 0→1`, → `Unauthenticated`, after 5 wrong → `ResourceExhausted` (`auth_service.rs:198`) |
| I9 | `test_verify_otp_expired` | insert `expires_at=NOW()-1s` | → `Unauthenticated "expired"` (`auth_service.rs:196`) |
| I10 | `test_verify_otp_replay_deleted` | verify correct then repeat same `code` | 2nd → `Unauthenticated` not found (deleted) |
| I11 | `test_verify_otp_6digit_validation` | `code=123` / `abc123` | → `InvalidArgument "6 digits"` (`auth_service.rs:171`) |
| I12 | `test_refresh_token_ok` | `RefreshToken {token}` | new `token` `sub` same, `iat` newer (`auth_service.rs:305`) |
| I13 | `test_refresh_token_invalid` | `token=bad` | → `Unauthenticated` |
| I14 | `test_psql_cleanup` | after success | `SELECT COUNT(*) FROM phone_verification WHERE phone=$1` ==0 |

**Autotest wiring:**
- `server/tests/common.rs` `test_pool()` reads `DATABASE_URL` (expand `${VAR}` via `config.rs:20`), runs `sqlx::migrate!("./migrations")`, helper `truncate_all(pool)` for isolation; `#[ignore]`-free, `cargo test` auto-skips if `DATABASE_URL` not set (returns `pool` error → test panics with clear msg). `--test-threads=1` ensures `DashMap` rate limiter not cross-test polluted (each `AuthServiceImpl::new` gets fresh `Arc<DashMap>`).
- `scripts/test_phase1.sh` — brings `postgres+redis` (`docker compose up -d postgres redis`), waits `pg_isready`, runs `cargo test -- --nocapture`, optionally `grpcurl` E2E if `server` healthy, prints `psql` `phone_verification` count, exits non-zero on failure.

## Phase 2 — Users & Profile (1-2 days)
Uses existing `user_profile` `001_init.sql:20`.
- [ ] 2.1 Proto: `UpdateProfileRequest { display_name, bio, avatar_url }`, `GetMe`, `GetUser` already exists
- [ ] 2.2 Server: `UserService` add `UpdateProfile`, handle avatar upload later via RustFS presigned URL
- [ ] 2.3 Client: `profile_screen.dart:1` — fetch `/me`, edit display_name/bio, logout clears storage
- [ ] Verify: profile round-trip, avatar_url nullable

## Phase 3 — Conversations / Simple Group Chats (3-4 days)
Fixes `ChatRoomService` UNIMPLEMENTED `server/src/main.rs:218`.
- [ ] 3.1 Proto `server/proto/messenger.proto:24` rename/extend:
  ```proto
  service ChatService { // or keep ChatRoomService
    rpc CreateConversation(CreateConversationRequest) returns (Conversation);
    rpc ListConversations(ListConversationsRequest) returns (ListConversationsResponse);
    rpc JoinConversation(JoinRequest) returns (JoinResponse);
    rpc LeaveConversation(LeaveRequest) returns (LeaveResponse);
    rpc AddMembers(AddMembersRequest) returns (Conversation);
  }
  message Conversation { string id=1; string title=2; bool is_group=3; repeated string member_ids=4; string created_by=5; string created_at=6; }
  ```
  `CreateConversationRequest { string title=1; bool is_group=2; repeated string member_ids=3; }` — if `!is_group` enforce 2 members, title nullable
- [ ] 3.2 Server `server/src/main.rs:214` real impl:
  - `CreateConversation`: insert `conversation` + `conversation_participant` rows (creator `owner`, others `member`), check 50-member limit, reject if DM already exists between pair
  - `ListConversations`: `SELECT c.* FROM conversation c JOIN conversation_participant cp ON cp.conversation_id=c.id WHERE cp.user_id=$1`
  - `Join/Leave/AddMembers`: check `owner` role for add, simple `member` removal
  - Replace global `broadcast::Sender<Message> AppState:25` with `Arc<DashMap<Uuid, broadcast::Sender<Message>>>` per `room_id` for group isolation
- [ ] 3.3 Cassandra: no change yet, but ensure `conversation_preview` `007_conversation_preview.cql` updated on each `SendMessage`
- [ ] 3.4 Client: `chat_list_screen.dart:1` — `ListConversations` with pull-to-refresh, FAB create group (title + member picker from `ListUsers`), `chat_room_screen.dart` shows member count
- [ ] Verify: create 1:1, create group 3-50 members, list filtering by user, leave removes participant

## Phase 4 — Real-time Messaging Core (3-4 days)
Hardens `MessageService` `server/proto/messenger.proto:17`.
- [ ] 4.1 Server `server/src/main.rs:138`:
  - `SendMessage`: authz check `conversation_participant` contains `sender_id` (from JWT), insert into `messenger.messages` `002_messages.cql` + `messages_by_user` `005_messages_by_user.cql` + `message_delivery` `003_message_delivery.cql` (one row per recipient) + `unread_counters` `006_unread_counters.cql` increment, update `conversation_preview`, broadcast to per-room `Sender`
  - `GetHistory`: query `SELECT ... FROM messenger.messages WHERE room_id=? ORDER BY created_at DESC LIMIT ?` with `req.limit` capped 50, support `paging_state` for infinite scroll (use `scylla` paging)
  - `StreamMessages`: validate membership, `tx.subscribe()` filtered by `room_id`, handle lag (`BroadcastStreamLagged`) → client refetch history
- [ ] 4.2 Client: `chat_room_screen.dart` — optimistic send, `StreamMessages` subscription per `roomId` via `messageService`, history pagination on scroll top, reconnect on app resume
- [ ] 4.3 Verify: two Android emulators in same room receive stream, history order newest-first, 100-msg burst no loss, leaving group stops stream

## Phase 5 — Attachments via RustFS (2-3 days)
- [ ] 5.1 Infra: `docker-compose.yml` add RustFS, init bucket on server start (`aws-sdk-s3` create_bucket if not exists), add env to `.env:1` `RUSTFS_ENDPOINT=http://rustfs:9000`, `RUSTFS_BUCKET=messenger-attachments`
- [ ] 5.2 Proto: extend `Message { repeated Attachment attachments = 6; }` and `Attachment { string id=1; string file_url=2; string file_name=3; string mime_type=4; int64 file_size=5; }` + `GetUploadUrl(GetUploadUrlRequest { file_name, mime_type, file_size }) returns (GetUploadUrlResponse { upload_url, attachment_id })`
- [ ] 5.3 Server: `attachment` table already exists `001_init.sql:122`, new endpoint `GetUploadUrl` generates presigned PUT URL (15-min expiry) via RustFS S3 API, `SendMessage` now accepts `attachment_ids` and validates uploader owns them, writes `attachment.message_id`
  - Enforce 50MB limit, mime whitelist `image/*, video/*, application/pdf, audio/*`, store `file_url` as `s3://bucket/key` or presigned GET URL
- [ ] 5.4 Client: `chat_room_screen.dart` — attach button → picker (`image_picker`/`file_picker`), request presigned URL, `http.put` to RustFS, then `SendMessage` with `attachment_ids`, render image preview vs file tile, download via cached GET URL
- [ ] 5.5 Verify: upload 10MB image from Android, check RustFS console `http://localhost:9000`, message history shows attachment, download opens

## Phase 6 — Minimal Polish: Delivery & Read (1-2 days)
Simplest adult-system parity, no reactions/edit.
- [ ] 6.1 Proto: `MarkRead(MarkReadRequest { room_id, last_message_id }) returns (Empty)`
- [ ] 6.2 Server: update `read_receipts` `004_read_receipts.cql` + decrement `unread_counters` `006_unread_counters.cql` + `conversation_preview.unread_count`
- [ ] 6.3 Client: `chat_list_screen.dart` badge from `unread_counters`, `chat_room_screen.dart` auto `MarkRead` on visible, `device` FCM registration `device` table for Android push (simple data message, no payload encryption in v1)
- [ ] Verify: unread badge increments on background, clears on open

## Phase 7 — Security & Android Hardening (1-2 days)
- [ ] 7.1 JWT: `server/src/main.rs:52` add `exp`, `iat`, refresh flow, `ChannelCredentials.secure` for prod (insecure only for `localhost` `grpc_client.dart:24`)
- [ ] 7.2 Rate limits: OTP 3/min, SendMessage 20/sec per user, upload 5/min
- [ ] 7.3 Android: `flutter_secure_storage` for JWT, `shared_preferences` for `last_room_id`, handle Doze/FCM background isolate
- [ ] 7.4 Verify: expired JWT returns `Unauthenticated`, rate limit returns `ResourceExhausted`

## Phase 8 — QA & Launch Prep (2 days)
- [ ] 8.1 Tests: `cargo test --workspace` (auth, conversation, message), `flutter test` (auth provider, chat list), manual 2-device matrix: OTP, 1:1, group 50, 50MB attachment, offline → online sync
- [ ] 8.2 Perf: Cassandra `cqlsh` `CONSISTENCY QUORUM`, Postgres indexes already present `001_init.sql:15`, load 1k msgs/room pagination <200ms
- [ ] 8.3 Prod: `docker-compose.prod.yml` — RustFS behind `127.0.0.1` or private network, `JWT_SECRET` from `.env.prod`, `flutter build apk` + `server` release binary `Cargo.toml:41`
- [ ] 8.4 Docs: update `PROJECT_STRUCTURE.md:150` env table, `AGENTS.md:12` commands for RustFS (`docker compose exec rustfs ...`)

## Phase 9 — Deferred (Post-v1)
- E2E encryption (Signal protocol lib `libsignal`), invite links, admin roles, reactions/reply/edit/forward, typing/presence, full-text search, iOS/web, international SMS (Twilio), video/voice calls.

## Execution Order (Strict)
0 → 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9 — each phase merges only when `grpcurl` + 2 Android emulators pass the verify checklist. Do not start Phase 5 before per-room broadcast (Phase 3) is done.

## How to Start Now
1. `docker compose up -d postgres cassandra rustfs` (add RustFS first)
2. Implement Phase 0 fixes, run `cargo sqlx migrate run` + `cqlsh -f migrations/cassandra/*.cql`
3. Implement Phase 1 proto + `phone_verification` table, test OTP mock flow
4. Continue sequentially; keep `STEPS.md` checkboxes updated.

---
*Stack: Tonic 0.12 + Axum, sqlx 0.8 + scylla 0.15, RustFS S3, Flutter 3.47 Riverpod/go_router, JWT+bcrypt, FCM Android.*
