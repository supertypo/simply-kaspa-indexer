use itertools::Itertools;
use sqlx::{Error, Executor, Pool, Postgres, Row};

use crate::models::address_transaction::AddressTransaction;
use crate::models::block::Block;
use crate::models::block_parent::BlockParent;
use crate::models::block_transaction::BlockTransaction;
use crate::models::script_transaction::ScriptTransaction;
use crate::models::transaction::Transaction;
use crate::models::transaction_acceptance::TransactionAcceptance;
use crate::models::transaction_input::TransactionInput;
use crate::models::transaction_output::TransactionOutput;
use crate::models::types::hash::Hash;
use crate::models::sequencing_commitment::SequencingCommitment;

pub async fn insert_subnetwork(subnetwork_id: &String, pool: &Pool<Postgres>) -> Result<i32, Error> {
    sqlx::query("INSERT INTO subnetworks (subnetwork_id) VALUES ($1) ON CONFLICT DO NOTHING RETURNING id")
        .bind(subnetwork_id)
        .fetch_one(pool)
        .await?
        .try_get(0)
}

pub async fn insert_blocks(blocks: &[Block], pool: &Pool<Postgres>) -> Result<u64, Error> {
    const COLS: usize = 15;
    let mut tx = pool.begin().await?;

    let sql = format!(
        "INSERT INTO blocks (hash, accepted_id_merkle_root, merge_set_blues_hashes, merge_set_reds_hashes,
            selected_parent_hash, bits, blue_score, blue_work, daa_score, hash_merkle_root, nonce, pruning_point,
            timestamp, utxo_commitment, version
        ) VALUES {} ON CONFLICT DO NOTHING",
        generate_placeholders(blocks.len(), COLS)
    );

    let mut query = sqlx::query(&sql);
    for block in blocks {
        query = query.bind(&block.hash);
        query = query.bind(&block.accepted_id_merkle_root);
        query = query.bind(&block.merge_set_blues_hashes);
        query = query.bind(&block.merge_set_reds_hashes);
        query = query.bind(&block.selected_parent_hash);
        query = query.bind(block.bits);
        query = query.bind(block.blue_score);
        query = query.bind(&block.blue_work);
        query = query.bind(block.daa_score);
        query = query.bind(&block.hash_merkle_root);
        query = query.bind(&block.nonce);
        query = query.bind(&block.pruning_point);
        query = query.bind(block.timestamp);
        query = query.bind(&block.utxo_commitment);
        query = query.bind(block.version);
    }
    let rows_affected = tx.execute(query).await?.rows_affected();
    tx.commit().await?;
    Ok(rows_affected)
}

pub async fn insert_block_parents(block_parents: &[BlockParent], pool: &Pool<Postgres>) -> Result<u64, Error> {
    const COLS: usize = 2;
    let sql = format!(
        "INSERT INTO block_parent (block_hash, parent_hash)
        VALUES {} ON CONFLICT DO NOTHING",
        generate_placeholders(block_parents.len(), COLS)
    );
    let mut query = sqlx::query(&sql);
    for block_transaction in block_parents {
        query = query.bind(&block_transaction.block_hash);
        query = query.bind(&block_transaction.parent_hash);
    }
    Ok(query.execute(pool).await?.rows_affected())
}

pub async fn insert_transactions(transactions: &[Transaction], pool: &Pool<Postgres>) -> Result<u64, Error> {
    const COLS: usize = 7;
    let sql = format!(
        "INSERT INTO transactions (transaction_id, subnetwork_id, hash, mass, payload, block_time, tag_id)
        VALUES {} ON CONFLICT DO NOTHING",
        generate_placeholders(transactions.len(), COLS)
    );
    let mut query = sqlx::query(&sql);
    for tx in transactions {
        query = query.bind(&tx.transaction_id);
        query = query.bind(tx.subnetwork_id);
        query = query.bind(&tx.hash);
        query = query.bind(tx.mass);
        query = query.bind(&tx.payload);
        query = query.bind(tx.block_time);
        query = query.bind(tx.tag_id);
    }
    Ok(query.execute(pool).await?.rows_affected())
}

