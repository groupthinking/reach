# Reach — Architecture

## System Overview

Reach is a polyglot monorepo integrating four services into a cohesive audio-aware multi-tenant community platform.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          User / Browser                                 │
└──────────────────────────────┬──────────────────────────────────────────┘
                               │ HTTPS / WebSocket
                               ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                  widget/ — Google CES Web Widget                        │
│              TypeScript + Vite  |  Port 5173                            │
│   ┌─────────────────────────────────────────────────────────────────┐   │
│   │  chat-messenger (Google CES SDK)                                │   │
│   │  Modality: chat | voice | mixed (VITE_MODALITY)                 │   │
│   │  relay-bridge.ts ──── NIP-01 WebSocket ──────────────────────► │   │
│   └─────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────┬──────────────────────────────────────────┘
                               │ REST (POST /api/chat, /api/audio, GET /api/token)
                               ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                  api/ — Orchestration Gateway                           │
│              TypeScript + Fastify  |  Port 3000                         │
│   /api/chat ──► relay-client.ts ──► Buzz Relay (NIP-01 EVENT)          │
│   /api/audio ──► audio-client.ts ──► Audio Flamingo /infer             │
│   /api/token ──► ces-client.ts ──► Google CES generateChatToken        │
└──────────┬──────────────────────────────┬───────────────────────────────┘
           │ NIP-01 WebSocket             │ HTTP multipart
           ▼                              ▼
┌──────────────────────┐      ┌───────────────────────────────────────────┐
│  relay/ — Buzz Relay │      │  audio/ — Audio Flamingo Service          │
│  Rust + Axum         │      │  Python + FastAPI  |  Port 8000           │
│  Port 8080           │      │  NVIDIA CUDA inference                    │
│  NIP-01/42/98        │      │  AF3 | AF2 | Music Flamingo               │
│  PostgreSQL + RLS    │      └───────────────────────────────────────────┘
└──────────┬───────────┘
           │ sqlx
           ▼
