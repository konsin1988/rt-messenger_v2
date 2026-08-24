-- ============================================================
-- CONVERSATION_PREVIEW
-- Denormalized last-message snapshot per room for the chat list.
-- Primary read pattern: inbox/conversation list without joining
-- into the full messages table.
-- ============================================================
CREATE TABLE IF NOT EXISTS messenger.conversation_preview (
    room_id           uuid PRIMARY KEY,
    last_message_id   uuid,
    last_sender_id    uuid,
    last_content      text,
    last_message_at   timestamp
);