pub async fn insert_transaction_inputs(
    resolve_previous_outpoints: bool,
    transaction_inputs: &[TransactionInput],
    pool: &Pool<Postgres>,
) -> Result<u64, Error> {
    const COLS: usize = 9;
    let sql = if resolve_previous_outpoints {
        format!(
            "INSERT INTO transactions_inputs (transaction_id, index, previous_outpoint_hash, previous_outpoint_index, 
                signature_script, sig_op_count, block_time, previous_outpoint_script, previous_outpoint_amount)
            SELECT 
                i.transaction_id, i.index, i.previous_outpoint_hash, i.previous_outpoint_index, i.signature_script, i.sig_op_count, i.block_time, 
                COALESCE(i.previous_outpoint_script, o.script_public_key), 
                COALESCE(i.previous_outpoint_amount, o.amount)
            FROM (VALUES {}) AS i (transaction_id, index, previous_outpoint_hash, previous_outpoint_index,
                signature_script, sig_op_count, block_time, previous_outpoint_script, previous_outpoint_amount)
            LEFT JOIN transactions_outputs o
                ON i.previous_outpoint_hash = o.transaction_id 
                AND i.previous_outpoint_index = o.index
            ON CONFLICT DO NOTHING",
            generate_placeholders(transaction_inputs.len(), COLS)
        )
    } else {
        format!(
            "INSERT INTO transactions_inputs (transaction_id, index, previous_outpoint_hash, previous_outpoint_index,
                signature_script, sig_op_count, block_time, previous_outpoint_script, previous_outpoint_amount)
            VALUES {} ON CONFLICT DO NOTHING",
            generate_placeholders(transaction_inputs.len(), COLS)
        )
    };
    let mut query = sqlx::query(&sql);
    for tin in transaction_inputs {
        query = query.bind(&tin.transaction_id);
        query = query.bind(tin.index);
        query = query.bind(&tin.previous_outpoint_hash);
        query = query.bind(tin.previous_outpoint_index);
        query = query.bind(&tin.signature_script);
        query = query.bind(tin.sig_op_count);
        query = query.bind(tin.block_time);
        query = query.bind(&tin.previous_outpoint_script);
        query = query.bind(tin.previous_outpoint_amount);
    }
    Ok(query.execute(pool).await?.rows_affected())
}

pub async fn insert_transaction_outputs(transaction_outputs: &[TransactionOutput], pool: &Pool<Postgres>) -> Result<u64, Error> {
    const COLS: usize = 6;
    let sql = format!(
        "INSERT INTO transactions_outputs (transaction_id, index, amount, script_public_key, script_public_key_address, block_time)
        VALUES {} ON CONFLICT DO NOTHING",
        generate_placeholders(transaction_outputs.len(), COLS)
    );
    let mut query = sqlx::query(&sql);
    for tout in transaction_outputs {
        query = query.bind(&tout.transaction_id);
        query = query.bind(tout.index);
        query = query.bind(tout.amount);
        query = query.bind(&tout.script_public_key);
        query = query.bind(&tout.script_public_key_address);
        query = query.bind(tout.block_time);
    }
    Ok(query.execute(pool).await?.rows_affected())
}

pub async fn insert_address_transactions(address_transactions: &[AddressTransaction], pool: &Pool<Postgres>) -> Result<u64, Error> {
    const COLS: usize = 3;
    let sql = format!(
        "INSERT INTO addresses_transactions (address, transaction_id, block_time)
        VALUES {} ON CONFLICT DO NOTHING",
        generate_placeholders(address_transactions.len(), COLS)
    );
    let mut query = sqlx::query(&sql);
    for address_transaction in address_transactions {
        query = query.bind(&address_transaction.address);
        query = query.bind(&address_transaction.transaction_id);
        query = query.bind(address_transaction.block_time);
    }
    Ok(query.execute(pool).await?.rows_affected())
}

pub async fn insert_script_transactions(script_transactions: &[ScriptTransaction], pool: &Pool<Postgres>) -> Result<u64, Error> {
    const COLS: usize = 3;
    let sql = format!(
        "INSERT INTO scripts_transactions (script_public_key, transaction_id, block_time)
        VALUES {} ON CONFLICT DO NOTHING",
        generate_placeholders(script_transactions.len(), COLS)
    );
    let mut query = sqlx::query(&sql);
    for script_transaction in script_transactions {
        query = query.bind(&script_transaction.script_public_key);
        query = query.bind(&script_transaction.transaction_id);
        query = query.bind(script_transaction.block_time);
    }
    Ok(query.execute(pool).await?.rows_affected())
}

pub async fn insert_address_transactions_from_inputs(
    use_tx: bool,
    transaction_ids: &[Hash],
    pool: &Pool<Postgres>,
) -> Result<u64, Error> {
    let sql = if use_tx {
        "INSERT INTO addresses_transactions (address, transaction_id, block_time)
        SELECT o.script_public_key_address, i.transaction_id, t.block_time
            FROM transactions_inputs i
            JOIN transactions t ON t.transaction_id = i.transaction_id
            JOIN transactions_outputs o ON o.transaction_id = i.previous_outpoint_hash AND o.index = i.previous_outpoint_index
        WHERE i.transaction_id = ANY($1) AND t.transaction_id = ANY($1)
        ON CONFLICT DO NOTHING"
    } else {
        "INSERT INTO addresses_transactions (address, transaction_id, block_time)
        SELECT o.script_public_key_address, i.transaction_id, i.block_time
            FROM transactions_inputs i
            JOIN transactions_outputs o ON o.transaction_id = i.previous_outpoint_hash AND o.index = i.previous_outpoint_index
        WHERE i.transaction_id = ANY($1)
        ON CONFLICT DO NOTHING"
    };
    Ok(sqlx::query(sql).bind(transaction_ids).execute(pool).await?.rows_affected())
}

