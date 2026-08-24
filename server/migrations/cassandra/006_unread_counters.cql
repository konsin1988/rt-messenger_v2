-- ============================================================
-- UNREAD_COUNTERS
-- Running unread message count per user per room (counter table).
-- Primary read pattern: badge count for a user in a room / inbox.
-- NOTE: counters cannot be inserted with a value; use UPDATE ... +1/-1.
-- ============================================================
CREATE TABLE IF NOT EXISTS messenger.unread_counters (
    user_id        uuid,
    room_id        uuid,
    unread_count   counter,
    PRIMARY KEY ((user_id), room_id)
);