use crate::models::query::api::{
    ApiAddressBalance, ApiAddressTransaction, ApiAddressUtxo, ApiBlock, ApiSearchResult, ApiToccataCovenantMetrics,
    ApiToccataLaneMetrics, ApiToccataMetrics, ApiTransaction,
};
use crate::models::query::database_details::DatabaseDetails;
use crate::models::query::table_details::TableDetails;
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

pub async fn select_recent_blocks(limit: i64, pool: &Pool<Postgres>) -> Result<Vec<ApiBlock>, Error> {
    sqlx::query(
        "
        SELECT
            b.hash,
            b.selected_parent_hash,
            b.blue_score,
            b.daa_score,
            b.timestamp,
            b.version,
            COUNT(bt.transaction_id)::BIGINT AS transaction_count,
            EXISTS(SELECT 1 FROM transactions_acceptances ta WHERE ta.block_hash = b.hash) AS is_chain_block
        FROM blocks b
        LEFT JOIN blocks_transactions bt ON bt.block_hash = b.hash
        GROUP BY b.hash
        ORDER BY b.blue_score DESC NULLS LAST, b.timestamp DESC NULLS LAST
        LIMIT $1
        ",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(row_to_api_block)
    .collect()
}

pub async fn select_block(hash: &Hash, pool: &Pool<Postgres>) -> Result<Option<ApiBlock>, Error> {
    sqlx::query(
        "
        SELECT
            b.hash,
            b.selected_parent_hash,
            b.blue_score,
            b.daa_score,
            b.timestamp,
            b.version,
            COUNT(bt.transaction_id)::BIGINT AS transaction_count,
            EXISTS(SELECT 1 FROM transactions_acceptances ta WHERE ta.block_hash = b.hash) AS is_chain_block
        FROM blocks b
        LEFT JOIN blocks_transactions bt ON bt.block_hash = b.hash
        WHERE b.hash = $1
        GROUP BY b.hash
        ",
    )
    .bind(hash)
    .fetch_optional(pool)
    .await?
    .map(row_to_api_block)
    .transpose()
}

pub async fn select_transaction(transaction_id: &Hash, pool: &Pool<Postgres>) -> Result<Option<ApiTransaction>, Error> {
    sqlx::query(
        "
        SELECT
            t.transaction_id,
            t.hash,
            t.block_time,
            t.version,
            t.mass,
            COALESCE(array_agg(bt.block_hash) FILTER (WHERE bt.block_hash IS NOT NULL), ARRAY[]::BYTEA[]) AS block_hashes,
            ta.block_hash AS accepted_block_hash
        FROM transactions t
        LEFT JOIN blocks_transactions bt ON bt.transaction_id = t.transaction_id
        LEFT JOIN transactions_acceptances ta ON ta.transaction_id = t.transaction_id
        WHERE t.transaction_id = $1
        GROUP BY t.transaction_id, ta.block_hash
        ",
    )
    .bind(transaction_id)
    .fetch_optional(pool)
    .await?
    .map(row_to_api_transaction)
    .transpose()
}

pub async fn select_address_transactions(
    address: &str,
    limit: i64,
    pool: &Pool<Postgres>,
) -> Result<Vec<ApiAddressTransaction>, Error> {
    sqlx::query(
        "
        SELECT address, transaction_id, block_time
        FROM addresses_transactions
        WHERE address = $1
        ORDER BY block_time DESC
        LIMIT $2
        ",
    )
    .bind(address)
    .bind(limit)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| {
        Ok(ApiAddressTransaction {
            address: row.try_get("address")?,
            transaction_id: row.try_get("transaction_id")?,
            block_time: row.try_get("block_time")?,
        })
    })
    .collect()
}

