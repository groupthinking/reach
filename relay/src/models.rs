use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Community {
    pub id: Uuid,
    pub name: String,
    pub signing_key: Vec<u8>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Channel {
    pub id: Uuid,
    pub community_id: Uuid,
    pub name: String,
    pub visibility: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Message {
    pub community_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub id: String,
    pub channel_id: Option<Uuid>,
    pub pubkey: String,
    pub kind: i32,
    pub content: String,
    pub tags: Option<serde_json::Value>,
    pub sig: String,
}

/// NIP-01 client message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ClientMessage {
    Event(Vec<serde_json::Value>),
    Req(Vec<serde_json::Value>),
    Close(Vec<serde_json::Value>),
    Auth(Vec<serde_json::Value>),
}

/// NIP-01 event structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NostrEvent {
    pub id: String,
    pub pubkey: String,
    pub created_at: i64,
    pub kind: u32,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
}

impl NostrEvent {
    /// Compute the canonical event id per NIP-01
    pub fn compute_id(&self) -> String {
        use sha2::{Sha256, Digest};
        let serialized = serde_json::json!([
            0,
            self.pubkey,
            self.created_at,
            self.kind,
            self.tags,
            self.content
        ]);
        let bytes = serde_json::to_vec(&serialized).unwrap();
        let hash = Sha256::digest(&bytes);
        hex::encode(hash)
    }

    /// Extract channel_id from tags ("e" tag with relay hint)
    pub fn channel_id(&self) -> Option<uuid::Uuid> {
        for tag in &self.tags {
            if tag.len() >= 2 && tag[0] == "e" {
                if let Ok(id) = uuid::Uuid::parse_str(&tag[1]) {
                    return Some(id);
                }
            }
        }
        None
    }
}

/// NIP-01 filter
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Filter {
    pub ids: Option<Vec<String>>,
    pub authors: Option<Vec<String>>,
    pub kinds: Option<Vec<u32>>,
    #[serde(rename = "#e")]
    pub e_tags: Option<Vec<String>>,
    #[serde(rename = "#p")]
    pub p_tags: Option<Vec<String>>,
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub limit: Option<u32>,
}
