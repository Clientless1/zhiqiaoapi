ALTER TABLE user_coupon_campaign ADD COLUMN is_public BOOLEAN NOT NULL DEFAULT FALSE;
CREATE INDEX idx_coupon_public_available ON user_coupon_campaign (is_public,enabled,expires_at);
