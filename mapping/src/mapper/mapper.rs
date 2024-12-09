use kaspa_rpc_core::{RpcBlock, RpcTransaction};

use simply_kaspa_database::models::address_transaction::AddressTransaction as SqlAddressTransaction;
use simply_kaspa_database::models::block::Block as SqlBlock;
use simply_kaspa_database::models::block_parent::BlockParent as SqlBlockParent;
use simply_kaspa_database::models::block_transaction::BlockTransaction as SqlBlockTransaction;
use simply_kaspa_database::models::transaction::Transaction as SqlTransaction;
use simply_kaspa_database::models::transaction_input::TransactionInput as SqlTransactionInput;
use simply_kaspa_database::models::transaction_output::TransactionOutput as SqlTransactionOutput;
use simply_kaspa_database::models::types::hash::Hash as SqlHash;

use crate::mapper::{blocks, transactions};

#[derive(Clone)]
pub struct KaspaDbMapper {
    block_accepted_id_merkle_root: bool,
    block_merge_set_blues_hashes: bool,
    block_merge_set_reds_hashes: bool,
    block_selected_parent_hash: bool,
    block_bits: bool,
    block_blue_work: bool,
    block_daa_score: bool,
    block_hash_merkle_root: bool,
    block_nonce: bool,
    block_pruning_point: bool,
    block_timestamp: bool,
    block_utxo_commitment: bool,
    block_version: bool,
    tx_hash: bool,
    tx_mass: bool,
    tx_payload: bool,
    tx_in_signature_script: bool,
    tx_in_sig_op_count: bool,
    tx_out_script_public_key_address: bool,
}

impl KaspaDbMapper {
    pub fn new(exclude_fields: &Option<Vec<String>>, include_fields: &Option<Vec<String>>) -> KaspaDbMapper {
        KaspaDbMapper {
            block_accepted_id_merkle_root: include_field(exclude_fields, include_fields, "block.accepted_id_merkle_root"),
            block_merge_set_blues_hashes: include_field(exclude_fields, include_fields, "block.merge_set_blues_hashes"),
            block_merge_set_reds_hashes: include_field(exclude_fields, include_fields, "block.merge_set_reds_hashes"),
            block_selected_parent_hash: include_field(exclude_fields, include_fields, "block.selected_parent_hash"),
            block_bits: include_field(exclude_fields, include_fields, "block.bits"),
            block_blue_work: include_field(exclude_fields, include_fields, "block.blue_work"),
            block_daa_score: include_field(exclude_fields, include_fields, "block.daa_score"),
            block_hash_merkle_root: include_field(exclude_fields, include_fields, "block.hash_merkle_root"),
            block_nonce: include_field(exclude_fields, include_fields, "block.nonce"),
            block_pruning_point: include_field(exclude_fields, include_fields, "block.pruning_point"),
            block_timestamp: include_field(exclude_fields, include_fields, "block.timestamp"),
            block_utxo_commitment: include_field(exclude_fields, include_fields, "block.utxo_commitment"),
            block_version: include_field(exclude_fields, include_fields, "block.version"),
            tx_hash: include_field(exclude_fields, include_fields, "tx.hash"),
            tx_mass: include_field(exclude_fields, include_fields, "tx.mass"),
            tx_payload: include_field(exclude_fields, include_fields, "tx.payload"),
            tx_in_signature_script: include_field(exclude_fields, include_fields, "tx_in.signature_script"),
            tx_in_sig_op_count: include_field(exclude_fields, include_fields, "tx_in.sig_op_count"),
            tx_out_script_public_key_address: include_field(exclude_fields, include_fields, "tx_out.script_public_key_address"),
        }
    }

    pub fn map_block(&self, block: &RpcBlock) -> SqlBlock {
        blocks::map_block(
            block,
            self.block_accepted_id_merkle_root,
            self.block_merge_set_blues_hashes,
            self.block_merge_set_reds_hashes,
            self.block_selected_parent_hash,
            self.block_bits,
            self.block_blue_work,
            self.block_daa_score,
            self.block_hash_merkle_root,
            self.block_nonce,
            self.block_pruning_point,
            self.block_timestamp,
            self.block_utxo_commitment,
            self.block_version,
        )
    }

    pub fn map_block_parents(&self, block: &RpcBlock) -> Vec<SqlBlockParent> {
        blocks::map_block_parents(block)
    }

    pub fn map_block_transaction_ids(&self, block: &RpcBlock) -> Vec<SqlHash> {
        blocks::map_block_transaction_ids(block)
    }

    pub fn count_block_transactions(&self, block: &RpcBlock) -> usize {
        block.verbose_data.as_ref().expect("Block verbose_data is missing").transaction_ids.len()
    }

    pub fn map_transaction(&self, transaction: &RpcTransaction, subnetwork_key: i32) -> SqlTransaction {
        transactions::map_transaction(subnetwork_key, transaction, self.tx_hash, self.tx_mass, self.tx_payload)
    }

    pub fn map_block_transaction(&self, transaction: &RpcTransaction) -> SqlBlockTransaction {
        transactions::map_block_transaction(transaction)
    }

    pub fn map_transaction_inputs(&self, transaction: &RpcTransaction) -> Vec<SqlTransactionInput> {
        transactions::map_transaction_inputs(transaction, self.tx_in_signature_script, self.tx_in_sig_op_count)
    }

    pub fn map_transaction_outputs(&self, transaction: &RpcTransaction) -> Vec<SqlTransactionOutput> {
        transactions::map_transaction_outputs(transaction, self.tx_out_script_public_key_address)
    }

    pub fn map_transaction_outputs_address(&self, transaction: &RpcTransaction) -> Vec<SqlAddressTransaction> {
        transactions::map_transaction_outputs_address(transaction)
    }
}

pub fn include_field(exclude_fields: &Option<Vec<String>>, include_fields: &Option<Vec<String>>, field_name: &str) -> bool {
    if let Some(include_fields) = include_fields {
        include_fields.contains(&field_name.to_string())
    } else if let Some(exclude_fields) = exclude_fields {
        !exclude_fields.contains(&field_name.to_string())
    } else {
        true
    }
}
