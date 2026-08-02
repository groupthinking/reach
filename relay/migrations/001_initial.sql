-- Enable pgcrypto for gen_random_uuid()
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- Communities (no RLS — admin-level table)
CREATE TABLE IF NOT EXISTS communities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    signing_key BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Host → community mapping
CREATE TABLE IF NOT EXISTS host_community_map (
    host TEXT PRIMARY KEY,
    community_id UUID NOT NULL REFERENCES communities(id)
);

-- Channels
CREATE TABLE IF NOT EXISTS channels (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    community_id UUID NOT NULL REFERENCES communities(id),
    name TEXT NOT NULL,
    visibility TEXT NOT NULL DEFAULT 'public',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Prevent community_id mutation on channels
CREATE OR REPLACE FUNCTION prevent_channel_community_update()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.community_id <> OLD.community_id THEN
        RAISE EXCEPTION 'community_id is immutable on channels';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER channels_immutable_community
    BEFORE UPDATE ON channels
    FOR EACH ROW EXECUTE FUNCTION prevent_channel_community_update();

-- Channel members
CREATE TABLE IF NOT EXISTS channel_members (
    community_id UUID NOT NULL,
    channel_id UUID NOT NULL,
    pubkey TEXT NOT NULL,
    joined_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (community_id, channel_id, pubkey)
);

-- Messages (append-only, time-partitioned primary key)
CREATE TABLE IF NOT EXISTS messages (
    community_id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    id TEXT NOT NULL,
    channel_id UUID,
    pubkey TEXT NOT NULL,
    kind INTEGER NOT NULL,
    content TEXT NOT NULL,
    tags JSONB,
    sig TEXT NOT NULL,
    PRIMARY KEY (community_id, created_at, id),
    UNIQUE (community_id, id)
);

-- API tokens
CREATE TABLE IF NOT EXISTS api_tokens (
    community_id UUID NOT NULL,
    token_hash TEXT NOT NULL,
    pubkey TEXT NOT NULL,
    channel_ids JSONB,
    scopes JSONB,
    created_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,
    PRIMARY KEY (community_id, token_hash)
);

-- Audit log (per-community hash chain)
CREATE TABLE IF NOT EXISTS audit_log (
    community_id UUID NOT NULL,
    seq BIGINT NOT NULL,
    prev_hash TEXT NOT NULL,
    entry_hash TEXT NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (community_id, seq)
);

-- Admitted members
CREATE TABLE IF NOT EXISTS admitted_members (
    community_id UUID NOT NULL,
    pubkey TEXT NOT NULL,
    admitted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (community_id, pubkey)
);

-- Enable RLS on all tenant-bearing tables
ALTER TABLE channels ENABLE ROW LEVEL SECURITY;
ALTER TABLE channels FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON channels
    USING (community_id = current_setting('app.community_id')::uuid);

ALTER TABLE channel_members ENABLE ROW LEVEL SECURITY;
ALTER TABLE channel_members FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON channel_members
    USING (community_id = current_setting('app.community_id')::uuid);

ALTER TABLE messages ENABLE ROW LEVEL SECURITY;
ALTER TABLE messages FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON messages
    USING (community_id = current_setting('app.community_id')::uuid);

ALTER TABLE api_tokens ENABLE ROW LEVEL SECURITY;
ALTER TABLE api_tokens FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON api_tokens
    USING (community_id = current_setting('app.community_id')::uuid);

ALTER TABLE audit_log ENABLE ROW LEVEL SECURITY;
ALTER TABLE audit_log FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON audit_log
    USING (community_id = current_setting('app.community_id')::uuid);

ALTER TABLE admitted_members ENABLE ROW LEVEL SECURITY;
ALTER TABLE admitted_members FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON admitted_members
    USING (community_id = current_setting('app.community_id')::uuid);
