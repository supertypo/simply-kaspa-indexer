use std::time::Duration;

use sqlx::postgres::PgPoolOptions;
use sqlx::{Error, Pool, Postgres};

use crate::database::client::query;
use crate::database::models::block::Block;

#[derive(Clone)]
pub struct KaspaDbClient {
    pool: Pool<Postgres>,
}

impl KaspaDbClient {
    pub async fn new(url: &String) -> Result<KaspaDbClient, Error> {
        Self::new_with_args(url, 10).await
    }

    pub async fn new_with_args(url: &String, pool_size: u32) -> Result<KaspaDbClient, Error> {
        let pool = PgPoolOptions::new().acquire_timeout(Duration::from_secs(10)).max_connections(pool_size).connect(&url).await?;
        Ok(KaspaDbClient { pool })
    }

    pub async fn close(&mut self) -> Result<(), Error> {
        self.pool.close().await;
        Ok(())
    }

    pub async fn select_tx_count(&self, block_hash: &Vec<u8>) -> Result<i64, Error> {
        query::select::select_tx_count(block_hash, &self.pool).await
    }

    pub async fn select_is_chain_block(&self, block_hash: &Vec<u8>) -> Result<bool, Error> {
        query::select::select_is_chain_block(block_hash, &self.pool).await
    }

    pub async fn insert_blocks(&self, blocks: &Vec<Block>) -> Result<u64, Error> {
        query::insert::insert_blocks(blocks, &self.pool).await
    }

    pub async fn delete_chain_blocks(&self, block_hashes: &Vec<Vec<u8>>) -> Result<u64, Error> {
        query::delete::delete_chain_blocks(block_hashes, &self.pool).await
    }

    pub async fn delete_transaction_acceptances(&self, block_hashes: &Vec<Vec<u8>>) -> Result<u64, Error> {
        query::delete::delete_chain_blocks(block_hashes, &self.pool).await
    }
}
