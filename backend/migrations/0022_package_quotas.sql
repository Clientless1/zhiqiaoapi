-- Legacy access plans keep their existing subscriptions and wallet billing.
ALTER TABLE user_package
    ADD COLUMN billing_mode TEXT NOT NULL DEFAULT 'metered' CHECK (billing_mode IN ('metered','quota')),
    ADD COLUMN total_quota_micros BIGINT CHECK (total_quota_micros BETWEEN 1 AND 9000000000000000),
    ADD COLUMN daily_quota_micros BIGINT CHECK (daily_quota_micros BETWEEN 1 AND 9000000000000000),
    ADD COLUMN weekly_quota_micros BIGINT CHECK (weekly_quota_micros BETWEEN 1 AND 9000000000000000),
    ADD CONSTRAINT package_quota_required CHECK (billing_mode <> 'quota' OR
        total_quota_micros IS NOT NULL OR daily_quota_micros IS NOT NULL OR weekly_quota_micros IS NOT NULL);

-- Every purchase is independent: quotas stack, expiry never extends another grant.
CREATE TABLE user_package_grant (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    package_id BIGINT NOT NULL REFERENCES user_package(id),
    order_id UUID UNIQUE REFERENCES user_package_order(id),
    redemption_id BIGINT UNIQUE REFERENCES user_redemption_code(id),
    package_name TEXT NOT NULL,
    total_quota_micros BIGINT CHECK (total_quota_micros > 0),
    daily_quota_micros BIGINT CHECK (daily_quota_micros > 0),
    weekly_quota_micros BIGINT CHECK (weekly_quota_micros > 0),
    starts_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    CHECK (expires_at > starts_at),
    CHECK (total_quota_micros IS NOT NULL OR daily_quota_micros IS NOT NULL OR weekly_quota_micros IS NOT NULL)
);
ALTER TABLE credit_account DROP CONSTRAINT credit_account_owner_type_check;
ALTER TABLE credit_account ADD CONSTRAINT credit_account_owner_type_check
    CHECK (owner_type IN ('project','user_key','user_key_model','package_grant'));
CREATE INDEX idx_package_grant_user_active ON user_package_grant(user_id,package_id,expires_at);
CREATE INDEX idx_package_grant_expiry ON user_package_grant(package_id,expires_at);
CREATE INDEX idx_credit_allocation_package_period ON credit_allocation(credit_account_id,created_at) INCLUDE(consumed_micros,amount_micros,returned_micros,status);
