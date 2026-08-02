---- MODULE MultiTenantRelay ----
(* TLA+ isolation model stub for the Buzz multi-tenant relay.
   Full model-checking requires TLC or Apalache.
   This stub documents the invariants and state space. *)

EXTENDS Naturals, Sequences, FiniteSets

CONSTANTS Communities, Pubkeys, EventIds

VARIABLES
    messages,        (* messages[c] = set of events for community c *)
    audit_chains,    (* audit_chains[c] = sequence of audit entries *)
    rls_context,     (* rls_context = current community_id in session *)
    authed_pubkeys   (* authed_pubkeys[conn] = pubkey bound to connection *)

(* --- Invariant NI: Non-Interference ---
   For any two distinct communities C1, C2:
   messages[C1] ∩ messages[C2] = ∅
   No message inserted under C1 is visible under C2. *)
NI == \A c1, c2 \in Communities :
    c1 # c2 => messages[c1] \cap messages[c2] = {}

(* --- Invariant I1: Tenant Binding ---
   Every accepted message has a verified community_id binding via P-RESOLVE.
   msg.community_id is set before insertion and matches the RLS context. *)
I1 == \A c \in Communities, msg \in messages[c] :
    msg.community_id = c

(* --- Invariant I2: Signature Validity ---
   Every accepted message has a valid Schnorr signature per P-SIG.
   Modeled as: msg.sig_valid = TRUE *)
I2 == \A c \in Communities, msg \in messages[c] :
    msg.sig_valid = TRUE

(* --- Invariant I3: Audit Completeness ---
   Every accepted message has a corresponding audit entry. *)
I3 == \A c \in Communities, msg \in messages[c] :
    \E entry \in Range(audit_chains[c]) : entry.event_id = msg.id

(* --- Invariant I4: Chain Integrity (A_HASH) ---
   The audit chain hash invariant holds for all entries:
   entry_hash[n] = sha256(entry_hash[n-1] || seq[n] || payload[n]) *)
I4 == \A c \in Communities :
    \A i \in 2..Len(audit_chains[c]) :
        audit_chains[c][i].prev_hash = audit_chains[c][i-1].entry_hash

(* --- Invariant I5: RLS Enforcement ---
   No query on a tenant table executes without a valid app.community_id. *)
I5 == rls_context \in Communities

(* Combined safety property *)
Safety == NI /\ I1 /\ I2 /\ I3 /\ I4 /\ I5

====
