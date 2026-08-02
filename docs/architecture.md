# Reach - Architecture

## System Overview

Reach is a polyglot monorepo integrating four services into a cohesive audio-aware multi-tenant community platform.

## Services

| Directory | Language   | Port | Description                                      |
|-----------|------------|------|--------------------------------------------------|
| relay/    | Rust       | 8080 | Buzz multi-tenant Nostr relay (NIP-01/42/98)     |
| audio/    | Python     | 8000 | NVIDIA Audio Flamingo inference service          |
| widget/   | TypeScript | 5173 | Google CES/CX Agent Studio web widget            |
| api/      | TypeScript | 3000 | API orchestration gateway                        |

## Data Flow

1. **User -> CES Widget**: User sends a message or audio via the Google CES chat-messenger component.
2. **Widget -> API Gateway**: The widget posts to `/api/chat` (text) or `/api/audio` (audio) on the Fastify gateway.
3. **API Gateway -> Buzz Relay**: For chat messages, the gateway constructs a NIP-01 EVENT and sends it to the Buzz relay over WebSocket.
4. **API Gateway -> Audio Flamingo**: For audio uploads, the gateway forwards the audio buffer to the Audio Flamingo `/infer` endpoint.
5. **Buzz Relay -> PostgreSQL**: The relay validates the event (NIP-42 auth, Schnorr sig), resolves the tenant, sets the RLS context, and inserts into `messages`.
6. **Relay -> Audit Chain**: Every accepted event appends a hash-chained entry to `audit_log`.
7. **Relay -> Widget (relay-bridge)**: The widget's `relay-bridge.ts` maintains a NIP-01 REQ subscription.
8. **Audio Flamingo -> Relay (webhook)**: Optionally, the audio service posts inference results back to the relay channel.

## Multi-Tenant Isolation Model

### RLS Axioms (A-RLS-1..5)
- **A-RLS-1**: RLS is ENABLED and FORCED on all tenant tables.
- **A-RLS-2**: The `tenant_isolation` policy enforces `community_id = current_setting('app.community_id')::uuid`.
- **A-RLS-3**: `SET LOCAL app.community_id` is called inside every transaction before any tenant query.
- **A-RLS-4**: All unique and FK constraints include `community_id`.
- **A-RLS-5**: The `channels.community_id` column is immutable (enforced by trigger).

### ResolveTenant (P-RESOLVE)
The `resolve_tenant` function in `tenant.rs` is fail-closed:
- If a channel_id is present, it looks up `channels.community_id`.
- If a host header is present, it looks up `host_community_map.community_id`.
- If both are present, they must agree - mismatch returns `Err`.
- If neither is present, returns `Err`.

## Audio Flamingo Model Selection

| Backend | HuggingFace Repo | Description |
|---------|-----------------|-------------|
| `af3` | `nvidia/audio-flamingo-3-hf` | 7B model, sound+music+speech, up to 10min audio |
| `af2` | `nvidia/audio-flamingo-2` | 3B model, long-audio up to 5min |
| `music` | `nvidia/music-flamingo-hf` | AF3 backbone, music/song understanding |

## Local Development

```bash
cp .env.example .env
docker-compose up
```

Services:
- Relay:  http://localhost:8080
- Audio:  http://localhost:8000/health
- API:    http://localhost:3000/health
- Widget: http://localhost:5173
