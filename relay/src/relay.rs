use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use serde_json::json;
use uuid::Uuid;

use crate::{
    audit,
    auth::verify_event_signature,
    errors::RelayError,
    models::{Filter, NostrEvent},
    tenant::{resolve_tenant, set_tenant_context},
    AppState,
};

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    let challenge = Uuid::new_v4().to_string();
    let auth_msg = json!(["AUTH", challenge]).to_string();
    if sender.send(Message::Text(auth_msg)).await.is_err() {
        return;
    }

    let mut authed_pubkey: Option<String> = None;

    while let Some(Ok(msg)) = receiver.next().await {
        let text = match msg {
            Message::Text(t) => t,
            Message::Close(_) => break,
            _ => continue,
        };

        if text.len() > 512 * 1024 {
            let _ = sender
                .send(Message::Text(json!(["NOTICE", "frame-too-large"]).to_string()))
                .await;
            break;
        }

        let parsed: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => {
                let _ = sender.send(Message::Text(json!(["NOTICE", "invalid"]).to_string())).await;
                continue;
            }
        };

        let arr = match parsed.as_array() {
            Some(a) if !a.is_empty() => a,
            _ => continue,
        };

        match arr[0].as_str().unwrap_or("") {
            "AUTH" => {
                if arr.len() < 2 { continue; }
                let event: NostrEvent = match serde_json::from_value(arr[1].clone()) {
                    Ok(e) => e,
                    Err(_) => {
                        let _ = sender.send(Message::Text(json!(["NOTICE", "invalid"]).to_string())).await;
                        continue;
                    }
                };
                match crate::auth::verify_nip42_auth(&event, &challenge, "wss://relay.reach.local") {
                    Ok(()) => {
                        authed_pubkey = Some(event.pubkey.clone());
                        let _ = sender.send(Message::Text(json!(["OK", event.id, true, ""]).to_string())).await;
                    }
                    Err(e) => {
                        let _ = sender.send(Message::Text(json!(["OK", event.id, false, e.prefix()]).to_string())).await;
                    }
                }
            }
            "EVENT" => {
                let pubkey = match &authed_pubkey {
                    Some(p) => p.clone(),
                    None => {
                        let _ = sender.send(Message::Text(json!(["NOTICE", "auth-required"]).to_string())).await;
                        continue;
                    }
                };
                if arr.len() < 2 { continue; }
                let event: NostrEvent = match serde_json::from_value(arr[1].clone()) {
                    Ok(e) => e,
                    Err(_) => {
                        let _ = sender.send(Message::Text(json!(["NOTICE", "invalid"]).to_string())).await;
                        continue;
                    }
                };
                if let Err(e) = verify_event_signature(&event) {
                    let _ = sender.send(Message::Text(json!(["OK", event.id, false, e.prefix()]).to_string())).await;
                    continue;
                }
                let channel_id = event.channel_id();
                let community_id = match resolve_tenant(channel_id, None, &state.pool).await {
                    Ok(id) => id,
                    Err(_) => {
                        let _ = sender.send(Message::Text(json!(["OK", event.id, false, "restricted"]).to_string())).await;
                        continue;
                    }
                };
                let result = accept_event(&state, community_id, channel_id, &event, &pubkey).await;
                match result {
                    Ok(()) => { let _ = sender.send(Message::Text(json!(["OK", event.id, true, ""]).to_string())).await; }
                    Err(e) => {
                        tracing::error!("accept_event error: {:?}", e);
                        let _ = sender.send(Message::Text(json!(["OK", event.id, false, "error"]).to_string())).await;
                    }
                }
            }
            "REQ" => {
                if arr.len() < 3 { continue; }
                let sub_id = arr[1].as_str().unwrap_or("").to_string();
                let filter: Filter = match serde_json::from_value(arr[2].clone()) {
                    Ok(f) => f,
                    Err(_) => {
                        let _ = sender.send(Message::Text(json!(["NOTICE", "invalid"]).to_string())).await;
                        continue;
                    }
                };
                let channel_id = filter.e_tags.as_ref().and_then(|tags| tags.first()).and_then(|t| Uuid::parse_str(t).ok());
                let community_id = match resolve_tenant(channel_id, None, &state.pool).await {
                    Ok(id) => id,
                    Err(_) => {
                        let _ = sender.send(Message::Text(json!(["EOSE", sub_id]).to_string())).await;
                        continue;
                    }
                };
                match serve_events(&state, community_id, &filter).await {
                    Ok(events) => {
                        for ev in events {
                            let _ = sender.send(Message::Text(json!(["EVENT", sub_id, ev]).to_string())).await;
                        }
                    }
                    Err(e) => { tracing::error!("serve_events error: {:?}", e); }
                }
                let _ = sender.send(Message::Text(json!(["EOSE", sub_id]).to_string())).await;
            }
            "CLOSE" => {}
            _ => { let _ = sender.send(Message::Text(json!(["NOTICE", "invalid"]).to_string())).await; }
        }
    }
}

async fn accept_event(
    state: &AppState,
    community_id: Uuid,
    channel_id: Option<Uuid>,
    event: &NostrEvent,
    pubkey: &str,
) -> anyhow::Result<()> {
    let mut tx = state.pool.begin().await?;
    set_tenant_context(&mut tx, community_id).await?;

    let created_at = chrono::DateTime::from_timestamp(event.created_at, 0)
        .unwrap_or_else(chrono::Utc::now);

    sqlx::query(
        "INSERT INTO messages (community_id, created_at, id, channel_id, pubkey, kind, content, tags, sig) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) ON CONFLICT (community_id, created_at, id) DO NOTHING",
    )
    .bind(community_id).bind(created_at).bind(&event.id).bind(channel_id)
    .bind(pubkey).bind(event.kind as i32).bind(&event.content)
    .bind(serde_json::to_value(&event.tags).ok()).bind(&event.sig)
    .execute(&mut *tx).await?;

    tx.commit().await?;

    let payload = serde_json::json!({
        "type": "event_accepted",
        "event_id": event.id,
        "pubkey": pubkey,
        "kind": event.kind,
    });
    audit::append_entry(community_id, payload, &state.pool).await?;
    Ok(())
}

async fn serve_events(
    state: &AppState,
    community_id: Uuid,
    filter: &Filter,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let mut tx = state.pool.begin().await?;
    set_tenant_context(&mut tx, community_id).await?;

    let limit = filter.limit.unwrap_or(100).min(500) as i64;

    let rows: Vec<crate::models::Message> = sqlx::query_as(
        "SELECT community_id, created_at, id, channel_id, pubkey, kind, content, tags, sig \
         FROM messages WHERE community_id = $1 ORDER BY created_at DESC LIMIT $2",
    )
    .bind(community_id).bind(limit)
    .fetch_all(&mut *tx).await?;

    tx.commit().await?;

    let events = rows.into_iter().map(|m| serde_json::json!({
        "id": m.id, "pubkey": m.pubkey,
        "created_at": m.created_at.timestamp(),
        "kind": m.kind, "content": m.content,
        "tags": m.tags.unwrap_or(serde_json::json!([])),
        "sig": m.sig,
    })).collect();

    Ok(events)
}
