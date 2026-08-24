-- ============================================================
-- MESSAGES_BY_USER
-- Index of messages sent by each user, newest-first.
-- Primary read pattern: list a user's sent messages / sent history.
-- ============================================================
CREATE TABLE IF NOT EXISTS messenger.messages_by_user (
    user_id      uuid,
    created_at   timestamp,
    message_id   uuid,
    room_id      uuid,
    sender_id    uuid,
    content      text,
    PRIMARY KEY ((user_id), created_at, message_id)
) WITH CLUSTERING ORDER BY (created_at DESC, message_id ASC);