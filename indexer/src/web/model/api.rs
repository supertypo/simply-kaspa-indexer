use serde::{Deserialize, Serialize};
use simply_kaspa_database::models::query::api::{
    ApiAddressBalance, ApiAddressTransaction, ApiAddressUtxo, ApiBlock, ApiSearchResult, ApiTransaction,
};
use utoipa::{IntoParams, ToSchema};

#[derive(Deserialize, IntoParams)]
pub struct LimitQuery {
    pub limit: Option<i64>,
}

#[derive(Deserialize, IntoParams)]
pub struct SearchQuery {
    pub q: String,
    pub limit: Option<i64>,
}

#[derive(ToSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockSummary {
    pub hash: String,
    pub selected_parent_hash: Option<String>,
    pub blue_score: Option<i64>,
    pub daa_score: Option<i64>,
    pub timestamp: Option<i64>,
    pub version: Option<i16>,
    pub transaction_count: i64,
    pub is_chain_block: bool,
}

impl From<ApiBlock> for BlockSummary {
    fn from(block: ApiBlock) -> Self {
        Self {
            hash: block.hash.to_string(),
            selected_parent_hash: block.selected_parent_hash.map(|hash| hash.to_string()),
            blue_score: block.blue_score,
            daa_score: block.daa_score,
            timestamp: block.timestamp,
            version: block.version,
            transaction_count: block.transaction_count,
            is_chain_block: block.is_chain_block,
        }
    }
}

#[derive(ToSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionSummary {
    pub transaction_id: String,
    pub hash: Option<String>,
    pub block_time: Option<i64>,
    pub version: Option<i16>,
    pub mass: Option<i32>,
    pub block_hashes: Vec<String>,
    pub accepted_block_hash: Option<String>,
}

impl From<ApiTransaction> for TransactionSummary {
    fn from(transaction: ApiTransaction) -> Self {
        Self {
            transaction_id: transaction.transaction_id.to_string(),
            hash: transaction.hash.map(|hash| hash.to_string()),
            block_time: transaction.block_time,
            version: transaction.version,
            mass: transaction.mass,
            block_hashes: transaction.block_hashes.into_iter().map(|hash| hash.to_string()).collect(),
            accepted_block_hash: transaction.accepted_block_hash.map(|hash| hash.to_string()),
        }
    }
}

#[derive(ToSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddressTransactionSummary {
    pub address: String,
    pub transaction_id: String,
    pub block_time: i64,
}

impl From<ApiAddressTransaction> for AddressTransactionSummary {
    fn from(transaction: ApiAddressTransaction) -> Self {
        Self {
            address: transaction.address,
            transaction_id: transaction.transaction_id.to_string(),
            block_time: transaction.block_time,
        }
    }
}

#[derive(ToSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddressBalance {
    pub address: String,
    pub balance_sompi: i64,
    pub balance_kas: f64,
    pub utxo_count: i64,
}

impl From<ApiAddressBalance> for AddressBalance {
    fn from(balance: ApiAddressBalance) -> Self {
        Self {
            address: balance.address,
            balance_sompi: balance.balance_sompi,
            balance_kas: balance.balance_sompi as f64 / 100_000_000.0,
            utxo_count: balance.utxo_count,
        }
    }
}

#[derive(ToSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddressUtxo {
    pub transaction_id: String,
    pub output_index: i16,
    pub amount_sompi: i64,
    pub amount_kas: f64,
    pub block_time: Option<i64>,
}

impl From<ApiAddressUtxo> for AddressUtxo {
    fn from(utxo: ApiAddressUtxo) -> Self {
        Self {
            transaction_id: utxo.transaction_id.to_string(),
            output_index: utxo.output_index,
            amount_sompi: utxo.amount_sompi,
            amount_kas: utxo.amount_sompi as f64 / 100_000_000.0,
            block_time: utxo.block_time,
        }
    }
}

#[derive(ToSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub kind: String,
    pub value: String,
}

impl From<ApiSearchResult> for SearchResult {
    fn from(result: ApiSearchResult) -> Self {
        Self { kind: result.kind, value: result.value }
    }
}

#[derive(ToSchema, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub chain_tip: Option<BlockSummary>,
    pub checkpoint_hash: Option<String>,
    pub checkpoint_daa_score: Option<u64>,
    pub virtual_chain_tip_distance: Option<u64>,
    pub transaction_processor_enabled: bool,
    pub virtual_chain_processor_enabled: bool,
}
