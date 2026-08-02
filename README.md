# Reach — Audio-Aware Multi-Tenant Community Platform

Reach is a polyglot monorepo for a production-grade, audio-aware multi-tenant community platform. It integrates four tightly coupled services: a Buzz multi-tenant Nostr relay (Rust), an NVIDIA Audio Flamingo inference service (Python), a Google CES/CX Agent Studio web widget (TypeScript), and an API orchestration gateway (TypeScript).

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        User / Browser                           │
└───────────────────────────┬─────────────────────────────────────┘
                            │ HTTPS / WebSocket
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│              widget/ — Google CES Web Widget (TS)               │
│         chat | voice | mixed modality (VITE_MODALITY)           │
└───────────────────────────┬─────────────────────────────────────┘
                            │ REST + WS
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│           api/ — Orchestration Gateway (TypeScript)             │
│   /api/chat  /api/audio  /api/token  /health                    │
└──────────┬──────────────────────────┬───────────────────────────┘
           │ NIP-01 WebSocket         │ HTTP multipart
           ▼                          ▼
┌──────────────────────┐   ┌──────────────────────────────────────┐
│  relay/ — Buzz Relay │   │  audio/ — Audio Flamingo Service     │
│  (Rust / Axum)       │   │  (Python / FastAPI)                  │
│  NIP-01/42/98        │   │  AF3 | AF2 | Music Flamingo          │
│  PostgreSQL + RLS    │   │  NVIDIA CUDA inference               │
└──────────────────────┘   └──────────────────────────────────────┘
           │
           ▼
┌──────────────────────┐
│  PostgreSQL 16       │
│  Multi-tenant RLS    │
│  Audit chains        │
└──────────────────────┘
```

## Quick Start

```bash
cp .env.example .env
# Edit .env with your credentials
docker-compose up
```

## Services

| Directory | Language   | Port | Description                                      |
|-----------|------------|------|--------------------------------------------------|
| relay/    | Rust       | 8080 | Buzz multi-tenant Nostr relay (NIP-01/42/98)     |
| audio/    | Python     | 8000 | NVIDIA Audio Flamingo inference service          |
| widget/   | TypeScript | 5173 | Google CES/CX Agent Studio web widget            |
| api/      | TypeScript | 3000 | API orchestration gateway                        |

## Documentation

- [Architecture](docs/architecture.md)
- [Formal Spec Notes](docs/spec/SPEC_NOTES.md)

## License

MIT
