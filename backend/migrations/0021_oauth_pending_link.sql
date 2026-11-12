-- A provider with an existing local email still requires proof of that local
-- account. Keep the verified identity briefly, bound to the originating browser.
CREATE TABLE auth_oauth_pending_link (
    ticket_hash TEXT PRIMARY KEY,
    browser_hash TEXT NOT NULL,
    provider TEXT NOT NULL REFERENCES auth_oauth_provider(provider),
    subject TEXT NOT NULL,
    email TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_oauth_pending_link_expiry ON auth_oauth_pending_link(expires_at);
