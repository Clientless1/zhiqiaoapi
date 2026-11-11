CREATE TABLE auth_oauth_provider (
    provider TEXT PRIMARY KEY CHECK (provider IN ('github','google')),
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    client_id TEXT NOT NULL DEFAULT '',
    client_secret_ciphertext TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
INSERT INTO auth_oauth_provider(provider) VALUES ('github'),('google');
CREATE TABLE user_oauth_identity (
    provider TEXT NOT NULL REFERENCES auth_oauth_provider(provider),
    subject TEXT NOT NULL,
    user_id BIGINT NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (provider,subject),
    UNIQUE (provider,user_id)
);
CREATE TABLE auth_oauth_flow (
    state_hash TEXT PRIMARY KEY,
    browser_hash TEXT NOT NULL,
    provider TEXT NOT NULL REFERENCES auth_oauth_provider(provider),
    verifier_ciphertext TEXT NOT NULL,
    intent TEXT NOT NULL CHECK (intent IN ('login','register')),
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_oauth_flow_expiry ON auth_oauth_flow(expires_at);
CREATE TABLE auth_oauth_ticket (
    ticket_hash TEXT PRIMARY KEY,
    browser_hash TEXT NOT NULL,
    user_id BIGINT NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    auth_version BIGINT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX idx_oauth_ticket_expiry ON auth_oauth_ticket(expires_at);
CREATE TABLE user_agreement_acceptance (
    user_id BIGINT NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    version TEXT NOT NULL,
    method TEXT NOT NULL CHECK (method IN ('email','github','google')),
    accepted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id,version)
);
