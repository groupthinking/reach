use crate::{
    errors::RelayError,
    models::NostrEvent,
    AppState,
};
use anyhow::Result;
use secp256k1::{schnorr::Signature, XOnlyPublicKey, SECP256K1};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

/// Verify a Schnorr/BIP-340 signature on a Nostr event.
/// Returns Ok(()) if valid, Err(RelayError::Invalid) otherwise.
pub fn verify_event_signature(event: &NostrEvent) -> Result<(), RelayError> {
    let computed_id = event.compute_id();
    if computed_id != event.id {
        tracing::debug!("event id mismatch: computed={} got={}", computed_id, event.id);
        return Err(RelayError::Invalid);
    }

    let pubkey_bytes = hex::decode(&event.pubkey).map_err(|_| RelayError::Invalid)?;
    let sig_bytes = hex::decode(&event.sig).map_err(|_| RelayError::Invalid)?;
    let id_bytes = hex::decode(&event.id).map_err(|_| RelayError::Invalid)?;

    let xonly = XOnlyPublicKey::from_slice(&pubkey_bytes).map_err(|_| RelayError::Invalid)?;
    let sig = Signature::from_slice(&sig_bytes).map_err(|_| RelayError::Invalid)?;
    let msg = secp256k1::Message::from_digest_slice(&id_bytes).map_err(|_| RelayError::Invalid)?;

    SECP256K1
        .verify_schnorr(&sig, &msg, &xonly)
        .map_err(|_| RelayError::Invalid)
}

/// NIP-42 AUTH challenge verification.
/// Verifies the AUTH event kind (22242), checks created_at within ±60s,
/// and validates the relay URL and challenge tags.
pub fn verify_nip42_auth(
    event: &NostrEvent,
    expected_challenge: &str,
    relay_url: &str,
) -> Result<(), RelayError> {
    if event.kind != 22242 {
        return Err(RelayError::Invalid);
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    if (event.created_at - now).abs() > 60 {
        return Err(RelayError::Invalid);
    }

    // Check relay and challenge tags
    let mut has_relay = false;
    let mut has_challenge = false;
    for tag in &event.tags {
        if tag.len() >= 2 {
            if tag[0] == "relay" && tag[1] == relay_url {
                has_relay = true;
            }
            if tag[0] == "challenge" && tag[1] == expected_challenge {
                has_challenge = true;
            }
        }
    }

    if !has_relay || !has_challenge {
        return Err(RelayError::Invalid);
    }

    verify_event_signature(event)
}

/// NIP-98 Bearer token verification.
/// Checks created_at within ±60s, deduplicates via moka seen-set,
/// verifies signature.
pub async fn verify_nip98_token(
    event: &NostrEvent,
    state: &AppState,
    url: &str,
    method: &str,
) -> Result<(), RelayError> {
    if event.kind != 27235 {
        return Err(RelayError::Invalid);
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    if (event.created_at - now).abs() > 60 {
        return Err(RelayError::Invalid);
    }

    // Replay protection via moka seen-set
    if state.nip98_seen.get(&event.id).await.is_some() {
        return Err(RelayError::Duplicate);
    }
    state.nip98_seen.insert(event.id.clone(), ()).await;

    // Verify u (URL) and method tags
    let mut has_url = false;
    let mut has_method = false;
    for tag in &event.tags {
        if tag.len() >= 2 {
            if tag[0] == "u" && tag[1] == url {
                has_url = true;
            }
            if tag[0] == "method" && tag[1].to_uppercase() == method.to_uppercase() {
                has_method = true;
            }
        }
    }

    if !has_url || !has_method {
        return Err(RelayError::Invalid);
    }

    verify_event_signature(event)
}
