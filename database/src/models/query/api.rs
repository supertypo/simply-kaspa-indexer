use crate::models::types::hash::Hash;

pub struct ApiBlock {
    pub hash: Hash,
    pub selected_parent_hash: Option<Hash>,
    pub blue_score: Option<i64>,
    pub daa_score: Option<i64>,
    pub timestamp: Option<i64>,
    pub version: Option<i16>,
    pub transaction_count: i64,
    pub is_chain_block: bool,
}

pub struct ApiTransaction {
    pub transaction_id: Hash,
    pub hash: Option<Hash>,
    pub block_time: Option<i64>,
    pub version: Option<i16>,
    pub mass: Option<i32>,
    pub block_hashes: Vec<Hash>,
    pub accepted_block_hash: Option<Hash>,
}

pub struct ApiAddressTransaction {
    pub address: String,
    pub transaction_id: Hash,
    pub block_time: i64,
}

pub struct ApiAddressBalance {
    pub address: String,
    pub balance_sompi: i64,
    pub utxo_count: i64,
}

pub struct ApiAddressUtxo {
    pub transaction_id: Hash,
    pub output_index: i16,
    pub amount_sompi: i64,
    pub block_time: Option<i64>,
}

pub struct ApiSearchResult {
    pub kind: String,
    pub value: String,
}

pub struct ApiToccataMetrics {
    pub rollup_updated_at: Option<i64>,
    pub tx_v1_count: i64,
    pub block_v2_count: i64,
    pub covenant_tx_count: i64,
    pub covenant_input_count: i64,
    pub covenant_output_count: i64,
    pub covenant_id_count: i64,
    pub user_lane_tx_count: i64,
    pub active_user_lanes: i64,
    pub seq_commit_block_count: i64,
    pub top_covenants: Vec<ApiToccataCovenantMetrics>,
    pub top_lanes: Vec<ApiToccataLaneMetrics>,
}

pub struct ApiToccataCovenantMetrics {
    pub covenant_id: String,
    pub tx_count: i64,
    pub input_count: i64,
    pub output_count: i64,
    pub latest_tx_id: Option<Hash>,
}

pub struct ApiToccataLaneMetrics {
    pub lane_key: String,
    pub tx_count: i64,
    pub latest_tx_id: Option<Hash>,
}
