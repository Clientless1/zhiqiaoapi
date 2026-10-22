ALTER TABLE "user" ADD COLUMN auth_version BIGINT NOT NULL DEFAULT 0 CHECK (auth_version >= 0);

-- Every password write, including administrator resets, revokes old sessions.
CREATE FUNCTION bump_user_auth_version() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.password_hash IS DISTINCT FROM OLD.password_hash THEN
        NEW.auth_version := OLD.auth_version + 1;
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER user_password_revokes_sessions
    BEFORE UPDATE OF password_hash ON "user"
    FOR EACH ROW EXECUTE FUNCTION bump_user_auth_version();

CREATE TABLE user_password_reset (
    token_hash TEXT PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    auth_version BIGINT NOT NULL CHECK (auth_version >= 0),
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_password_reset_user ON user_password_reset(user_id);
CREATE INDEX idx_password_reset_expiry ON user_password_reset(expires_at);
