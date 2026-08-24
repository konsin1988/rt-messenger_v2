-- ============================================================
-- MESSAGES
-- Stores every message, ordered newest-first within a room.
-- Primary read pattern: fetch history for a room.
-- ============================================================
CREATE TABLE IF NOT EXISTS messenger.messages (
    id          uuid,
    room_id     uuid,
    sender_id   uuid,
    content     text,
    created_at  timestamp,
    PRIMARY KEY ((room_id), created_at, id)
) WITH CLUSTERING ORDER BY (created_at DESC, id ASC);