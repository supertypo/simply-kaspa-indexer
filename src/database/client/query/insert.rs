use itertools::Itertools;
use sqlx::{Error, Executor, Pool, Postgres};

use crate::database::models::block::Block;
use crate::database::models::chain_block::ChainBlock;
use crate::database::models::transaction_acceptance::TransactionAcceptance;

pub async fn insert_blocks(blocks: &[Block], pool: &Pool<Postgres>) -> Result<u64, Error> {
    const COLS: usize = 17;
    let mut tx = pool.begin().await?;

    let sql = format!(
        "INSERT INTO blocks (hash, accepted_id_merkle_root, difficulty, merge_set_blues_hashes, merge_set_reds_hashes,
            selected_parent_hash, bits, blue_score, blue_work, daa_score, hash_merkle_root, nonce, parents, pruning_point,
            timestamp, utxo_commitment, version
        ) VALUES {} ON CONFLICT DO NOTHING",
        (0..blocks.len()).map(|i| format!("({})", (1..=COLS).map(|c| format!("${}", c + i * COLS)).join(", "))).join(", ")
    );

    let mut query = sqlx::query(&sql);
    for block in blocks {
        query = query.bind(&block.hash);
        query = query.bind(&block.accepted_id_merkle_root);
        query = query.bind(&block.difficulty);
        query = query.bind(if !&block.merge_set_blues_hashes.is_empty() { Some(&block.merge_set_blues_hashes) } else { None });
        query = query.bind(if !&block.merge_set_reds_hashes.is_empty() { Some(&block.merge_set_reds_hashes) } else { None });
        query = query.bind(&block.selected_parent_hash);
        query = query.bind(&block.bits);
        query = query.bind(&block.blue_score);
        query = query.bind(&block.blue_work);
        query = query.bind(&block.daa_score);
        query = query.bind(&block.hash_merkle_root);
        query = query.bind(&block.nonce);
        query = query.bind(&block.parents);
        query = query.bind(&block.pruning_point);
        query = query.bind(&block.timestamp);
        query = query.bind(&block.utxo_commitment);
        query = query.bind(&block.version);
    }
    let rows_affected = tx.execute(query).await?.rows_affected();
    tx.commit().await?;
    Ok(rows_affected)
}

pub async fn insert_chain_blocks(cbs: &[ChainBlock], pool: &Pool<Postgres>) -> Result<u64, Error> {
    const COLS: usize = 1;
    let sql = format!(
        "INSERT INTO chain_blocks (block_hash) VALUES {} ON CONFLICT DO NOTHING",
        (0..cbs.len()).map(|i| format!("({})", (1..=COLS).map(|c| format!("${}", c + i * COLS)).join(", "))).join(", ")
    );
    let mut query = sqlx::query(&sql);
    for cb in cbs {
        query = query.bind(&cb.block_hash);
    }
    Ok(query.execute(pool).await?.rows_affected())
}

pub async fn insert_transaction_acceptances(tas: &[TransactionAcceptance], db_pool: &Pool<Postgres>) -> Result<u64, Error> {
    const COLS: usize = 2;
    let sql = format!(
        "INSERT INTO transactions_acceptances (transaction_id, block_hash) VALUES {} ON CONFLICT DO NOTHING",
        (0..tas.len()).map(|i| format!("({})", (1..=COLS).map(|c| format!("${}", c + i * COLS)).join(", "))).join(", ")
    );
    let mut query = sqlx::query(&sql);
    for ta in tas {
        query = query.bind(&ta.transaction_id);
        query = query.bind(&ta.block_hash);
    }
    Ok(query.execute(db_pool).await?.rows_affected())
}
