use anyhow::{anyhow, Result};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct TenantContext {
    pub community_id: Uuid,
    pub actor_pubkey: String,
}

/// Resolve tenant from channel_id (if present) or HTTP host header.
/// Fail-closed: returns Err if host is unmapped or host/channel community disagree.
pub async fn resolve_tenant(
    channel_id: Option<Uuid>,
    host: Option<&str>,
    pool: &PgPool,
) -> Result<Uuid> {
    match (channel_id, host) {
        (Some(cid), Some(h)) => {
            let channel_community: Option<Uuid> = sqlx::query_scalar(
                "SELECT community_id FROM channels WHERE id = $1"
            )
            .bind(cid)
            .fetch_optional(pool)
            .await?;

            let channel_community = channel_community
                .ok_or_else(|| anyhow!("channel not found"))?;

            let host_community: Option<Uuid> = sqlx::query_scalar(
                "SELECT community_id FROM host_community_map WHERE host = $1"
            )
            .bind(h)
            .fetch_optional(pool)
            .await?;

            let host_community = host_community
                .ok_or_else(|| anyhow!("host not mapped to any community"))?;

            if channel_community != host_community {
                return Err(anyhow!("host/channel community mismatch - fail closed"));
            }

            Ok(channel_community)
        }
        (None, Some(h)) => {
            let community: Option<Uuid> = sqlx::query_scalar(
                "SELECT community_id FROM host_community_map WHERE host = $1"
            )
            .bind(h)
            .fetch_optional(pool)
            .await?;

            community.ok_or_else(|| anyhow!("host not mapped to any community"))
        }
        (Some(cid), None) => {
            let community: Option<Uuid> = sqlx::query_scalar(
                "SELECT community_id FROM channels WHERE id = $1"
            )
            .bind(cid)
            .fetch_optional(pool)
            .await?;

            community.ok_or_else(|| anyhow!("channel not found"))
        }
        (None, None) => Err(anyhow!("cannot resolve tenant: no channel_id or host")),
    }
}

/// Set PostgreSQL session-local RLS context variable.
pub async fn set_tenant_context(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    community_id: Uuid,
) -> Result<()> {
    sqlx::query(&format!(
        "SET LOCAL app.community_id = '{}'",
        community_id
    ))
    .execute(&mut **tx)
    .await?;
    Ok(())
}
