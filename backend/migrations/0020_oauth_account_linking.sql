ALTER TABLE auth_oauth_flow DROP CONSTRAINT auth_oauth_flow_intent_check;
ALTER TABLE auth_oauth_flow ADD CONSTRAINT auth_oauth_flow_intent_check
    CHECK (intent IN ('login','register','link'));
ALTER TABLE auth_oauth_flow
    ADD COLUMN link_user_id BIGINT REFERENCES "user"(id) ON DELETE CASCADE,
    ADD COLUMN link_auth_version BIGINT,
    ADD CONSTRAINT oauth_link_requires_session CHECK (
        (intent = 'link' AND link_user_id IS NOT NULL AND link_auth_version IS NOT NULL)
        OR (intent <> 'link' AND link_user_id IS NULL AND link_auth_version IS NULL)
    );
