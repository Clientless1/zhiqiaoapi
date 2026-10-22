-- Selling a package and serving an existing entitlement are separate actions.
ALTER TABLE user_package ADD COLUMN service_enabled BOOLEAN NOT NULL DEFAULT TRUE;
ALTER TABLE user_package_subscription ADD COLUMN requires_payment BOOLEAN NOT NULL DEFAULT FALSE;
UPDATE user_package_subscription s SET requires_payment = TRUE
WHERE EXISTS (SELECT 1 FROM user_package p WHERE p.id=s.package_id AND p.price_micros > 0)
   OR EXISTS (SELECT 1 FROM user_package_order o WHERE o.user_id=s.user_id
              AND o.package_id=s.package_id AND o.amount_micros > 0);

ALTER TABLE payment ADD COLUMN expires_at TIMESTAMPTZ NOT NULL DEFAULT (now() + interval '30 minutes');
ALTER TABLE payment ADD COLUMN canceled_at TIMESTAMPTZ;
UPDATE payment SET expires_at=created_at + interval '30 minutes';
UPDATE payment SET status='expired', updated_at=now()
WHERE status='pending' AND expires_at <= now();
CREATE INDEX idx_payment_pending_expiry ON payment(expires_at) WHERE status='pending';
CREATE INDEX idx_payment_user_created_id ON payment(user_id,created_at DESC,id DESC);
CREATE INDEX idx_notification_read_user ON user_notification_read(user_id,notification_id);
CREATE INDEX idx_package_subscription_active ON user_package_subscription(package_id,expires_at)
    WHERE requires_payment;

-- Credit accounts outlive deleted API keys. Keep their owner and label so
-- deleting a key cannot erase the user's financial history from the UI.
ALTER TABLE credit_account ADD COLUMN owner_user_id BIGINT REFERENCES "user"(id) ON DELETE SET NULL;
ALTER TABLE credit_account ADD COLUMN owner_name TEXT NOT NULL DEFAULT '';
ALTER TABLE credit_account ADD COLUMN owner_model TEXT;
UPDATE credit_account w SET owner_user_id=p.owner_user_id,owner_name=p.name
FROM project p WHERE w.owner_type='project' AND w.owner_id=p.id;
UPDATE credit_account w SET owner_user_id=k.user_id,owner_name=k.name
FROM user_key k WHERE w.owner_type='user_key' AND w.owner_id=k.id;
UPDATE credit_account w SET owner_user_id=k.user_id,owner_name=k.name,owner_model=km.model
FROM user_key_model km JOIN user_key k ON k.id=km.user_key_id
WHERE w.owner_type='user_key_model' AND w.owner_id=km.id;
UPDATE credit_account w SET owner_user_id=recovered.user_id,owner_name='历史密钥'
FROM (
    SELECT l.credit_account_id,min(u.user_id) AS user_id
    FROM credit_ledger l JOIN usage u ON u.id=l.usage_id
    WHERE u.user_id IS NOT NULL GROUP BY l.credit_account_id
    HAVING count(DISTINCT u.user_id)=1
) recovered WHERE w.id=recovered.credit_account_id AND w.owner_user_id IS NULL;
CREATE INDEX idx_credit_account_user_owner ON credit_account(owner_user_id,id);

CREATE FUNCTION snapshot_credit_account_owner() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    account_user BIGINT;
    account_label TEXT;
    account_model TEXT;
BEGIN
    IF NEW.owner_type='project' THEN
        SELECT p.owner_user_id,p.name INTO account_user,account_label FROM project p WHERE p.id=NEW.owner_id;
    ELSIF NEW.owner_type='user_key' THEN
        SELECT k.user_id,k.name INTO account_user,account_label FROM user_key k WHERE k.id=NEW.owner_id;
    ELSIF NEW.owner_type='user_key_model' THEN
        SELECT k.user_id,k.name,km.model INTO account_user,account_label,account_model
        FROM user_key_model km JOIN user_key k ON k.id=km.user_key_id WHERE km.id=NEW.owner_id;
    END IF;
    NEW.owner_user_id=COALESCE(account_user,NEW.owner_user_id);
    NEW.owner_name=COALESCE(account_label,NEW.owner_name);
    NEW.owner_model=COALESCE(account_model,NEW.owner_model);
    RETURN NEW;
END;
$$;
CREATE TRIGGER credit_account_owner_snapshot BEFORE INSERT OR UPDATE OF owner_type,owner_id
ON credit_account FOR EACH ROW EXECUTE FUNCTION snapshot_credit_account_owner();
