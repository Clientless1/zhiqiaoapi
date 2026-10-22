-- Project integrations keep their existing project allowlist. Personal keys
-- must explicitly choose a package; preserve secrets/usage and allow rebinding.
CREATE TABLE user_package (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    description TEXT NOT NULL DEFAULT '',
    family TEXT NOT NULL DEFAULT 'custom',
    models TEXT[] NOT NULL DEFAULT '{}',
    price_micros BIGINT NOT NULL DEFAULT 0 CHECK (price_micros >= 0),
    duration_days INTEGER NOT NULL DEFAULT 30 CHECK (duration_days BETWEEN 1 AND 3660),
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
INSERT INTO user_package (name, description, family) VALUES
('ChatGPT 系列', '按实际调用量计费，仅限所选 ChatGPT 系列模型。', 'chatgpt'),
('Claude 系列', '按实际调用量计费，仅限所选 Claude 系列模型。', 'claude'),
('Gemini 系列', '按实际调用量计费，仅限所选 Gemini 系列模型。', 'gemini'),
('DeepSeek 系列', '按实际调用量计费，仅限所选 DeepSeek 系列模型。', 'deepseek'),
('通义千问系列', '按实际调用量计费，仅限所选通义千问系列模型。', 'qwen'),
('Kimi 系列', '按实际调用量计费，仅限所选 Kimi 系列模型。', 'kimi'),
('GLM 系列', '按实际调用量计费，仅限所选 GLM 系列模型。', 'glm'),
('豆包系列', '按实际调用量计费，仅限所选豆包系列模型。', 'doubao'),
('图像模型', '按实际调用量计费，仅限所选图像模型。', 'image'),
('视频模型', '按实际调用量计费，仅限所选视频模型。', 'video'),
('语音模型', '按实际调用量计费，仅限所选语音与 TTS 模型。', 'audio');

CREATE TABLE user_package_subscription (
    user_id BIGINT NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    package_id BIGINT NOT NULL REFERENCES user_package(id),
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, package_id)
);
ALTER TABLE user_key ADD COLUMN package_id BIGINT REFERENCES user_package(id);
ALTER TABLE user_key ADD COLUMN model_limits TEXT[];
UPDATE user_key uk SET model_limits = '{}'
FROM project p, "user" u
WHERE p.id = uk.project_id AND u.id = uk.user_id AND p.is_default
  AND u.email::TEXT NOT IN ('system-apps@zhiqiaoapi.local', 'system-apps@neogate.local');

CREATE TABLE user_coupon_campaign (
    id BIGSERIAL PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    amount_micros BIGINT NOT NULL CHECK (amount_micros > 0),
    min_spend_micros BIGINT NOT NULL DEFAULT 0 CHECK (min_spend_micros >= 0),
    max_claims INTEGER NOT NULL DEFAULT 1 CHECK (max_claims > 0),
    claimed_count INTEGER NOT NULL DEFAULT 0 CHECK (claimed_count >= 0 AND claimed_count <= max_claims),
    expires_at TIMESTAMPTZ,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE user_coupon (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    campaign_id BIGINT NOT NULL REFERENCES user_coupon_campaign(id),
    used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, campaign_id)
);
CREATE TABLE user_package_order (
    id UUID PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    request_id UUID NOT NULL,
    package_id BIGINT NOT NULL REFERENCES user_package(id),
    package_name TEXT NOT NULL,
    amount_micros BIGINT NOT NULL CHECK (amount_micros >= 0),
    discount_micros BIGINT NOT NULL DEFAULT 0 CHECK (discount_micros >= 0),
    coupon_id BIGINT UNIQUE REFERENCES user_coupon(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, request_id)
);
CREATE TABLE user_redemption_code (
    id BIGSERIAL PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    amount_micros BIGINT NOT NULL DEFAULT 0 CHECK (amount_micros >= 0),
    package_id BIGINT REFERENCES user_package(id),
    expires_at TIMESTAMPTZ,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    redeemed_by BIGINT REFERENCES "user"(id) ON DELETE SET NULL,
    redeemed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (amount_micros > 0 OR package_id IS NOT NULL)
);
CREATE TABLE user_notification (
    id BIGSERIAL PRIMARY KEY,
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    content TEXT NOT NULL,
    user_id BIGINT REFERENCES "user"(id) ON DELETE CASCADE,
    published BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE user_notification_read (
    user_id BIGINT NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    notification_id BIGINT NOT NULL REFERENCES user_notification(id) ON DELETE CASCADE,
    read_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, notification_id)
);
CREATE INDEX idx_user_package_order_user ON user_package_order(user_id, created_at DESC);
CREATE INDEX idx_user_notification_target ON user_notification(user_id, created_at DESC) WHERE published;
CREATE INDEX idx_user_redemption_user ON user_redemption_code(redeemed_by, redeemed_at DESC) WHERE redeemed_at IS NOT NULL;
