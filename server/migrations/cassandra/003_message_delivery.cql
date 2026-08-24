-- ============================================================
-- MESSAGE_DELIVERY
-- Delivery state per message per recipient.
-- Primary read pattern: delivery status of a given message
-- (which recipients have received/delivered it yet).
-- ============================================================
CREATE TABLE IF NOT EXISTS messenger.message_delivery (
    room_id        uuid,
    message_id     uuid,
    user_id        uuid,
    status         text,      -- 'sent' | 'delivered' | 'failed'
    delivered_at   timestamp,
    PRIMARY KEY ((room_id), message_id, user_id)
);