pub async fn select_address_utxos(address: &str, limit: i64, pool: &Pool<Postgres>) -> Result<Vec<ApiAddressUtxo>, Error> {
    sqlx::query(
        "
        WITH address_txs AS (
            SELECT DISTINCT transaction_id
            FROM addresses_transactions
            WHERE address = $1
        ),
        candidate_outputs AS (
            SELECT
                t.transaction_id,
                (o).index AS output_index,
                (o).amount AS amount_sompi,
                t.block_time
            FROM address_txs at
            JOIN transactions t ON t.transaction_id = at.transaction_id
            CROSS JOIN LATERAL unnest(t.outputs) AS o
            WHERE (o).script_public_key_address = $1
        )
        SELECT
            co.transaction_id,
            co.output_index,
            co.amount_sompi,
            co.block_time
        FROM candidate_outputs co
        WHERE NOT EXISTS (
            SELECT 1
            FROM address_txs at_spend
            JOIN transactions spending ON spending.transaction_id = at_spend.transaction_id
            CROSS JOIN LATERAL unnest(spending.inputs) AS i
            WHERE (i).previous_outpoint_hash = co.transaction_id
              AND (i).previous_outpoint_index = co.output_index
        )
        ORDER BY co.block_time DESC NULLS LAST, co.output_index ASC
        LIMIT $2
        ",
    )
    .bind(address)
    .bind(limit)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| {
        Ok(ApiAddressUtxo {
            transaction_id: row.try_get("transaction_id")?,
            output_index: row.try_get("output_index")?,
            amount_sompi: row.try_get("amount_sompi")?,
            block_time: row.try_get("block_time")?,
        })
    })
    .collect()
}

pub async fn select_address_balance(address: &str, pool: &Pool<Postgres>) -> Result<ApiAddressBalance, Error> {
    let row = sqlx::query(
        "
        WITH address_txs AS (
            SELECT DISTINCT transaction_id
            FROM addresses_transactions
            WHERE address = $1
        ),
        candidate_outputs AS (
            SELECT
                t.transaction_id,
                (o).index AS output_index,
                (o).amount AS amount_sompi
            FROM address_txs at
            JOIN transactions t ON t.transaction_id = at.transaction_id
            CROSS JOIN LATERAL unnest(t.outputs) AS o
            WHERE (o).script_public_key_address = $1
        )
        SELECT
            COALESCE(SUM(co.amount_sompi), 0)::BIGINT AS balance_sompi,
            COUNT(*)::BIGINT AS utxo_count
        FROM candidate_outputs co
        WHERE NOT EXISTS (
            SELECT 1
            FROM address_txs at_spend
            JOIN transactions spending ON spending.transaction_id = at_spend.transaction_id
            CROSS JOIN LATERAL unnest(spending.inputs) AS i
            WHERE (i).previous_outpoint_hash = co.transaction_id
              AND (i).previous_outpoint_index = co.output_index
        )
        ",
    )
    .bind(address)
    .fetch_one(pool)
    .await?;

    Ok(ApiAddressBalance {
        address: address.to_string(),
        balance_sompi: row.try_get("balance_sompi")?,
        utxo_count: row.try_get("utxo_count")?,
    })
}

pub async fn select_search(query: &str, limit: i64, pool: &Pool<Postgres>) -> Result<Vec<ApiSearchResult>, Error> {
    let mut results = Vec::new();

    if let Ok(hash) = parse_hash(query) {
        if sqlx::query("SELECT 1 FROM blocks WHERE hash = $1").bind(&hash).fetch_optional(pool).await?.is_some() {
            results.push(ApiSearchResult { kind: "block".to_string(), value: hash.to_string() });
        }
        if sqlx::query("SELECT 1 FROM transactions WHERE transaction_id = $1").bind(&hash).fetch_optional(pool).await?.is_some() {
            results.push(ApiSearchResult { kind: "transaction".to_string(), value: hash.to_string() });
        }
    }

    if query.starts_with("kaspa") {
        let count: i64 = sqlx::query("SELECT COUNT(*) FROM addresses_transactions WHERE address = $1")
            .bind(query)
            .fetch_one(pool)
            .await?
            .try_get(0)?;
        if count > 0 {
            results.push(ApiSearchResult { kind: "address".to_string(), value: query.to_string() });
        }
    }

    results.truncate(limit as usize);
    Ok(results)
}