pub async fn insert_script_transactions_from_inputs(
    use_tx: bool,
    transaction_ids: &[Hash],
    pool: &Pool<Postgres>,
) -> Result<u64, Error> {
    let sql = if use_tx {
        "INSERT INTO scripts_transactions (script_public_key, transaction_id, block_time)
        SELECT o.script_public_key, i.transaction_id, t.block_time
            FROM transactions_inputs i
            JOIN transactions t ON t.transaction_id = i.transaction_id
            JOIN transactions_outputs o ON o.transaction_id = i.previous_outpoint_hash AND o.index = i.previous_outpoint_index
        WHERE i.transaction_id = ANY($1) AND t.transaction_id = ANY($1)
        ON CONFLICT DO NOTHING"
    } else {
        "INSERT INTO scripts_transactions (script_public_key, transaction_id, block_time)
        SELECT o.script_public_key, i.transaction_id, i.block_time
            FROM transactions_inputs i
            JOIN transactions_outputs o ON o.transaction_id = i.previous_outpoint_hash AND o.index = i.previous_outpoint_index
        WHERE i.transaction_id = ANY($1)
        ON CONFLICT DO NOTHING"
    };
    Ok(sqlx::query(sql).bind(transaction_ids).execute(pool).await?.rows_affected())
}

pub async fn insert_block_transactions(block_transactions: &[BlockTransaction], pool: &Pool<Postgres>) -> Result<u64, Error> {
    const COLS: usize = 2;
    let sql = format!(
        "INSERT INTO blocks_transactions (block_hash, transaction_id)
        VALUES {} ON CONFLICT DO NOTHING",
        generate_placeholders(block_transactions.len(), COLS)
    );
    let mut query = sqlx::query(&sql);
    for block_transaction in block_transactions {
        query = query.bind(&block_transaction.block_hash);
        query = query.bind(&block_transaction.transaction_id);
    }
    Ok(query.execute(pool).await?.rows_affected())
}

pub async fn insert_transaction_acceptances(tx_acceptances: &[TransactionAcceptance], pool: &Pool<Postgres>) -> Result<u64, Error> {
    const COLS: usize = 2;
    let sql = format!(
        "INSERT INTO transactions_acceptances (transaction_id, block_hash) VALUES {} ON CONFLICT DO NOTHING",
        generate_placeholders(tx_acceptances.len(), COLS)
    );
    let mut query = sqlx::query(&sql);
    for ta in tx_acceptances {
        query = query.bind(&ta.transaction_id);
        query = query.bind(&ta.block_hash);
    }
    Ok(query.execute(pool).await?.rows_affected())
}

fn generate_placeholders(rows: usize, columns: usize) -> String {
    (0..rows).map(|i| format!("({})", (1..=columns).map(|c| format!("${}", c + i * columns)).join(", "))).join(", ")
}

pub async fn insert_sequencing_commitments(commitments: &[SequencingCommitment], pool: &Pool<Postgres>) -> Result<u64, Error> {
    const COLS: usize = 3;
    let sql = format!(
        "INSERT INTO sequencing_commitments (block_hash, seqcom_hash, parent_seqcom_hash)
        VALUES {} ON CONFLICT DO NOTHING",
        generate_placeholders(commitments.len(), COLS)
    );
    let mut query = sqlx::query(&sql);
    for commitment in commitments {
        query = query.bind(&commitment.block_hash);
        query = query.bind(&commitment.seqcom_hash);
        query = query.bind(&commitment.parent_seqcom_hash);
    }
    Ok(query.execute(pool).await?.rows_affected())
}

/// Insert a single tag provider, returning its ID
pub async fn insert_tag_provider(
    tag: &str,
    module: Option<&str>,
    prefix: &str,
    repository_url: Option<&str>,
    description: Option<&str>,
    pool: &Pool<Postgres>,
) -> Result<i32, Error> {
    sqlx::query(
        "INSERT INTO tag_providers (tag, module, prefix, repository_url, description)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (tag, module) DO UPDATE SET
             prefix = EXCLUDED.prefix,
             repository_url = EXCLUDED.repository_url,
             description = EXCLUDED.description
         RETURNING id"
    )
    .bind(tag)
    .bind(module)
    .bind(prefix)
    .bind(repository_url)
    .bind(description)
    .fetch_one(pool)
    .await?
    .try_get(0)
}

/// Get tag_provider ID by tag name
pub async fn get_tag_id_by_name(tag: &str, pool: &Pool<Postgres>) -> Result<Option<i32>, Error> {
    sqlx::query("SELECT id FROM tag_providers WHERE tag = $1")
        .bind(tag)
        .fetch_optional(pool)
        .await?
        .map(|row| row.try_get(0))
        .transpose()
}
