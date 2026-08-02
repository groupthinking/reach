use anyhow::Result;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

/// Append an entry to the per-community audit hash chain.
/// Chain invariant: entry_hash = sha256(prev_hash || seq || payload_bytes)
pub async fn append_entry(
    community_id: Uuid,
    payload: serde_json::Value,
    pool: &PgPool,
) -> Result<()> {
    let mut tx = pool.begin().await?;

    sqlx::query(&format!("SET LOCAL app.community_id = '{}'", community_id))
        .execute(&mut *tx)
        .await?;

    let latest: Option<(i64, String)> = sqlx::query_as(
        "SELECT seq, entry_hash FROM audit_log \
         WHERE community_id = $1 ORDER BY seq DESC LIMIT 1",
    )
    .bind(community_id)
    .fetch_optional(&mut *tx)
    .await?;

    let (prev_seq, prev_hash) = match latest {
        Some((seq, hash)) => (seq, hash),
        None => (0i64, "0000000000000000000000000000000000000000000000000000000000000000".to_string()),
    };

    let new_seq = prev_seq + 1;
    let payload_bytes = serde_json::to_vec(&payload)?;

    let mut hasher = Sha256::new();
    hasher.update(prev_hash.as_bytes());
    hasher.update(new_seq.to_be_bytes());
    hasher.update(&payload_bytes);
    let new_hash = hex::encode(hasher.finalize());

    sqlx::query(
        "INSERT INTO audit_log (community_id, seq, prev_hash, entry_hash, payload) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(community_id)
    .bind(new_seq)
    .bind(&prev_hash)
    .bind(&new_hash)
    .bind(&payload)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}