pub async fn select_toccata_metrics(pool: &Pool<Postgres>) -> Result<ApiToccataMetrics, Error> {
    let aggregate = sqlx::query(
        "
        SELECT
            COALESCE((SELECT value FROM toccata_metrics WHERE key = 'tx_v1_count'), 0)::BIGINT AS tx_v1_count,
            COALESCE((SELECT value FROM toccata_metrics WHERE key = 'block_v2_count'), 0)::BIGINT AS block_v2_count,
            COALESCE((SELECT value FROM toccata_metrics WHERE key = 'covenant_tx_count'), 0)::BIGINT AS covenant_tx_count,
            COALESCE((SELECT value FROM toccata_metrics WHERE key = 'covenant_input_count'), 0)::BIGINT AS covenant_input_count,
            COALESCE((SELECT value FROM toccata_metrics WHERE key = 'covenant_output_count'), 0)::BIGINT AS covenant_output_count,
            (SELECT COUNT(*) FROM toccata_covenants)::BIGINT AS covenant_id_count,
            COALESCE((SELECT value FROM toccata_metrics WHERE key = 'user_lane_tx_count'), 0)::BIGINT AS user_lane_tx_count,
            (SELECT COUNT(*) FROM toccata_lanes)::BIGINT AS active_user_lanes,
            COALESCE((SELECT value FROM toccata_metrics WHERE key = 'seq_commit_block_count'), 0)::BIGINT AS seq_commit_block_count
        ",
    )
    .fetch_one(pool)
    .await?;

    let top_covenants = sqlx::query(
        "
        SELECT
            encode(covenant_id, 'hex') AS covenant_id,
            tx_count,
            input_count,
            output_count,
            latest_tx_id
        FROM toccata_covenants
        ORDER BY tx_count DESC, output_count DESC
        LIMIT 20
        ",
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| {
        Ok(ApiToccataCovenantMetrics {
            covenant_id: row.try_get("covenant_id")?,
            tx_count: row.try_get("tx_count")?,
            input_count: row.try_get("input_count")?,
            output_count: row.try_get("output_count")?,
            latest_tx_id: row.try_get("latest_tx_id")?,
        })
    })
    .collect::<Result<Vec<_>, Error>>()?;

    let top_lanes = sqlx::query(
        "
        SELECT
            encode(lane_key, 'hex') AS lane_key,
            tx_count,
            latest_tx_id
        FROM toccata_lanes
        ORDER BY tx_count DESC
        LIMIT 20
        ",
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| {
        Ok(ApiToccataLaneMetrics {
            lane_key: row.try_get("lane_key")?,
            tx_count: row.try_get("tx_count")?,
            latest_tx_id: row.try_get("latest_tx_id")?,
        })
    })
    .collect::<Result<Vec<_>, Error>>()?;

    Ok(ApiToccataMetrics {
        tx_v1_count: aggregate.try_get("tx_v1_count")?,
        block_v2_count: aggregate.try_get("block_v2_count")?,
        covenant_tx_count: aggregate.try_get("covenant_tx_count")?,
        covenant_input_count: aggregate.try_get("covenant_input_count")?,
        covenant_output_count: aggregate.try_get("covenant_output_count")?,
        covenant_id_count: aggregate.try_get("covenant_id_count")?,
        user_lane_tx_count: aggregate.try_get("user_lane_tx_count")?,
        active_user_lanes: aggregate.try_get("active_user_lanes")?,
        seq_commit_block_count: aggregate.try_get("seq_commit_block_count")?,
        top_covenants,
        top_lanes,
    })
}

fn row_to_api_block(row: sqlx::postgres::PgRow) -> Result<ApiBlock, Error> {
    Ok(ApiBlock {
        hash: row.try_get("hash")?,
        selected_parent_hash: row.try_get("selected_parent_hash")?,
        blue_score: row.try_get("blue_score")?,
        daa_score: row.try_get("daa_score")?,
        timestamp: row.try_get("timestamp")?,
        version: row.try_get("version")?,
        transaction_count: row.try_get("transaction_count")?,
        is_chain_block: row.try_get("is_chain_block")?,
    })
}

fn row_to_api_transaction(row: sqlx::postgres::PgRow) -> Result<ApiTransaction, Error> {
    Ok(ApiTransaction {
        transaction_id: row.try_get("transaction_id")?,
        hash: row.try_get("hash")?,
        block_time: row.try_get("block_time")?,
        version: row.try_get("version")?,
        mass: row.try_get("mass")?,
        block_hashes: row.try_get("block_hashes")?,
        accepted_block_hash: row.try_get("accepted_block_hash")?,
    })
}

fn parse_hash(value: &str) -> Result<Hash, ()> {
    use std::str::FromStr;
    kaspa_hashes::Hash::from_str(value).map(Hash::from).map_err(|_| ())
}
