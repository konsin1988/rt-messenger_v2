# Messenger v2 — Use Cases

> All modern messenger features — functional catalog for implementation.
> Stack: Flutter 3.47 (Riverpod, go_router) + Rust Tonic gRPC 0.12 + Postgres 15 + Cassandra 5.0.9 + RustFS S3.
> Proto: `server/proto/messenger.proto` | DB: `server/migrations/001_init.sql` + `server/migrations/cassandra/*.cql`
> Last updated: 2026-08-25

---

## 1. Actors & Roles

| Actor | Description |
|-------|-------------|
| **Guest** | Unauthenticated user |
| **User** | Authenticated user (`user` + `user_profile` tables) |
| **Participant** | User in a `conversation` (`conversation_participant`) |
| **Owner** | `role='owner'` — creator of conversation, can manage members/settings |
| **Admin** | `role='admin'` — delegated moderator (promoted by Owner) |
| **Member** | `role='member'` — default participant |
| **System** | Server, RustFS, FCM push service |

Device: Android only v1 (`device.platform='android'`), FCM token per `device` table.

---

## 2. Use Case Map (Summary)

| Epic | ID | Use Case | Priority | Status |
|------|----|----------|----------|--------|
| **A — Auth & Session** | UC-A1 | Request OTP (phone) | Must | Planned (STEPS Phase 1) |
| | UC-A2 | Verify OTP & Auto-Register/Login | Must | Planned |
| | UC-A3 | Register with Email+Password (legacy/fallback) | Should | Implemented (`UserService.Register` `main.rs:43`) |
| | UC-A4 | Login with Email+Password | Should | Implemented (`main.rs:75`) |
| | UC-A5 | Refresh JWT | Must | Planned (`AuthService.RefreshToken` STEPS 1.2) |
| | UC-A6 | Logout (single device) | Must | Planned |
| | UC-A7 | Logout All Devices | Should | Planned |
| | UC-A8 | Re-auth / Token Expiry Handling | Must | Partial (`auth.rs:13` Claims exp/iat) |
| **B — Profile** | UC-B1 | View Own Profile (`GetMe`) | Must | Planned STEPS 2 |
| | UC-B2 | View Other User Profile | Must | Implemented (`GetUser` `main.rs:110`) |
| | UC-B3 | Update Profile (display_name, bio) | Must | Planned |
| | UC-B4 | Upload Avatar (RustFS presigned URL) | Must | Planned |
| | UC-B5 | Set Last Seen / Online Visibility | Should | Planned |
| | UC-B6 | Delete Account | Could | Deferred |
| **C — Contacts & Discovery** | UC-C1 | List Users / Search by Username | Must | Implemented (`ListUsers` `main.rs:128`) |
| | UC-C2 | Search Users by Phone/Username (server-side) | Must | Planned |
| | UC-C3 | Add Contact | Must | Planned (`contact` table) |
| | UC-C4 | Remove Contact | Must | Planned |
| | UC-C5 | Sync Phone Contacts (optional) | Could | Deferred |
| | UC-C6 | Block User | Must | Planned (`block` table) |
| | UC-C7 | Unblock User | Must | Planned |
| **D — Conversations** | UC-D1 | Create 1:1 Conversation | Must | Planned STEPS 3 |
| | UC-D2 | Create Group (title, members up to 50) | Must | Planned (`conversation.is_group`, 50 limit) |
| | UC-D3 | List Conversations (my chats) | Must | Planned (`ChatRoomService.ListRooms` UNIMPLEMENTED `main.rs:270`) |
| | UC-D4 | Get Conversation Details + Members | Must | Planned |
| | UC-D5 | Add Members (owner/admin only) | Must | Planned |
| | UC-D6 | Remove Member / Kick | Must | Planned |
| | UC-D7 | Leave Conversation | Must | Planned (`LeaveRoom` UNIMPLEMENTED) |
| | UC-D8 | Delete Conversation (owner only) | Should | Planned |
| | UC-D9 | Rename Group / Change Avatar | Should | Planned |
| | UC-D10 | Role Management (promote/demote admin) | Should | Planned (`conversation_participant.role` owner/admin/member) |
| | UC-D11 | Generate Invite Link | Could | Deferred (STEPS 9) |
| | UC-D12 | Join via Invite Link | Could | Deferred |
| **E — Messaging Core** | UC-E1 | Send Text Message | Must | Implemented (`MessageService.SendMessage` `main.rs:150`, `messenger.messages` CQL) |
| | UC-E2 | Stream Real-time Messages | Must | Implemented (`StreamMessages` `main.rs:186` BroadcastStream) |
| | UC-E3 | Get Message History (paginated) | Must | Implemented (`GetHistory` `main.rs:201`, capped limit) |
| | UC-E4 | Edit Message (sender, time-limited) | Should | Planned |
| | UC-E5 | Delete Message for Everyone | Should | Planned |
| | UC-E6 | Delete Message for Self | Should | Planned |
| | UC-E7 | Reply to Message (thread) | Should | Planned |
| | UC-E8 | Forward Message | Should | Planned |
| | UC-E9 | Pin Message | Could | Planned |
| | UC-E10 | Copy / Select Messages | Must | Client UI |
| | UC-E11 | Mention User (@username) | Should | Planned |
| | UC-E12 | Draft Save (per conversation) | Could | Client local |
| **F — Attachments (RustFS)** | UC-F1 | Get Presigned Upload URL | Must | Planned STEPS 5 (`GetUploadUrl`) |
| | UC-F2 | Upload Image / Photo | Must | Planned (50 MB, `image/*`) |
| | UC-F3 | Upload File / Document | Must | Planned (`application/pdf`, etc.) |
| | UC-F4 | Upload Voice Note (audio/*) | Should | Planned |
| | UC-F5 | Upload Video | Should | Planned (`video/*`) |
| | UC-F6 | Send Message with Attachments | Must | Planned (`attachment` table `001_init.sql:122`, s3:// or presigned GET) |
| | UC-F7 | Download / Preview Attachment | Must | Planned (presigned GET, cached) |
| | UC-F8 | Render Gallery / File Tiles in Chat | Must | Client |
| | UC-F9 | Cancel / Retry Upload | Should | Client |
| **G — Delivery & Presence** | UC-G1 | Track Delivery (sent/delivered) | Must | Planned (`message_delivery` `003_message_delivery.cql`) |
| | UC-G2 | Read Receipts (per message) | Must | Planned (`read_receipts` `004_read_receipts.cql`, `MarkRead` STEPS 6) |
| | UC-G3 | Unread Counters / Badges | Must | Planned (`unread_counters` `006_unread_counters.cql`, `conversation_preview` `007_...`) |
| | UC-G4 | Typing Indicator | Should | Planned |
| | UC-G5 | Online / Last Seen | Should | Planned (`user_profile.last_seen_at`) |
| | UC-G6 | Presence Subscription (online events) | Could | Deferred |
| **H — Notifications** | UC-H1 | Register FCM Device Token | Must | Planned (`device` table, `fcm_token`) |
| | UC-H2 | Push on New Message (background/killed) | Must | Planned STEPS 6.2 |
| | UC-H3 | In-App Notification / Snackbar | Should | Client |
| | UC-H4 | Mute Conversation | Must | Planned (`conversation_setting.is_muted`) |
| | UC-H5 | Pin Conversation | Should | Planned (`is_pinned`) |
| | UC-H6 | Archive Conversation | Should | Planned (`is_archived`) |
| | UC-H7 | Notification Sound / Vibration Settings | Could | Client |
| **I — Search & History** | UC-I1 | Search Messages in Conversation | Should | Planned |
| | UC-I2 | Global Search (all chats) | Could | Deferred (no full-text in v1) |
| | UC-I3 | Search Conversations by Title | Should | Planned |
| | UC-I4 | Jump to Message / Scroll | Should | Client |
| **J — Reactions & Interactivity** | UC-J1 | Emoji Reactions | Could | Deferred (STEPS 9: no reactions v1) |
| | UC-J2 | Polls | Could | Deferred |
| | UC-J3 | Message Reply Thread View | Should | Planned |
| **K — Privacy & Safety** | UC-K1 | Block → Hide Messages / Prevent Adds | Must | Planned (check `block` on send/create) |
| | UC-K2 | Report User/Message | Could | Deferred |
| | UC-K3 | Hide Phone / Profile Visibility Setting | Should | Planned |
| | UC-K4 | Disappearing Messages (TTL) | Could | Deferred |
| **L — Calls (future)** | UC-L1 | 1:1 Voice Call (WebRTC) | Could | Deferred STEPS 9 |
| | UC-L2 | 1:1 Video Call | Could | Deferred |
| | UC-L3 | Group Call | Won't | Deferred |
| **M — Reliability & Offline** | UC-M1 | Offline Queue (send when back online) | Should | Client |
| | UC-M2 | Reconnect Stream on Resume | Must | Planned (`BroadcastStreamLagged` → refetch `main.rs:103`) |
| | UC-M3 | Paging / Infinite Scroll History | Must | Planned (scylla paging_state, limit 50) |
| | UC-M4 | Optimistic UI + Retry | Should | Client |

---

## 3. Detailed Use Cases

### Epic A — Auth & Session

#### UC-A1 — Request OTP
- **Actor:** Guest
- **Pre:** Phone valid `^\+?[0-9]{10,15}$` single country code (configurable).
- **Flow:** 1. Enter phone 2. Client `AuthService.RequestOTP{phone}` 3. Server rate-limit 3/min per phone+IP (DashMap) 4. Generate 6-digit, bcrypt hash, `INSERT phone_verification{phone, otp_hash, expires_at=+5min, attempts=0}` 5. If `SMS_MOCK=true` log + return `debug_otp`, else Twilio stub 6. Client shows OTP screen + 60s resend timer.
- **Alt:** Invalid format → `InvalidArgument`; rate limited → `ResourceExhausted`.
- **Post:** Row in `phone_verification`, OTP logged.
- **Proto:** `RequestOTPRequest/Response` (STEPS 1.2).

#### UC-A2 — Verify OTP & Auto-Register/Login
- **Actor:** Guest
- **Pre:** OTP requested, not expired.
- **Flow:** 1. Enter 6-digit code + optional username on first verify 2. `VerifyOTP{phone, code, username}` 3. Fetch latest non-expired row, `attempts<5`, `bcrypt::verify` 4. Inc `attempts` on fail 5. On success delete row, `SELECT user WHERE phone=?` else `INSERT user(phone, username) + user_profile` 6. Issue JWT `Claims{sub, exp=7d, iat}` `jsonwebtoken` `main.rs:62` 7. Store JWT `flutter_secure_storage`, attach `authorization: Bearer` via `grpc_client.dart:24` interceptor `auth.rs:25`.
- **Alt:** Expired → `Unauthenticated`; attempts≥5 → blocked 15min; phone exists → login branch.
- **Post:** Authenticated, token stored, `go_router` guard → `/chats`.

#### UC-A3/A4 — Email+Password Register/Login
- **Current:** Implemented `UserService.Register/Login` `proto:9` `main.rs:43,75` — kept for compat/dev, will be deprecated but not removed. Requires `username`, `email`, `password`, bcrypt hash, JWT.

#### UC-A5 — Refresh JWT
- **Flow:** `RefreshToken{refresh_token}` → verify & rotate, new `exp`. Used when `verify_token` `auth.rs:55` fails with expired. Client refreshes silently.

#### UC-A6/A7 — Logout
- **Flow:** Clear `flutter_secure_storage` + `shared_preferences`, optionally `DELETE device WHERE fcm_token=?`, revoke refresh token. All devices: delete all `device` rows for user.

#### UC-A8 — Token Expiry Handling
- **Flow:** Interceptor `AuthInterceptor::extract_user_id` `auth.rs:25` validates HS256; on expiry client gets `Unauthenticated` → refresh or redirect `/login`.

---

### Epic B — Profile

#### UC-B1/B2 — View Profile
- **Flow:** `GetUser{user_id}` `main.rs:110` or `GetMe` (from JWT `sub`). Returns `User{id, username, email, phone, created_at}` + `user_profile{display_name, bio, avatar_url, last_seen_at}`. Client `profile_screen.dart:1` displays placeholders + fetch.
- **Pre:** Authenticated.

#### UC-B3 — Update Profile
- **Flow:** `UpdateProfile{display_name, bio}` → `UPDATE user_profile SET ... , updated_at=NOW() WHERE user_id=$1`. Validate length 1-64/500. Avatar via UC-B4.

#### UC-B4 — Upload Avatar
- **Flow:** `GetUploadUrl{file_name, mime_type, file_size}` → S3 presigned PUT 15min `storage.rs:create_s3_client` `main.rs:320` → `http.put` to RustFS `http://rustfs:9000` → `UpdateProfile{avatar_url=s3://bucket/key or presigned GET}`. Whitelist `image/*`, 5 MB avatar limit.

#### UC-B5 — Last Seen / Online Visibility
- **Flow:** On app foreground set `last_seen_at=NOW()`, on disconnect update. Setting: `Everyone / Contacts / Nobody`.

---

### Epic C — Contacts & Discovery

#### UC-C1/C2 — List/Search Users
- **Flow:** `ListUsers` `main.rs:128` returns all (dev); prod adds `WHERE username ILIKE $1 OR phone LIKE $1 LIMIT 20` pagination. Used for group member picker `chat_list_screen.dart:1`.
- **Alt:** Empty query → return recent contacts.

#### UC-C3/C4 — Add/Remove Contact
- **Flow:** `AddContact{contact_user_id}` → `INSERT contact(user_id, contact_user_id)` unique pair `001_init.sql:49`; remove → `DELETE`. `ListContacts` joins `user`.

#### UC-C6/C7 — Block/Unblock
- **Flow:** `BlockUser{blocked_id}` → `INSERT block(blocker_id, blocked_id)` `001_init.sql:63`; unblock → `DELETE`. Effects: blocked cannot DM you, cannot add you to groups, messages hidden. Checked in `CreateConversation`/`SendMessage` interceptor.

---

### Epic D — Conversations

#### UC-D1 — Create 1:1 Conversation
- **Flow:** `CreateConversation{is_group=false, member_ids=[other]}` → validate 2 members, no duplicate `SELECT` where both participants exist with `is_group=false` → `INSERT conversation{title=null, is_group=false, created_by}` + 2× `conversation_participant` (creator `owner`, other `member`), `conversation_setting` rows.
- **Proto:** `ChatService.CreateConversation` (rename from `ChatRoomService.CreateRoom` `proto:24`).

#### UC-D2 — Create Group
- **Flow:** `CreateConversation{title, is_group=true, member_ids[]}` → title 1-64 required, 3-50 members (STEPS 3.2), atomic tx. Title nullable for DMs.
- **Alt:** >50 → `InvalidArgument`; duplicate members deduped.

#### UC-D3 — List Conversations
- **Flow:** `ListConversations{}` uses JWT `sub` → `SELECT c.* FROM conversation c JOIN conversation_participant cp ON cp.conversation_id=c.id WHERE cp.user_id=$1 ORDER BY updated_at DESC`. Hydrate `member_ids`, `unread_count` from `conversation_preview`/`unread_counters`, `last_message`. Client pull-to-refresh `chat_list_screen.dart`.
- **Current:** UNIMPLEMENTED `main.rs:270` — Must implement.

#### UC-D4 — Get Conversation Details
- **Flow:** `GetConversation{id}` → check membership → return `Conversation{id,title,is_group,member_ids[],created_by,created_at, participant roles}`.

#### UC-D5/D6 — Add/Remove Members
- **Flow:** `AddMembers{conversation_id, user_ids[]}` → authz caller `owner/admin`, enforce 50 cap, skip blocked users. `RemoveMember` similar. Broadcast `ConversationUpdated` event (future).

#### UC-D7 — Leave Conversation
- **Flow:** `LeaveConversation{conversation_id}` → `DELETE FROM conversation_participant WHERE conversation_id=$1 AND user_id=$2`. If `owner` leaves, promote earliest `admin`/`member` to `owner` or delete conversation if last member. `LeaveRoom` currently UNIMPLEMENTED.

#### UC-D8 — Delete Conversation
- **Flow:** Owner only → `DELETE conversation` cascade participants/settings; messages in Cassandra kept (or tombstoned) — history retained for others if 1:1? v1: hard delete only for groups with 0 members.

#### UC-D9/D10 — Rename / Role Mgmt
- **Flow:** `UpdateConversation{title, avatar_url}` + `UpdateRole{user_id, role}` — owner only can promote member→admin.

#### UC-D11/D12 — Invite Links (Deferred)
- **Flow:** `CreateInviteLink` generates `invite_token` UUID, `JoinConversation{token}` validates.

---

### Epic E — Messaging Core

#### UC-E1 — Send Text Message
- **Flow:** `SendMessage{room_id, content}` auth via `AuthInterceptor` `main.rs:150` extracts `sender_id` from JWT `extensions` `main.rs:154` → authz `conversation_participant` contains sender (else `PermissionDenied`) → `INSERT messenger.messages (id, room_id, sender_id, content, created_at)` `002_messages.cql` + `messages_by_user` `005_...` + `message_delivery` per recipient `003_...` + `unread_counters` inc `006_...` + `conversation_preview` update `007_...` → `broadcast::Sender<Message>` per-room (`DashMap<Uuid, Sender>` STEPS 3.2, current global `AppState.tx:30` must shard) `tx.send`.
- **Validate:** content 1-4096 chars, rate 20/sec per user (STEPS 7.2).

#### UC-E2 — Stream Real-time
- **Flow:** `StreamMessages{room_id}` validates membership → `tx.subscribe()` → `BroadcastStream` `main.rs:192` `filter_map` by `room_id`, `Pin<Box<dyn Stream>>`. Client subscribes per `roomId` `chat_room_screen.dart`, reconnects on resume, on `Lagged` → refetch `GetHistory`.
- **Current:** No `room_id` filter — must add per-room filtering.

#### UC-E3 — History (Paginated)
- **Flow:** `GetHistory{room_id, limit, paging_state?}` → `SELECT ... WHERE room_id=? ORDER BY created_at DESC LIMIT ?` using scylla paging. Caps `limit` 50. Client infinite scroll on top, 1k msgs <200ms (STEPS 8.2).
- **Current:** Implemented without paging_state, cap via `req.limit` — extend.

#### UC-E4 — Edit Message
- **Flow:** `EditMessage{message_id, content}` → only sender, within 10min, `UPDATE messages SET content=?, edited_at=NOW() WHERE id=?` (Cassandra LWW) + broadcast `MessageEdited` event.

#### UC-E5/E6 — Delete
- **For Everyone:** sender within 1h → tombstone or `DELETE` + `message_delivery` cleanup + broadcast `MessageDeleted`.
- **For Self:** `conversation_setting.last_read` + client local hide (server keeps row).

#### UC-E7 — Reply
- **Flow:** `SendMessage{content, reply_to_id}` → store `reply_to` UUID, render quoted preview, `GetHistory` joins reply content.

#### UC-E8 — Forward
- **Flow:** Select message → picker conversation → `SendMessage` with `forwarded_from` metadata + original content.

#### UC-E11 — Mention
- **Field:** Parse `@username`, validate members, store `mentioned_ids`, push mentions even if muted.

---

### Epic F — Attachments (RustFS S3)

#### UC-F1 — Get Presigned Upload URL
- **Flow:** `GetUploadUrl{file_name, mime_type, file_size}` → validate 50 MB limit, whitelist `image/*, video/*, audio/*, application/pdf` → `aws-sdk-s3` presign PUT 15min `storage.rs` → `CreateBucket` if missing `ensure_bucket_exists` `main.rs:326` → return `{upload_url, attachment_id}`.
- **Rate:** 5/min per user.

#### UC-F2-F5 — Upload
- **Client:** `image_picker`/`file_picker` → `GetUploadUrl` → `http.put` bytes to `RUSTFS_ENDPOINT=http://rustfs:9000` (docker) / `RUSTFS_PUBLIC_ENDPOINT=http://localhost:9000` for client → on 200 → `SendMessage{attachment_ids: [id]}`.
- **Server:** Validates uploader owns `attachment` rows (before message), writes `attachment.message_id` `001_init.sql:122`.

#### UC-F6 — Send with Attachments
- **Proto extension:** `Message{repeated Attachment attachments=6}` `Attachment{id, file_url, file_name, mime_type, file_size}` + `SendMessageRequest{attachment_ids}`.

#### UC-F7/F8 — Download/Preview
- **Flow:** `GetAttachmentUrl{id}` → presigned GET (15min) → cached via `flutter_cache`. Images inline, files as tiles with name/size, video with thumbnail. RustFS console `http://localhost:9001`.

---

### Epic G — Delivery & Presence

#### UC-G1/G2 — Delivery/Read
- **Flow:** On `SendMessage` insert `message_delivery{message_id, recipient_id, status='sent'}` `003_...`; when recipient Stream receives → `delivered`; `MarkRead{room_id, last_message_id}` → `INSERT read_receipts` `004_...` + `UPDATE unread_counters SET count = count - N` `006_...` + `conversation_preview.unread_count` decrement.
- **Proto:** `MarkRead` STEPS 6.1.

#### UC-G3 — Unread Badges
- **Flow:** `chat_list_screen.dart` badge = `SELECT count FROM unread_counters WHERE user_id=? AND room_id=?`. Clears on `MarkRead` when opening `chat_room_screen`.

#### UC-G4 — Typing Indicator
- **Flow:** Client `SendTyping{room_id, is_typing}` (transient, not persisted) → broadcast to room via same per-room `Sender` (separate channel or `Message{type=TYPING}`), auto-stop after 3s / on send.

#### UC-G5 — Online / Last Seen
- **Flow:** `UpdatePresence{status=online}` on app resume/foreground → `UPDATE user_profile SET last_seen_at=NOW()`; offline after 60s idle. Visibility respects UC-K3.

---

### Epic H — Notifications

#### UC-H1 — Register Device
- **Flow:** On login `RegisterDevice{fcm_token, platform='android'}` → `INSERT device(user_id, fcm_token, platform)` upsert `is_active=true`. On logout `is_active=false`.

#### UC-H2 — Push
- **Flow:** Server after `SendMessage` queries active `device` tokens for recipients → FCM data message `{room_id, sender_name, preview}` — no plaintext payload encryption v1 (deferred). Suppressed if `is_muted`.

#### UC-H4/H5/H6 — Mute/Pin/Archive
- **Flow:** `UpdateSetting{conversation_id, is_muted/pinned/archived}` → `UPSERT conversation_setting` `001_init.sql:104`. Muted → no push/sound; pinned → top of `ListConversations` order; archived → hidden filter. Settings per-user.

---

### Epic I — Search & History

#### UC-I1/I3 — Search
- **Flow:** `SearchMessages{room_id, query}` → Cassandra `ALLOW FILTERING` on `messages` (limited) or Postgres log for v1; `SearchConversations{query}` → `WHERE title ILIKE %query%`. Global search deferred (no Elasticsearch in v1).
- **Client:** Debounced search bar, highlight matches.

#### UC-I4 — Jump to Message
- **Flow:** Search result tap → `GetHistory` around `message_id` + scroll animation.

---

### Epic J — Reactions & Interactivity (Deferred v1, STEPS 9)

- **UC-J1** Reactions: `AddReaction{message_id, emoji}` stored `reactions` table (future), broadcast, max 1 per user per message.
- **UC-J2** Polls: `CreatePoll{question, options[]}` rendered as message type poll.
- **UC-J3** Threads: Reply threading UI.

---

### Epic K — Privacy & Safety

#### UC-K1 — Block Enforcement
- Checks in `CreateConversation`, `AddMembers`, `SendMessage`, `StreamMessages` — block unilateral (blocker hides blocked).

#### UC-K3 — Visibility
- `privacy_settings{phone_visibility, last_seen_visibility, avatar_visibility: Everyone|Contacts|Nobody}` stored in `user_profile` extension or new `privacy_setting` table.

#### UC-K4 — Disappearing Messages
- `conversation_setting{ttl_seconds}` → server TTL `USING TTL` on Cassandra insert.

---

### Epic L — Calls (Deferred Post-v1, STEPS 9)

- **UC-L1/L2** WebRTC SFU (e.g., `livekit`) 1:1 voice/video, `CallService {Initiate, Signal, End}` — not in current proto.
- Requires STUN/TURN, push to ring.

---

### Epic M — Reliability & Offline

#### UC-M1 — Offline Queue
- Client holds unsent `SendMessageRequest` in `shared_preferences` + retry with exponential backoff when `grpc_client` reconnects. Shows “pending” checkmark.

#### UC-M2 — Reconnect
- `StreamMessages` `BroadcastStreamLagged` handling `main.rs:192` → client refetch `GetHistory` since lag. On app resume re-subscribe.

#### UC-M3 — Paging
- Scylla `paging_state` opaque token passed as `GetHistoryRequest.paging_state`. Server returns `next_paging_state`.

#### UC-M4 — Optimistic UI
- Insert locally before ack, update `id`/`timestamp` on `SendMessage` response, show error/retry on `Status::ResourceExhausted`.

---

## 4. Non-Functional Requirements

| Category | Requirement |
|----------|-------------|
| **Perf** | P95 `GetHistory` 50 msgs <200ms (STEPS 8.2), `CONSISTENCY QUORUM` Cassandra, Postgres indexes `001_init.sql:15`. |
| **Scale** | Per-room `broadcast::Sender` sharding (DashMap) for group isolation STEPS 3.2; Cassandra RF=1 dev, RF=3 prod. |
| **Security** | JWT HS256 `exp/iat` `auth.rs:7`, rate limits OTP 3/min, Send 20/s, Upload 5/min STEPS 7.2, `authorization: Bearer`. |
| **Storage** | RustFS `rustfs:1.0.0-rc.3` 9000/9001, `aws-sdk-s3` force_path_style `storage.rs`, bucket auto-create. |
| **Client** | `flutter_secure_storage` JWT, `shared_preferences` last_room_id, Doze/FCM isolate STEPS 7.3, `flutter build apk`. |
| **Proto** | Breaking changes → copy `server/proto/messenger.proto` → `client/lib/core/api/generated/` → `build_runner` (PROJECT_STRUCTURE.md workflow). |

---

## 5. Implementation Phases (STEPS.md traceability)

| Phase | STEPS | UCs Covered |
|-------|-------|-------------|
| 0 Foundation Fix | 0.1-0.5 | A3,A4,E1-E3 (fix `sender_id` stub, deps, RustFS, stubs) |
| 1 OTP Auth | 1.1-1.4 | A1,A2,A5,A6,A8,B5 |
| 2 Profile | 2.1-2.3 | B1-B4, C1 |
| 3 Conversations | 3.1-3.4 | D1-D10, G3 (preview), M3 |
| 4 Real-time | 4.1-4.3 | E1-E3,G1-G3,H2,M1-M4 |
| 5 Attachments | 5.1-5.5 | F1-F9,B4 |
| 6 Delivery & Read | 6.1-6.3 | G1-G5,H1-H6 |
| 7 Security | 7.1-7.4 | A5,A8,K1-K3 + rate limits |
| 8 QA & Prod | 8.1-8.4 | All Must UCs verified via `grpcurl` + 2 emulators matrix |
| 9 Deferred | 9 | J1-J3,L1-L3,C5,D11-D12,K2,K4 |

Strict order `0→8` where each merges only after `grpcurl` + 2-device verification.

---

## 6. Verification Matrix (examples)

| UC | grpcurl / Test |
|----|----------------|
| A1,A2 | `grpcurl -plaintext -d '{"phone":"+79990001122"}' localhost:50051 messenger.AuthService/RequestOTP` → `VerifyOTP` → JWT; check `psql SELECT * FROM phone_verification`. |
| D1-D3 | Create 1:1 + group 50, `ListConversations` filtered by JWT user, leave removes participant. |
| E1-E3 | Two emulators same room Stream receives, history newest-first, 100-msg burst no loss. |
| F2,F6 | Upload 10MB image → RustFS console 9000 → history shows attachment → download. |
| G2,G3 | Badge increments background, clears on open `MarkRead`. |
| H2 | Kill app → send → FCM received. |

---

*Next: keep this file updated — check `[x]` as UCs land; out-of-scope items remain under Deferred for post-v1.*
