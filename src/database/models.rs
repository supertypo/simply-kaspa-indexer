use std::hash::{Hash, Hasher};
use bigdecimal::BigDecimal;
use diesel::prelude::*;

pub const VAR_KEY_START_HASH: &str = "vspc_last_start_hash";

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::database::schema::vars)]
#[diesel(primary_key(key))]
pub struct Var {
    pub key: String,
    pub value: String,
}

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::database::schema::blocks)]
#[diesel(primary_key(hash))]
pub struct Block {
    pub hash: Vec<u8>,
    pub accepted_id_merkle_root: Option<Vec<u8>>,
    pub difficulty: Option<f64>,
    pub is_chain_block: Option<bool>,
    pub merge_set_blues_hashes: Option<Vec<Option<Vec<u8>>>>,
    pub merge_set_reds_hashes: Option<Vec<Option<Vec<u8>>>>,
    pub selected_parent_hash: Option<Vec<u8>>,
    pub bits: i64,
    pub blue_score: i64,
    pub blue_work: Vec<u8>,
    pub daa_score: i64,
    pub hash_merkle_root: Vec<u8>,
    pub nonce: BigDecimal,
    pub parents: Vec<Option<Vec<u8>>>,
    pub pruning_point: Vec<u8>,
    pub timestamp: i32,
    pub utxo_commitment: Vec<u8>,
    pub version: i16,
}

impl Eq for Block {}

impl PartialEq for Block {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl Hash for Block {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}

#[derive(Insertable)]
#[diesel(table_name = crate::database::schema::subnetworks)]
pub struct SubnetworkInsertable {
    pub subnetwork_id: String,
}

#[derive(Queryable, Selectable, Clone)]
#[diesel(table_name = crate::database::schema::subnetworks)]
#[diesel(primary_key(id))]
pub struct Subnetwork {
    pub id: i32,
    pub subnetwork_id: String,
}

impl Eq for Subnetwork {}

impl PartialEq for Subnetwork {
    fn eq(&self, other: &Self) -> bool {
        self.subnetwork_id == other.subnetwork_id
    }
}

impl Hash for Subnetwork {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.subnetwork_id.hash(state);
    }
}

#[derive(Queryable, Selectable, Insertable, Clone)]
#[diesel(table_name = crate::database::schema::transactions)]
#[diesel(primary_key(transaction_id))]
pub struct Transaction {
    pub transaction_id: Vec<u8>,
    pub subnetwork: i32,
    pub hash: Vec<u8>,
    pub mass: i32,
    pub block_hash: Vec<Option<Vec<u8>>>,
    pub block_time: i32,
    pub is_accepted: bool,
    pub accepting_block_hash: Option<Vec<u8>>,
}

impl Eq for Transaction {}

impl PartialEq for Transaction {
    fn eq(&self, other: &Self) -> bool {
        self.transaction_id == other.transaction_id
    }
}

impl Hash for Transaction {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.transaction_id.hash(state);
    }
}
