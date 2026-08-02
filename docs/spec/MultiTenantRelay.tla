---- MODULE MultiTenantRelay ----
(* TLA+ isolation model stub for the Buzz multi-tenant relay. *)

EXTENDS Naturals, Sequences, FiniteSets

CONSTANTS Communities, Pubkeys, EventIds

VARIABLES
    messages,
    audit_chains,
    rls_context,
    authed_pubkeys

NI == \A c1, c2 \in Communities :
    c1 # c2 => messages[c1] \cap messages[c2] = {}

I1 == \A c \in Communities, msg \in messages[c] :
    msg.community_id = c

I2 == \A c \in Communities, msg \in messages[c] :
    msg.sig_valid = TRUE

I3 == \A c \in Communities, msg \in messages[c] :
    \E entry \in Range(audit_chains[c]) : entry.event_id = msg.id

I4 == \A c \in Communities :
    \A i \in 2..Len(audit_chains[c]) :
        audit_chains[c][i].prev_hash = audit_chains[c][i-1].entry_hash

I5 == rls_context \in Communities

Safety == NI /\ I1 /\ I2 /\ I3 /\ I4 /\ I5

====
