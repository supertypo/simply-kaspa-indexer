use std::str::FromStr;
use std::time::Duration;

use log::{debug, info, trace, warn, LevelFilter};
use regex::Regex;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{ConnectOptions, Error, Pool, Postgres};

use crate::client::query;
use crate::models::address_transaction::AddressTransaction;
use crate::models::block::Block;
use crate::models::block_parent::BlockParent;
use crate::models::block_transaction::BlockTransaction;
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
    const SCHEMA_VERSION: u8 = 5;

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
                let mut version = v.parse::<u8>().expect("Expected valid schema version");
                if version < Self::SCHEMA_VERSION {
                    if version == 1 {
                        let v1_v2_ddl = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/schema/v1_to_v2.sql"));
                        if upgrade_db {
                            warn!("Upgrading schema from v1 to v2, this may take a while...");
                            query::misc::execute_ddl(v1_v2_ddl, &self.pool).await?;
                            info!("\x1b[32mSchema upgrade completed successfully\x1b[0m");
                            version = 2;
                        } else {
                            panic!("\n{v1_v2_ddl}\nFound outdated schema v1. Set flag '-u' to upgrade, or apply manually ^")
                        }
                    }
                    if version == 2 {
                        let v2_v3_ddl = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/schema/v2_to_v3.sql"));
                        if upgrade_db {
                            warn!("Upgrading schema from v2 to v3...");
                            query::misc::execute_ddl(v2_v3_ddl, &self.pool).await?;
                            info!("\x1b[32mSchema upgrade completed successfully\x1b[0m");
                            version = 3;
                        } else {
                            panic!("\n{v2_v3_ddl}\nFound outdated schema v2. Set flag '-u' to upgrade, or apply manually ^")
                        }
                    }
                    if version == 3 {
                        let v3_v4_ddl = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/schema/v3_to_v4.sql"));
                        if upgrade_db {
                            warn!("Upgrading schema from v3 to v4...");
                            query::misc::execute_ddl(v3_v4_ddl, &self.pool).await?;
                            info!("\x1b[32mSchema upgrade completed successfully\x1b[0m");
                            version = 4;
                        } else {
                            panic!("\n{v3_v4_ddl}\nFound outdated schema v3. Set flag '-u' to upgrade, or apply manually ^")
                        }
                    }
                    if version == 4 {
                        let v4_v5_ddl = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/schema/v4_to_v5.sql"));
                        if upgrade_db {
                            warn!("Upgrading schema from v4 to v5...");
                            query::misc::execute_ddl(v4_v5_ddl, &self.pool).await?;
                            info!("\x1b[32mSchema upgrade completed successfully\x1b[0m");
                            version = 5;
                        } else {
                            panic!("\n{v4_v5_ddl}\nFound outdated schema v4. Set flag '-u' to upgrade, or apply manually ^")
                        }
                    }
                    trace!("Schema version is v{version}")
                }
                version = self.select_var("schema_version").await?.parse::<u8>().unwrap();
                if version < Self::SCHEMA_VERSION {
                    panic!("Found old & unsupported schema v{version}", )
                }
                if version > Self::SCHEMA_VERSION {
                    panic!("Found newer & unsupported schema v{version}", )
                }
                info!("Schema v{} is up to date", version)
            }
            Err(_) => {
                warn!("Applying schema v{}", Self::SCHEMA_VERSION);
                query::misc::execute_ddl(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/schema/up.sql")), &self.pool)
                    .await?;
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

    pub async fn insert_subnetwork(&self, subnetwork_id: &String) -> Result<i32, Error> {
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

    pub async fn upsert_var(&self, key: &str, value: &String) -> Result<u64, Error> {
        query::upsert::upsert_var(key, value, &self.pool).await
    }

    pub async fn delete_transaction_acceptances(&self, block_hashes: &[Hash]) -> Result<u64, Error> {
        query::delete::delete_transaction_acceptances(block_hashes, &self.pool).await
    }
}
