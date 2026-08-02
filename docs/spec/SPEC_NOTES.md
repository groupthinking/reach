# Buzz Multi-Tenant Relay - Formal Verification Notes

This document describes the formal verification approach for the Reach relay's multi-tenant isolation model.

## Axioms

### A-RLS-1..5 - Row-Level Security Axioms
- **A-RLS-1**: RLS is ENABLED and FORCED on all tenant-bearing tables. The relay DB role is NOBYPASSRLS and non-owner.
- **A-RLS-2**: The `tenant_isolation` policy is the sole access predicate: `community_id = current_setting('app.community_id')::uuid`.
- **A-RLS-3**: `SET LOCAL app.community_id` is called inside every transaction, before any DML or SELECT on tenant tables.
- **A-RLS-4**: All unique constraints and foreign keys include `community_id` as the leading column.
- **A-RLS-5**: `channels.community_id` is immutable after insert, enforced by a BEFORE UPDATE trigger.

### P-RESOLVE - Tenant Resolution Protocol
1. If channel_id is present, resolve community from `channels.community_id`.
2. If host is present, resolve community from `host_community_map.community_id`.
3. If both are present, they MUST agree. Mismatch -> `Err`.
4. If neither is present -> `Err`.

### P-SIG - Signature Verification Protocol
1. Recompute event id from canonical NIP-01 serialization.
2. Verify id matches the event's `id` field.
3. Verify Schnorr signature over the id using the event's `pubkey`.

### A_HASH - Audit Chain Hash Invariant
```
entry_hash[n] = sha256(entry_hash[n-1] || seq[n] || payload[n])
```

### P3 - NIP-98 Replay Protection
1. `created_at` must be within +/-60s of server time.
2. The event `id` is inserted into a moka cache (capacity 10,000, TTL 120s) on first use.
3. Subsequent requests with the same `id` within the TTL window are rejected.

## 13 Mutation Tests

1. **MT-01**: Insert message with wrong `community_id` -> rejected by RLS.
2. **MT-02**: Query messages without `SET LOCAL app.community_id` -> empty result.
3. **MT-03**: Update `channels.community_id` -> rejected by trigger.
4. **MT-04**: Submit EVENT with invalid Schnorr signature -> `RelayError::Invalid`.
5. **MT-05**: Submit EVENT with mismatched event id -> `RelayError::Invalid`.
6. **MT-06**: Replay NIP-98 token within TTL -> `RelayError::Duplicate`.
7. **MT-07**: Submit NIP-98 token with `created_at` > 60s in the past -> `RelayError::Invalid`.
8. **MT-08**: Submit EVENT with channel_id from a different community than host -> `RelayError::Restricted`.
9. **MT-09**: Submit EVENT with no channel_id and unmapped host -> `RelayError::Restricted`.
10. **MT-10**: Audit chain: insert entry with wrong `prev_hash` -> chain integrity violation.
11. **MT-11**: NIP-42 AUTH with wrong challenge -> `RelayError::Invalid`.
12. **MT-12**: NIP-42 AUTH with `created_at` > 60s drift -> `RelayError::Invalid`.
13. **MT-13**: Duplicate message -> silently ignored (`ON CONFLICT DO NOTHING`), `OK true` returned.
