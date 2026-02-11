-- Make sure this is in migrations/XXXXXX_create_invitations_table.sql
CREATE TABLE
    IF NOT EXISTS invitations (
        token VARCHAR(255) PRIMARY KEY,
        created_by UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
        expires_at TIMESTAMPTZ NOT NULL,
        used_by UUID REFERENCES users (id) ON DELETE SET NULL,
        used_at TIMESTAMPTZ,
        created_at TIMESTAMPTZ DEFAULT NOW (),
        CONSTRAINT valid_expiry CHECK (expires_at > created_at)
    );

CREATE INDEX idx_invitations_created_by ON invitations (created_by);

CREATE INDEX idx_invitations_expires_at ON invitations (expires_at);