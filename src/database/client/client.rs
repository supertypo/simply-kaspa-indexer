use std::time::Duration;

use log::{debug, info};
use regex::Regex;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Error, Pool, Postgres};

use crate::database::client::query;
use crate::database::models::address_transaction::AddressTransaction;
use crate::database::models::block::Block;
use crate::database::models::block_transaction::BlockTransaction;
use crate::database::models::chain_block::ChainBlock;
use crate::database::models::subnetwork::Subnetwork;
use crate::database::models::transaction::Transaction;
use crate::database::models::transaction_acceptance::TransactionAcceptance;
use crate::database::models::transaction_input::TransactionInput;
use crate::database::models::transaction_output::TransactionOutput;

#[derive(Clone)]
pub struct KaspaDbClient {
    pool: Pool<Postgres>,
}

impl KaspaDbClient {
    pub async fn new(url: &String) -> Result<KaspaDbClient, Error> {
        Self::new_with_args(url, 10).await
    }

    pub async fn new_with_args(url: &String, pool_size: u32) -> Result<KaspaDbClient, Error> {
        let url_cleaned = Regex::new(r"(postgres://postgres:)[^@]+(@)").unwrap().replace(url, "$1$2");
        debug!("Connecting to PostgreSQL {}", url_cleaned);
        let pool = PgPoolOptions::new().acquire_timeout(Duration::from_secs(10)).max_connections(pool_size).connect(&url).await?;
        info!("Connected to PostgreSQL {}", url_cleaned);
        Ok(KaspaDbClient { pool })
    }

    pub async fn close(&mut self) -> Result<(), Error> {
        self.pool.close().await;
        Ok(())
    }

    pub async fn select_var(&self, key: &str) -> Result<String, Error> {
        query::select::select_var(key, &self.pool).await
    }

    pub async fn select_subnetworks(&self) -> Result<Vec<Subnetwork>, Error> {
        query::select::select_subnetworks(&self.pool).await
    }

    pub async fn select_tx_count(&self, block_hash: &Vec<u8>) -> Result<i64, Error> {
        query::select::select_tx_count(block_hash, &self.pool).await
    }

    pub async fn select_is_chain_block(&self, block_hash: &Vec<u8>) -> Result<bool, Error> {
        query::select::select_is_chain_block(block_hash, &self.pool).await
    }

    pub async fn insert_subnetwork(&self, subnetwork_id: &String) -> Result<i16, Error> {
        query::insert::insert_subnetwork(subnetwork_id, &self.pool).await
    }

    pub async fn insert_blocks(&self, blocks: &[Block]) -> Result<u64, Error> {
        query::insert::insert_blocks(blocks, &self.pool).await
    }

    pub async fn insert_transactions(&self, transactions: &[Transaction]) -> Result<u64, Error> {
        query::insert::insert_transactions(transactions, &self.pool).await
    }

    pub async fn insert_transaction_inputs(&self, transaction_inputs: &[TransactionInput]) -> Result<u64, Error> {
        query::insert::insert_transaction_inputs(transaction_inputs, &self.pool).await
    }

    pub async fn insert_transaction_outputs(&self, transaction_outputs: &[TransactionOutput]) -> Result<u64, Error> {
        query::insert::insert_transaction_outputs(transaction_outputs, &self.pool).await
    }

    pub async fn insert_address_transactions(&self, address_transactions: &[AddressTransaction]) -> Result<u64, Error> {
        query::insert::insert_address_transactions(address_transactions, &self.pool).await
    }

    pub async fn insert_address_transactions_from_inputs(&self, transaction_ids: &[Vec<u8>]) -> Result<u64, Error> {
        query::insert::insert_address_transactions_from_inputs(transaction_ids, &self.pool).await
    }

    pub async fn insert_block_transactions(&self, block_transactions: &[BlockTransaction]) -> Result<u64, Error> {
        query::insert::insert_block_transactions(block_transactions, &self.pool).await
    }

    pub async fn insert_transaction_acceptances(&self, transaction_acceptances: &[TransactionAcceptance]) -> Result<u64, Error> {
        query::insert::insert_transaction_acceptances(transaction_acceptances, &self.pool).await
    }

    pub async fn insert_chain_blocks(&self, chain_blocks: &[ChainBlock]) -> Result<u64, Error> {
        query::insert::insert_chain_blocks(chain_blocks, &self.pool).await
    }

    pub async fn upsert_var(&self, key: &str, value: &String) -> Result<u64, Error> {
        query::upsert::upsert_var(key, value, &self.pool).await
    }

    pub async fn delete_chain_blocks(&self, block_hashes: &[Vec<u8>]) -> Result<u64, Error> {
        query::delete::delete_chain_blocks(block_hashes, &self.pool).await
    }

    pub async fn delete_transaction_acceptances(&self, block_hashes: &[Vec<u8>]) -> Result<u64, Error> {
        query::delete::delete_transaction_acceptances(block_hashes, &self.pool).await
    }
}
