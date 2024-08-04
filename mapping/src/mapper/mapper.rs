use kaspa_rpc_core::{RpcBlock, RpcTransaction};

use kaspa_database::models::address_transaction::AddressTransaction as SqlAddressTransaction;
use kaspa_database::models::block::Block as SqlBlock;
use kaspa_database::models::block_parent::BlockParent as SqlBlockParent;
use kaspa_database::models::block_transaction::BlockTransaction as SqlBlockTransaction;
use kaspa_database::models::transaction::Transaction as SqlTransaction;
use kaspa_database::models::transaction_input::TransactionInput as SqlTransactionInput;
use kaspa_database::models::transaction_output::TransactionOutput as SqlTransactionOutput;
use kaspa_database::models::types::hash::Hash as SqlHash;

use crate::mapper::{blocks, transactions};

#[derive(Clone)]
pub struct KaspaDbMapper {
    map_payload: bool,
}

impl KaspaDbMapper {
    pub fn new(map_payload: bool) -> KaspaDbMapper {
        KaspaDbMapper { map_payload }
    }

    pub fn map_block(&self, block: &RpcBlock) -> SqlBlock {
        blocks::map_block(block)
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

    pub fn map_transaction(&self, transaction: &RpcTransaction, subnetwork_key: i16) -> SqlTransaction {
        transactions::map_transaction(self.map_payload, subnetwork_key, transaction)
    }

    pub fn map_block_transaction(&self, transaction: &RpcTransaction) -> SqlBlockTransaction {
        transactions::map_block_transaction(transaction)
    }

    pub fn map_transaction_inputs(&self, transaction: &RpcTransaction) -> Vec<SqlTransactionInput> {
        transactions::map_transaction_inputs(transaction)
    }

    pub fn map_transaction_outputs(&self, transaction: &RpcTransaction) -> Vec<SqlTransactionOutput> {
        transactions::map_transaction_outputs(transaction)
    }

    pub fn map_transaction_outputs_address(&self, transaction: &RpcTransaction) -> Vec<SqlAddressTransaction> {
        transactions::map_transaction_outputs_address(transaction)
    }
}
