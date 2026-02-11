CREATE TABLE
    IF NOT EXISTS messages (
        id UUID PRIMARY KEY DEFAULT gen_random_uuid (),
        user_id UUID REFERENCES users (id) ON DELETE SET NULL,
        content TEXT NOT NULL,
        created_at TIMESTAMPTZ DEFAULT NOW (),
        edited_at TIMESTAMPTZ,
        reply_to UUID REFERENCES messages (id) ON DELETE SET NULL,
        CONSTRAINT content_not_empty CHECK (char_length(content) > 0)
    );

CREATE INDEX idx_messages_created_at ON messages (created_at DESC);

CREATE INDEX idx_messages_user_id ON messages (user_id);

CREATE INDEX idx_messages_reply_to ON messages (reply_to);