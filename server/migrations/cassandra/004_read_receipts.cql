-- ============================================================
-- READ_RECEIPTS
-- Per-message read tracking, one row per reader.
-- Primary read pattern: which users have read a given message.
-- ============================================================
CREATE TABLE IF NOT EXISTS messenger.read_receipts (
    room_id      uuid,
    message_id   uuid,
    user_id      uuid,
    read_at      timestamp,
    PRIMARY KEY ((room_id, message_id), user_id)
);