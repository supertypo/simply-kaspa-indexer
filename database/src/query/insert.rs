use sqlx::{Error, Executor, Pool, Postgres};

use crate::models::address_transaction::AddressTransaction;
use crate::models::block::Block;
use crate::models::block_parent::BlockParent;
use crate::models::script_transaction::ScriptTransaction;
use crate::models::transaction::Transaction;
use crate::models::transaction_acceptance::TransactionAcceptance;
use crate::query::common::generate_placeholders;

pub async fn insert_blocks(blocks: &[Block], pool: &Pool<Postgres>) -> Result<u64, Error> {
    const COLS: usize = 16;
    let mut tx = pool.begin().await?;

    let sql = format!(
        "INSERT INTO blocks (hash, accepted_id_merkle_root, merge_set_blues_hashes, merge_set_reds_hashes,
            selected_parent_hash, transaction_ids, bits, blue_score, blue_work, daa_score, hash_merkle_root, nonce,
            pruning_point, timestamp, utxo_commitment, version
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
        query = query.bind(&block.transaction_ids);
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

pub async fn insert_transactions(transactions: &[Transaction], upsert_inputs: bool, pool: &Pool<Postgres>) -> Result<u64, Error> {
    const COLS: usize = 10;
    let on_conflict =
        if upsert_inputs { "ON CONFLICT (transaction_id) DO UPDATE SET inputs = EXCLUDED.inputs" } else { "ON CONFLICT DO NOTHING" };
    let sql = format!(
        "INSERT INTO transactions (transaction_id, subnetwork_id, hash, mass, payload, block_hash, block_time, version, inputs, outputs)
         VALUES {}
         {}",
        generate_placeholders(transactions.len(), COLS),
        on_conflict
    );

    let mut query = sqlx::query(&sql);
    for tx in transactions {
        query = query.bind(&tx.transaction_id);
        query = query.bind(&tx.subnetwork_id);
        query = query.bind(&tx.hash);
        query = query.bind(tx.mass);
        query = query.bind(&tx.payload);
        query = query.bind(&tx.block_hash);
        query = query.bind(tx.block_time);
        query = query.bind(tx.version);
        query = query.bind(&tx.inputs);
        query = query.bind(&tx.outputs);
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
