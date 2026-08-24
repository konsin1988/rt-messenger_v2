CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- ============================================================
-- USER
-- ============================================================
CREATE TABLE IF NOT EXISTS "user" (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    username        VARCHAR(255) UNIQUE NOT NULL,
    email           VARCHAR(255) UNIQUE NOT NULL,
    password_hash   VARCHAR(255) NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_user_email ON "user"(email);

-- ============================================================
-- USER_PROFILE
-- ============================================================
CREATE TABLE IF NOT EXISTS user_profile (
    user_id         UUID PRIMARY KEY REFERENCES "user"(id) ON DELETE CASCADE,
    display_name    VARCHAR(255),
    bio             TEXT,
    avatar_url      TEXT,
    phone           VARCHAR(50),
    last_seen_at    TIMESTAMPTZ,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================
-- DEVICE
-- ============================================================
CREATE TABLE IF NOT EXISTS device (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id         UUID NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    fcm_token       TEXT NOT NULL,
    platform        VARCHAR(20) NOT NULL CHECK (platform IN ('ios', 'android', 'web')),
    is_active       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_device_user_id ON device(user_id);
CREATE INDEX IF NOT EXISTS idx_device_fcm_token ON device(fcm_token);

-- ============================================================
-- CONTACT
-- ============================================================
CREATE TABLE IF NOT EXISTS contact (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id         UUID NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    contact_user_id UUID NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, contact_user_id)
);

CREATE INDEX IF NOT EXISTS idx_contact_user_id ON contact(user_id);
CREATE INDEX IF NOT EXISTS idx_contact_contact_user_id ON contact(contact_user_id);

-- ============================================================
-- BLOCK
-- ============================================================
CREATE TABLE IF NOT EXISTS block (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    blocker_id      UUID NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    blocked_id      UUID NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(blocker_id, blocked_id)
);

CREATE INDEX IF NOT EXISTS idx_block_blocker_id ON block(blocker_id);
CREATE INDEX IF NOT EXISTS idx_block_blocked_id ON block(blocked_id);

-- ============================================================
-- CONVERSATION
-- ============================================================
CREATE TABLE IF NOT EXISTS conversation (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    title           VARCHAR(255),
    is_group        BOOLEAN NOT NULL DEFAULT FALSE,
    created_by      UUID REFERENCES "user"(id) ON DELETE SET NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================
-- CONVERSATION_PARTICIPANT
-- ============================================================
CREATE TABLE IF NOT EXISTS conversation_participant (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    conversation_id UUID NOT NULL REFERENCES conversation(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    role            VARCHAR(20) NOT NULL DEFAULT 'member' CHECK (role IN ('owner', 'admin', 'member')),
    joined_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(conversation_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_cp_conversation_id ON conversation_participant(conversation_id);
CREATE INDEX IF NOT EXISTS idx_cp_user_id ON conversation_participant(user_id);

-- ============================================================
-- CONVERSATION_SETTING
-- ============================================================
CREATE TABLE IF NOT EXISTS conversation_setting (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id         UUID NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    conversation_id UUID NOT NULL REFERENCES conversation(id) ON DELETE CASCADE,
    is_muted        BOOLEAN NOT NULL DEFAULT FALSE,
    is_pinned       BOOLEAN NOT NULL DEFAULT FALSE,
    is_archived     BOOLEAN NOT NULL DEFAULT FALSE,
    last_read_at    TIMESTAMPTZ,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, conversation_id)
);

CREATE INDEX IF NOT EXISTS idx_cs_user_id ON conversation_setting(user_id);
CREATE INDEX IF NOT EXISTS idx_cs_conversation_id ON conversation_setting(conversation_id);

-- ============================================================
-- ATTACHMENT
-- ============================================================
CREATE TABLE IF NOT EXISTS attachment (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    message_id      UUID NOT NULL,
    uploader_id     UUID NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    file_url        TEXT NOT NULL,
    file_name       VARCHAR(255) NOT NULL,
    mime_type       VARCHAR(127) NOT NULL,
    file_size       BIGINT NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_attachment_message_id ON attachment(message_id);
CREATE INDEX IF NOT EXISTS idx_attachment_uploader_id ON attachment(uploader_id);
