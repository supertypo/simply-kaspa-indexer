use std::str::FromStr;
use std::time::Duration;

use log::{debug, info, warn, LevelFilter};
use regex::Regex;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{ConnectOptions, Error, Pool, Postgres};

use crate::client::query;
use crate::models::address_transaction::AddressTransaction;
use crate::models::block::Block;
use crate::models::block_parent::BlockParent;
use crate::models::block_transaction::BlockTransaction;
use crate::models::chain_block::ChainBlock;
use crate::models::subnetwork::Subnetwork;
use crate::models::transaction::Transaction;
use crate::models::transaction_acceptance::TransactionAcceptance;
use crate::models::transaction_input::TransactionInput;
use crate::models::transaction_output::TransactionOutput;
use crate::models::types::hash::Hash;

#[derive(Clone)]
pub struct KaspaDbClient {
    pool: Pool<Postgres>,
}

impl KaspaDbClient {
    const SCHEMA_VERSION: u8 = 3;

    pub async fn new(url: &String) -> Result<KaspaDbClient, Error> {
        Self::new_with_args(url, 10).await
    }

    pub async fn new_with_args(url: &String, pool_size: u32) -> Result<KaspaDbClient, Error> {
        let url_cleaned = Regex::new(r"(postgres://postgres:)[^@]+(@)").expect("Failed to parse url").replace(url, "$1$2");
        debug!("Connecting to PostgreSQL {}", url_cleaned);
        let connect_opts = PgConnectOptions::from_str(&url)?.log_slow_statements(LevelFilter::Warn, Duration::from_secs(60));
        let pool = PgPoolOptions::new()
            .acquire_timeout(Duration::from_secs(10))
            .max_connections(pool_size)
            .connect_with(connect_opts)
            .await?;
        info!("Connected to PostgreSQL {}", url_cleaned);
        Ok(KaspaDbClient { pool })
    }

    pub async fn close(&mut self) -> Result<(), Error> {
        self.pool.close().await;
        Ok(())
    }

    pub async fn create_schema(&self, upgrade_db: bool) -> Result<(), Error> {
        match &self.select_var("schema_version").await {
            Ok(v) => {
                let mut version = v.parse::<u8>().expect("Existing schema is unknown");
                if version < Self::SCHEMA_VERSION {
                    if version == 1 {
                        let v1_v2_ddl = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/schema/v1_to_v2.sql"));
                        if upgrade_db {
                            warn!("Upgrading schema from v1 to v2, this may take a while...");
                            query::misc::execute_ddl(v1_v2_ddl, &self.pool).await?;
                            self.upsert_var("schema_version", &2.to_string()).await?;
                            info!("\x1b[32mSchema upgrade completed successfully\x1b[0m");
                            version = 2;
                        } else {
                            panic!("\n{}\nFound outdated schema v1. Set flag '-u' to upgrade, or apply manually ^", v1_v2_ddl)
                        }
                    }
                    if version == 2 {
                        let v2_v3_ddl = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/schema/v2_to_v3.sql"));
                        if upgrade_db {
                            warn!("Upgrading schema from v2 to v3...");
                            query::misc::execute_ddl(v2_v3_ddl, &self.pool).await?;
                            self.upsert_var("schema_version", &3.to_string()).await?;
                            info!("\x1b[32mSchema upgrade completed successfully\x1b[0m");
                        } else {
                            warn!("\n{}\nFound outdated schema v2. Set flag '-u' to upgrade, or apply manually ^", v2_v3_ddl)
                        }
                    }
                } else if version > Self::SCHEMA_VERSION {
                    panic!("Found newer & unsupported schema v{}", version)
                } else {
                    info!("Existing schema v{} is up to date", version)
                }
            }
            Err(_) => {
                warn!("Applying schema v{}", Self::SCHEMA_VERSION);
                query::misc::execute_ddl(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/schema/up.sql")), &self.pool)
                    .await?;
                self.upsert_var("schema_version", &Self::SCHEMA_VERSION.to_string()).await?;
                info!("\x1b[32mSchema applied successfully\x1b[0m");
            }
        };
        Ok(())
    }

    pub async fn drop_schema(&self) -> Result<(), Error> {
        query::misc::execute_ddl(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/schema/down.sql")), &self.pool).await
    }

    pub async fn select_var(&self, key: &str) -> Result<String, Error> {
        query::select::select_var(key, &self.pool).await
    }

    pub async fn select_subnetworks(&self) -> Result<Vec<Subnetwork>, Error> {
        query::select::select_subnetworks(&self.pool).await
    }

    pub async fn select_tx_count(&self, block_hash: &Hash) -> Result<i64, Error> {
        query::select::select_tx_count(block_hash, &self.pool).await
    }

    pub async fn select_is_chain_block(&self, block_hash: &Hash) -> Result<bool, Error> {
        query::select::select_is_chain_block(block_hash, &self.pool).await
    }

    pub async fn insert_subnetwork(&self, subnetwork_id: &String) -> Result<i16, Error> {
        query::insert::insert_subnetwork(subnetwork_id, &self.pool).await
    }

    pub async fn insert_blocks(&self, blocks: &[Block]) -> Result<u64, Error> {
        query::insert::insert_blocks(blocks, &self.pool).await
    }

    pub async fn insert_block_parents(&self, block_parents: &[BlockParent]) -> Result<u64, Error> {
        query::insert::insert_block_parents(block_parents, &self.pool).await
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

    pub async fn insert_address_transactions_from_inputs(&self, transaction_ids: &[Hash]) -> Result<u64, Error> {
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

    pub async fn delete_chain_blocks(&self, block_hashes: &[Hash]) -> Result<u64, Error> {
        query::delete::delete_chain_blocks(block_hashes, &self.pool).await
    }

    pub async fn delete_transaction_acceptances(&self, block_hashes: &[Hash]) -> Result<u64, Error> {
        query::delete::delete_transaction_acceptances(block_hashes, &self.pool).await
    }
}
