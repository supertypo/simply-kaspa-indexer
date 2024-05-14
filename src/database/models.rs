use std::fmt::Debug;
use std::hash::{Hash, Hasher};

use diesel::prelude::*;

pub const VAR_KEY_BLOCK_CHECKPOINT: &str = "block_checkpoint";
pub const VAR_KEY_LEGACY_CHECKPOINT: &str = "vspc_last_start_hash";

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
    pub accepted_id_merkle_root: Vec<u8>,
    pub difficulty: f64,
    pub merge_set_blues_hashes: Vec<Vec<u8>>,
    pub merge_set_reds_hashes: Vec<Vec<u8>>,
    pub selected_parent_hash: Vec<u8>,
    pub bits: i64,
    pub blue_score: i64,
    pub blue_work: Vec<u8>,
    pub daa_score: i64,
    pub hash_merkle_root: Vec<u8>,
    pub nonce: Vec<u8>,
    pub parents: Vec<Vec<u8>>,
    pub pruning_point: Vec<u8>,
    pub timestamp: i64,
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

#[derive(Queryable, Selectable, Insertable, Clone, Eq, PartialEq, Hash)]
#[diesel(table_name = crate::database::schema::chain_blocks)]
#[diesel(primary_key(block_hash))]
pub struct ChainBlock {
    pub block_hash: Vec<u8>,
}

#[derive(Queryable, Selectable, Clone)]
#[diesel(table_name = crate::database::schema::subnetworks)]
#[diesel(primary_key(id))]
pub struct Subnetwork {
    pub id: i16,
    pub subnetwork_id: String,
}

#[derive(Insertable)]
#[diesel(table_name = crate::database::schema::subnetworks)]
pub struct SubnetworkInsertable {
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
    pub subnetwork_id: i16,
    pub hash: Vec<u8>,
    pub mass: Option<i32>,
    pub block_time: i64,
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

#[derive(Queryable, Selectable, Insertable, Clone, Eq, PartialEq, Hash)]
#[diesel(table_name = crate::database::schema::transactions_acceptances)]
#[diesel(primary_key(transaction_id))]
pub struct TransactionAcceptance {
    pub transaction_id: Vec<u8>,
    pub block_hash: Vec<u8>,
}

#[derive(Queryable, Selectable, Insertable, Clone, Eq, PartialEq, Hash)]
#[diesel(table_name = crate::database::schema::blocks_transactions)]
#[diesel(primary_key(block_hash, transaction_id))]
pub struct BlockTransaction {
    pub block_hash: Vec<u8>,
    pub transaction_id: Vec<u8>,
}

#[derive(Queryable, Selectable, Insertable, Identifiable, Clone)]
#[diesel(table_name = crate::database::schema::transactions_inputs)]
#[diesel(primary_key(transaction_id, index))]
pub struct TransactionInput {
    pub transaction_id: Vec<u8>,
    pub index: i16,
    pub previous_outpoint_hash: Vec<u8>,
    pub previous_outpoint_index: i16,
    pub signature_script: Vec<u8>,
    pub sig_op_count: i16,
}

impl Eq for TransactionInput {}

impl PartialEq for TransactionInput {
    fn eq(&self, other: &Self) -> bool {
        self.transaction_id == other.transaction_id && self.index == other.index
    }
}

impl Hash for TransactionInput {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.transaction_id.hash(state);
        self.index.hash(state);
    }
}

#[derive(Queryable, Selectable, Insertable, Identifiable, Clone, Debug)]
#[diesel(table_name = crate::database::schema::transactions_outputs)]
#[diesel(primary_key(transaction_id, index))]
pub struct TransactionOutput {
    pub transaction_id: Vec<u8>,
    pub index: i16,
    pub amount: i64,
    pub script_public_key: Vec<u8>,
    pub script_public_key_address: String,
}

impl Eq for TransactionOutput {}

impl PartialEq for TransactionOutput {
    fn eq(&self, other: &Self) -> bool {
        self.transaction_id == other.transaction_id && self.index == other.index
    }
}

impl Hash for TransactionOutput {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.transaction_id.hash(state);
        self.index.hash(state);
    }
}

#[derive(Queryable, Selectable, Insertable, Identifiable, Clone, Debug)]
#[diesel(table_name = crate::database::schema::addresses_transactions)]
#[diesel(primary_key(address, transaction_id))]
pub struct AddressTransaction {
    pub address: String,
    pub transaction_id: Vec<u8>,
    pub block_time: i64,
}

impl Eq for AddressTransaction {}

impl PartialEq for AddressTransaction {
    fn eq(&self, other: &Self) -> bool {
        self.address == other.address && self.transaction_id == other.transaction_id
    }
}

impl Hash for AddressTransaction {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.address.hash(state);
        self.transaction_id.hash(state);
    }
}