┌──────────────────────┐
│  PostgreSQL 16       │
│  Multi-tenant RLS    │
│  Per-community audit │
└──────────────────────┘
```

## Data Flow

1. **User → CES Widget**: User sends a message or audio via the Google CES chat-messenger component.
2. **Widget → API Gateway**: The widget posts to `/api/chat` (text) or `/api/audio` (audio) on the Fastify gateway.
3. **API Gateway → Buzz Relay**: For chat messages, the gateway constructs a NIP-01 EVENT, signs it with the community keypair, and sends it to the Buzz relay over WebSocket.
4. **API Gateway → Audio Flamingo**: For audio uploads, the gateway forwards the audio buffer to the Audio Flamingo `/infer` endpoint and returns the transcript/understanding to the widget.
5. **Buzz Relay → PostgreSQL**: The relay validates the event (NIP-42 auth, Schnorr sig), resolves the tenant (community_id via channel or host), sets the RLS context (`SET LOCAL app.community_id`), and inserts into `messages` with `ON CONFLICT DO NOTHING`.
6. **Relay → Audit Chain**: Every accepted event appends a hash-chained entry to `audit_log` for the community.
7. **Relay → Widget (relay-bridge)**: The widget's `relay-bridge.ts` maintains a NIP-01 REQ subscription; incoming EVENTs are surfaced in the CES widget via `renderCustomText` or `renderCustomCard`.
8. **Audio Flamingo → Relay (webhook)**: Optionally, the audio service posts inference results back to the relay channel via `RELAY_WEBHOOK_URL`.

## Multi-Tenant Isolation Model

### community_id Scoping
Every tenant-bearing table (`messages`, `channels`, `channel_members`, `api_tokens`, `audit_log`, `admitted_members`) carries a `community_id` column. All queries are scoped to a single community.

### RLS Axioms (A-RLS-1..5)
- **A-RLS-1**: RLS is ENABLED and FORCED on all tenant tables. The relay DB role is NOBYPASSRLS and non-owner.
- **A-RLS-2**: The `tenant_isolation` policy enforces `community_id = current_setting('app.community_id')::uuid` on every row access.
- **A-RLS-3**: `SET LOCAL app.community_id` is called inside every transaction before any tenant query.
- **A-RLS-4**: All unique and FK constraints include `community_id` to prevent cross-tenant collisions.
- **A-RLS-5**: The `channels.community_id` column is immutable (enforced by trigger).

### ResolveTenant (P-RESOLVE)
The `resolve_tenant` function in `tenant.rs` is fail-closed:
- If a channel_id is present, it looks up `channels.community_id`.
- If a host header is present, it looks up `host_community_map.community_id`.
- If both are present, they must agree — mismatch returns `Err` (no fallback).
- If neither is present, returns `Err`.

### NIP-42 / NIP-98 Auth
- **NIP-42**: WebSocket AUTH challenge/response. The relay sends a random challenge on connect; the client must respond with a signed kind-22242 event. Pubkey is bound to the connection.
- **NIP-98**: Bearer token for REST endpoints. Verifies `created_at` within ±60s, deduplicates via moka seen-set (capacity 10,000, TTL 120s), verifies Schnorr signature.

### Audit Chains
Each community has an independent hash chain in `audit_log`. Every accepted event appends `entry_hash = sha256(prev_hash || seq || payload)`. Chains are non-cross-community (N independent chains).

## Audio Flamingo Model Selection

| Backend | HuggingFace Repo | Description |
|---------|-----------------|-------------|
| `af3` | `nvidia/audio-flamingo-3-hf` | 7B model, sound+music+speech, up to 10min audio |
| `af2` | `nvidia/audio-flamingo-2` | 3B model, long-audio up to 5min |
| `music` | `nvidia/music-flamingo-hf` | AF3 backbone, music/song understanding |

Set `MODEL_BACKEND` in `audio/.env` to select the backend. The model is loaded once at startup and cached.

## CES Widget Modality Config

| `VITE_MODALITY` | Behavior |
|----------------|----------|
| `chat` | Text-only, no audio input |
| `voice` | Audio-input-only (`enable-audio-input-only`) |
| `mixed` | Both text and audio (`enable-audio-input`) — default |

## Environment Variable Reference

### relay/
| Variable | Description |
|----------|-------------|
| `DATABASE_URL` | PostgreSQL connection string |
| `RELAY_NAME` | NIP-11 relay name |
| `RELAY_DESCRIPTION` | NIP-11 relay description |
| `RELAY_PUBKEY` | NIP-11 relay pubkey |
| `RELAY_CONTACT` | NIP-11 contact |
| `RUST_LOG` | Log level (e.g., `info`) |

### audio/
| Variable | Description |
|----------|-------------|
| `MODEL_BACKEND` | `af3` \| `af2` \| `music` |
| `HF_TOKEN` | HuggingFace access token |
| `RELAY_WEBHOOK_URL` | URL to post inference results back to relay |
| `RELAY_COMMUNITY_ID` | Community ID for webhook posts |
| `RELAY_CHANNEL_ID` | Channel ID for webhook posts |

### api/
| Variable | Description |
|----------|-------------|
| `PORT` | Gateway port (default 3000) |
| `RELAY_URL` | Buzz relay WebSocket URL |
| `AUDIO_SERVICE_URL` | Audio Flamingo HTTP URL |
| `CES_DEPLOYMENT_NAME` | Full CES deployment resource name |
| `GOOGLE_APPLICATION_CREDENTIALS` | Path to service account JSON |
| `RELAY_COMMUNITY_ID` | Default community ID |
| `RELAY_CHANNEL_ID` | Default channel ID |
| `RELAY_KEYPAIR` | JSON `{privateKeyHex, publicKeyHex}` |

### widget/
| Variable | Description |
|----------|-------------|
| `VITE_CES_DEPLOYMENT_NAME` | CES deployment resource name |
| `VITE_MODALITY` | `chat` \| `voice` \| `mixed` |
| `VITE_API_GATEWAY_URL` | API gateway base URL |

## Local Development

```bash
# 1. Copy and configure environment
cp .env.example .env
# Edit .env with your HF_TOKEN, CES credentials, etc.

# 2. Start all services
docker-compose up

# Services will be available at:
# Relay:  http://localhost:8080  (NIP-11 info)
# Audio:  http://localhost:8000/health
# API:    http://localhost:3000/health
# Widget: http://localhost:5173
```

## Production Deployment Notes

### GPU Requirements (audio/)
The Audio Flamingo service requires an NVIDIA GPU with CUDA 12.1+ and at least:
- AF3 (7B): 16GB VRAM minimum, 24GB recommended
- AF2 (3B): 8GB VRAM minimum
- Music Flamingo: 16GB VRAM minimum

The `docker-compose.yml` includes NVIDIA device reservation. Ensure `nvidia-container-toolkit` is installed on the host.

### HA Relay (P3 / NIP-98 Replay Protection)
For high-availability relay deployments:
- The moka seen-set for NIP-98 replay protection is in-process. In HA mode, replace with a shared Redis cache.
- The audit chain uses sequential `seq` per community; in HA mode, use `SELECT ... FOR UPDATE` or advisory locks to prevent seq gaps.
- PostgreSQL should be deployed with streaming replication and a connection pooler (PgBouncer).

### RLS DB Role
The relay's PostgreSQL role must be created as:
```sql
CREATE ROLE reach_relay LOGIN PASSWORD '...' NOBYPASSRLS;
GRANT SELECT, INSERT ON messages, channels, channel_members, api_tokens, audit_log, admitted_members TO reach_relay;
```
