use axum::{
    extract::State,
    http::HeaderMap,
    response::{IntoResponse, Json},
};
use serde_json::json;
use crate::AppState;

/// NIP-11 relay information document.
pub async fn relay_info_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
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
