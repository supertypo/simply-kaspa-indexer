use std::fmt;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};

use bigdecimal::BigDecimal;
use diesel::prelude::*;

pub const VAR_KEY_BLOCK_CHECKPOINT: &str = "block_checkpoint";
pub const VAR_KEY_VIRTUAL_CHECKPOINT: &str = "virtual_checkpoint";
pub const VAR_KEY_LEGACY_CHECKPOINT: &str = "vspc_last_start_hash";

pub trait Similar {
    fn is_similar(&self, other: &Self) -> bool;
}

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
    pub is_chain_block: bool,
    pub merge_set_blues_hashes: Option<Vec<Option<Vec<u8>>>>,
    pub merge_set_reds_hashes: Option<Vec<Option<Vec<u8>>>>,
    pub selected_parent_hash: Option<Vec<u8>>,
    pub bits: Option<i64>,
    pub blue_score: Option<i64>,
    pub blue_work: Option<Vec<u8>>,
    pub daa_score: Option<i64>,
    pub hash_merkle_root: Option<Vec<u8>>,
    pub nonce: Option<BigDecimal>,
    pub parents: Option<Vec<Option<Vec<u8>>>>,
    pub pruning_point: Option<Vec<u8>>,
    pub timestamp: Option<i32>,
    pub utxo_commitment: Option<Vec<u8>>,
    pub version: Option<i16>,
}

impl Block {
    pub fn new(hash: Vec<u8>, is_chain_block: bool) -> Block {
        return Block {
            hash,
            accepted_id_merkle_root: None,
            difficulty: None,
            is_chain_block,
            merge_set_blues_hashes: None,
            merge_set_reds_hashes: None,
            selected_parent_hash: None,
            bits: None,
            blue_score: None,
            blue_work: None,
            daa_score: None,
            hash_merkle_root: None,
            nonce: None,
            parents: None,
            pruning_point: None,
            timestamp: None,
            utxo_commitment: None,
            version: None,
        };
    }
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
    pub subnetwork: Option<i32>,
    pub hash: Option<Vec<u8>>,
    pub mass: Option<i32>,
    pub block_time: Option<i32>,
    pub is_accepted: bool,
    pub accepting_block_hash: Option<Vec<u8>>,
}

impl Transaction {
    pub fn new(transaction_id: Vec<u8>, is_accepted: bool, accepting_block_hash: Option<Vec<u8>>) -> Transaction {
        return Transaction {
            transaction_id,
            subnetwork: None,
            hash: None,
            mass: None,
            block_time: None,
            is_accepted,
            accepting_block_hash,
        };
    }
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
#[diesel(table_name = crate::database::schema::blocks_transactions)]
#[diesel(primary_key(block_hash, transaction_id))]
pub struct BlockTransaction {
    pub block_hash: Vec<u8>,
    pub transaction_id: Vec<u8>,
}

#[derive(Queryable, Selectable, Insertable, Identifiable, QueryableByName, Clone)]
#[diesel(table_name = crate::database::schema::transactions_inputs)]
#[diesel(primary_key(transaction_id, index))]
pub struct TransactionInput {
    pub transaction_id: Vec<u8>,
    pub index: i16,
    pub previous_outpoint_hash: Vec<u8>,
    pub previous_outpoint_index: i16,
    pub script_public_key_address: Option<String>,
    pub signature_script: Vec<u8>,
    pub sig_op_count: i16,
}

impl Debug for TransactionInput {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut output = vec!["TransactionInput {".to_string()];
        output.push(format!("  transaction_id: {}", hex::encode(&self.transaction_id)));
        output.push(format!("  index: {}", &self.index));
        output.push(format!("  previous_outpoint_hash: {}", hex::encode(&self.previous_outpoint_hash)));
        output.push(format!("  previous_outpoint_index: {}", &self.previous_outpoint_index));
        output.push(format!("  script_public_key_address: {}", &self.script_public_key_address.clone().unwrap_or_default()));
        output.push(format!("  sig_op_count: {}", &self.sig_op_count));
        output.push("}".to_string());
        write!(f, "{}", output.join("\n"))
    }
}

impl Similar for TransactionInput {
    fn is_similar(&self, other: &Self) -> bool {
        return self.transaction_id == other.transaction_id
            && self.index == other.index
            && self.previous_outpoint_hash == other.previous_outpoint_hash
            && self.previous_outpoint_index == other.previous_outpoint_index
            // Ignores script_public_key_address as that is newer present when doing the exists check
            && self.signature_script == other.signature_script
            && self.sig_op_count == other.sig_op_count;
    }
}

impl Eq for TransactionInput {}

impl PartialEq for TransactionInput {
    fn eq(&self, other: &Self) -> bool {
        self.transaction_id == other.transaction_id
            && self.index == other.index
    }
}

impl Hash for TransactionInput {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.transaction_id.hash(state);
        self.index.hash(state);
    }
}

#[derive(Queryable, Selectable, Insertable, Identifiable, QueryableByName, Clone, Debug)]
#[diesel(table_name = crate::database::schema::transactions_outputs)]
#[diesel(primary_key(transaction_id, index))]
pub struct TransactionOutput {
    pub transaction_id: Vec<u8>,
    pub index: i16,
    pub amount: i64,
    pub script_public_key: Vec<u8>,
    pub script_public_key_address: String,
    pub script_public_key_type: String,
}

impl Similar for TransactionOutput {
    fn is_similar(&self, other: &Self) -> bool {
        return self.transaction_id == other.transaction_id
            && self.index == other.index
            && self.amount == other.amount
            && self.script_public_key == other.script_public_key
            && self.script_public_key_address == other.script_public_key_address
            && self.script_public_key_type == other.script_public_key_type;
    }
}

impl Eq for TransactionOutput {}

impl PartialEq for TransactionOutput {
    fn eq(&self, other: &Self) -> bool {
        self.transaction_id == other.transaction_id
            && self.index == other.index
    }
}

impl Hash for TransactionOutput {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.transaction_id.hash(state);
        self.index.hash(state);
    }
}
