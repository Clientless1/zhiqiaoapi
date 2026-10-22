CREATE TABLE billing_hold (
    transaction_id UUID PRIMARY KEY,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE billing_hold_allocation (
    transaction_id UUID NOT NULL REFERENCES billing_hold(transaction_id) ON DELETE CASCADE,
    allocation_id BIGINT NOT NULL REFERENCES credit_allocation(id),
    amount_micros BIGINT NOT NULL CHECK (amount_micros > 0),
    PRIMARY KEY (transaction_id, allocation_id)
);
CREATE INDEX idx_billing_hold_allocation ON billing_hold_allocation(allocation_id);
CREATE INDEX idx_billing_hold_expiry ON billing_hold(expires_at);

ALTER TABLE usage DROP CONSTRAINT usage_billing_status_check;
ALTER TABLE usage ADD CONSTRAINT usage_billing_status_check
    CHECK (billing_status IN ('not_billed', 'billed', 'usage_missing', 'undercharged', 'payment_pending'));

-- Keep late usage collectible even if recovered credit has since been spent.
CREATE TABLE billing_receivable (
    id BIGSERIAL PRIMARY KEY,
    transaction_id UUID NOT NULL,
    usage_id BIGINT REFERENCES usage(id) ON DELETE SET NULL,
    allocation_id BIGINT NOT NULL REFERENCES credit_allocation(id),
    credit_account_id BIGINT NOT NULL REFERENCES credit_account(id),
    amount_micros BIGINT NOT NULL CHECK (amount_micros > 0),
    billing_status TEXT NOT NULL CHECK (billing_status IN ('billed', 'usage_missing', 'undercharged')),
    settled_at TIMESTAMPTZ,
    retry_after TIMESTAMPTZ NOT NULL DEFAULT now(),
    attempts BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (transaction_id, allocation_id)
);
CREATE INDEX idx_billing_receivable_pending ON billing_receivable(retry_after, id)
    WHERE settled_at IS NULL;
CREATE INDEX idx_billing_receivable_usage ON billing_receivable(usage_id)
    WHERE settled_at IS NULL;
CREATE INDEX idx_billing_receivable_account ON billing_receivable(credit_account_id)
    WHERE settled_at IS NULL;
