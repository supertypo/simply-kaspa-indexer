use crate::models::query::database_details::DatabaseDetails;
use crate::models::query::table_details::TableDetails;
use crate::models::subnetwork::Subnetwork;
use crate::models::types::hash::Hash;
use sqlx::{Error, Pool, Postgres, Row};

pub async fn select_database_details(pool: &Pool<Postgres>) -> Result<DatabaseDetails, Error> {
    sqlx::query_as::<_, DatabaseDetails>(
        "
        SELECT 
            current_database() database_name,
            current_schema() schema_name,
            pg_database_size(current_database()) database_size,
            (SELECT count(*) FROM pg_stat_activity WHERE state = 'active' AND pid <> pg_backend_pid()) active_queries,
            (SELECT count(*) FROM pg_locks WHERE NOT granted) blocked_queries,
            (SELECT count(*) FROM pg_stat_activity) active_connections,
            (SELECT setting::int FROM pg_settings WHERE name = 'max_connections') max_connections;
    ",
    )
    .fetch_one(pool)
    .await
}

pub async fn select_all_table_details(pool: &Pool<Postgres>) -> Result<Vec<TableDetails>, Error> {
    sqlx::query_as::<_, TableDetails>(
        "
        SELECT
            cls.relname name,
            pg_total_relation_size(cls.relname::text) total_size,
            pg_indexes_size(cls.relname::text) indexes_size,
            cls.reltuples::bigint approximate_row_count
        FROM pg_class cls
        JOIN pg_namespace nsp ON cls.relnamespace = nsp.oid
        WHERE nsp.nspname = current_schema()
        AND cls.relkind = 'r'
        ORDER BY cls.relname
    ",
    )
    .fetch_all(pool)
    .await
}

pub async fn select_var(key: &str, pool: &Pool<Postgres>) -> Result<String, Error> {
    sqlx::query("SELECT value FROM vars WHERE key = $1").bind(key).fetch_one(pool).await?.try_get(0)
}

pub async fn select_subnetworks(pool: &Pool<Postgres>) -> Result<Vec<Subnetwork>, Error> {
    let rows = sqlx::query("SELECT id, subnetwork_id FROM subnetworks").fetch_all(pool).await?;
    let subnetworks = rows.into_iter().map(|row| Subnetwork { id: row.get("id"), subnetwork_id: row.get("subnetwork_id") }).collect();
    Ok(subnetworks)
}

pub async fn select_tx_count(block_hash: &Hash, pool: &Pool<Postgres>) -> Result<i64, Error> {
    sqlx::query("SELECT COUNT(*) FROM blocks_transactions WHERE block_hash = $1").bind(block_hash).fetch_one(pool).await?.try_get(0)
}

pub async fn select_is_chain_block(block_hash: &Hash, pool: &Pool<Postgres>) -> Result<bool, Error> {
    sqlx::query("SELECT EXISTS(SELECT 1 FROM transactions_acceptances WHERE block_hash = $1)")
        .bind(block_hash)
        .fetch_one(pool)
        .await?
        .try_get(0)
}
