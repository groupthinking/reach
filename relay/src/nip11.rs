use axum::{
    extract::State,
    http::HeaderMap,
    response::{IntoResponse, Json},
};
use serde_json::json;
use crate::AppState;

/// NIP-11 relay information document.
/// Returns static config only — no DB access, no tenant context.
/// Per spec C2.4: the NIP-11 handler must not hold a DB handle.
pub async fn relay_info_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Only serve NIP-11 JSON when Accept: application/nostr+json is present
    let accept = headers
        .get("accept")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if accept.contains("application/nostr+json") {
        let info = build_relay_info(&state.config);
        return Json(info).into_response();
    }

    Json(build_relay_info(&state.config)).into_response()
}

pub fn build_relay_info(config: &crate::Config) -> serde_json::Value {
    json!({
        "name": config.relay_name,
        "description": config.relay_description,
        "pubkey": config.relay_pubkey,
        "contact": config.relay_contact,
        "supported_nips": [1, 42, 98],
        "software": config.software,
        "version": config.version,
        "limitation": {
            "auth_required": true,
            "payment_required": false,
            "restricted_writes": true
        }
    })
}
