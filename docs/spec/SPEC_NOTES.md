# Buzz Multi-Tenant Relay — Formal Verification Notes

This document describes the formal verification approach for the Reach relay's multi-tenant isolation model.

## Axioms

### A-RLS-1..5 — Row-Level Security Axioms
- **A-RLS-1**: RLS is ENABLED and FORCED on all tenant-bearing tables. The relay DB role is NOBYPASSRLS and non-owner. No query can bypass the policy.
- **A-RLS-2**: The `tenant_isolation` policy is the sole access predicate: `community_id = current_setting('app.community_id')::uuid`. No other policy exists on these tables.
- **A-RLS-3**: `SET LOCAL app.community_id` is called inside every transaction, before any DML or SELECT on tenant tables. The LOCAL scope ensures the setting is transaction-scoped and cannot leak across connections.
- **A-RLS-4**: All unique constraints and foreign keys include `community_id` as the leading column, preventing cross-tenant key collisions.
- **A-RLS-5**: `channels.community_id` is immutable after insert, enforced by a BEFORE UPDATE trigger. This prevents channel re-assignment attacks.

### P-RESOLVE — Tenant Resolution Protocol
The `resolve_tenant` function is fail-closed:
1. If channel_id is present, resolve community from `channels.community_id`.
2. If host is present, resolve community from `host_community_map.community_id`.
3. If both are present, they MUST agree. Mismatch → `Err` (no fallback, no default).
4. If neither is present → `Err`.

This ensures no event can be accepted without a verified community binding.

### P-SIG — Signature Verification Protocol
Every EVENT must pass Schnorr/BIP-340 signature verification before insertion:
1. Recompute event id from canonical NIP-01 serialization.
2. Verify id matches the event's `id` field.
3. Verify Schnorr signature over the id using the event's `pubkey`.

Failure at any step → `RelayError::Invalid` (sanitized, no internal detail leaked).

### A_HASH — Audit Chain Hash Invariant
For each community, the audit chain satisfies:
```
entry_hash[n] = sha256(entry_hash[n-1] || seq[n] || payload[n])
```
The genesis entry uses `prev_hash = "000...0"` (64 hex zeros). Chains are independent per community; no cross-community references exist.

### P3 — NIP-98 Replay Protection
NIP-98 tokens are single-use within a 120-second window:
1. `created_at` must be within ±60s of server time.
2. The event `id` is inserted into a moka cache (capacity 10,000, TTL 120s) on first use.
3. Subsequent requests with the same `id` within the TTL window are rejected with `RelayError::Duplicate`.

## TLA+ Isolation Model (MultiTenantRelay.tla)

See `MultiTenantRelay.tla` for the formal TLA+ model. Key invariants:

- **NI (Non-Interference)**: For any two communities C1 ≠ C2, no message inserted under C1 is visible under C2.
- **I1 (Tenant Binding)**: Every accepted message has a verified community_id binding via P-RESOLVE.
- **I2 (Signature Validity)**: Every accepted message has a valid Schnorr signature per P-SIG.
- **I3 (Audit Completeness)**: Every accepted message has a corresponding audit entry in the community's chain.
- **I4 (Chain Integrity)**: The audit chain hash invariant A_HASH holds for all entries.
- **I5 (RLS Enforcement)**: No query on a tenant table executes without a valid `app.community_id` session variable.

## Tamarin Authorization Model (MultiTenantAuth.spthy)

See `MultiTenantAuth.spthy` for the Tamarin model. Key lemmas:

- **S1**: An adversary cannot forge a valid NIP-01 event without the private key.
- **S2**: An adversary cannot replay a NIP-98 token after its TTL window.
- **S3**: An adversary cannot access community C2's messages by authenticating to community C1.
- **S4**: Channel re-assignment is impossible (A-RLS-5 trigger).
- **S5**: The audit chain cannot be forked without detection.
- **S6**: NIP-42 challenge-response binds pubkey to connection; a MITM cannot substitute a different pubkey.
- **S7**: Host/channel community mismatch is always rejected (P-RESOLVE fail-closed).
- **S8**: The relay never leaks internal error details (Σ_err sanitization).

## 13 Mutation Tests

The following mutation tests verify the isolation model:

1. **MT-01**: Insert message with wrong `community_id` → rejected by RLS.
2. **MT-02**: Query messages without `SET LOCAL app.community_id` → empty result (RLS blocks all rows).
3. **MT-03**: Update `channels.community_id` → rejected by trigger.
4. **MT-04**: Submit EVENT with invalid Schnorr signature → `RelayError::Invalid`.
5. **MT-05**: Submit EVENT with mismatched event id → `RelayError::Invalid`.
6. **MT-06**: Replay NIP-98 token within TTL → `RelayError::Duplicate`.
7. **MT-07**: Submit NIP-98 token with `created_at` > 60s in the past → `RelayError::Invalid`.
8. **MT-08**: Submit EVENT with channel_id from a different community than host → `RelayError::Restricted`.
9. **MT-09**: Submit EVENT with no channel_id and unmapped host → `RelayError::Restricted`.
10. **MT-10**: Audit chain: insert entry with wrong `prev_hash` → chain integrity violation detected.
11. **MT-11**: NIP-42 AUTH with wrong challenge → `RelayError::Invalid`.
12. **MT-12**: NIP-42 AUTH with `created_at` > 60s drift → `RelayError::Invalid`.
13. **MT-13**: Duplicate message (same `community_id`, `created_at`, `id`) → silently ignored (`ON CONFLICT DO NOTHING`), `OK true` returned.
