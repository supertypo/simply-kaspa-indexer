use kaspa_rpc_core::{RpcBlock, RpcTransaction};
use simply_kaspa_cli::cli_args::{CliArgs, CliDisable, CliField};
use simply_kaspa_database::models::address_transaction::AddressTransaction as SqlAddressTransaction;
use simply_kaspa_database::models::block::Block as SqlBlock;
use simply_kaspa_database::models::block_parent::BlockParent as SqlBlockParent;
use simply_kaspa_database::models::block_transaction::BlockTransaction as SqlBlockTransaction;
use simply_kaspa_database::models::script_transaction::ScriptTransaction as SqlScriptTransaction;
use simply_kaspa_database::models::transaction::Transaction as SqlTransaction;
use simply_kaspa_database::models::types::hash::Hash as SqlHash;

use crate::{blocks, transactions};

#[derive(Clone)]
pub struct KaspaDbMapper {
    block_accepted_id_merkle_root: bool,
    block_merge_set_blues_hashes: bool,
    block_merge_set_reds_hashes: bool,
    block_selected_parent_hash: bool,
    block_bits: bool,
    block_blue_work: bool,
    block_blue_score: bool,
    block_daa_score: bool,
    block_hash_merkle_root: bool,
    block_nonce: bool,
    block_pruning_point: bool,
    block_timestamp: bool,
    block_utxo_commitment: bool,
    block_version: bool,
    tx_subnetwork_id: bool,
    tx_hash: bool,
    tx_mass: bool,
    tx_payload: bool,
    tx_block_time: bool,
    tx_in: bool,
    tx_in_previous_outpoint: bool,
    tx_in_signature_script: bool,
    tx_in_sig_op_count: bool,
    tx_out: bool,
    tx_out_amount: bool,
    tx_out_script_public_key: bool,
    tx_out_script_public_key_address: bool,
}

impl KaspaDbMapper {
    pub fn new(cli_args: CliArgs) -> KaspaDbMapper {
        KaspaDbMapper {
            block_accepted_id_merkle_root: !cli_args.is_excluded(CliField::BlockAcceptedIdMerkleRoot),
            block_merge_set_blues_hashes: !cli_args.is_excluded(CliField::BlockMergeSetBluesHashes),
            block_merge_set_reds_hashes: !cli_args.is_excluded(CliField::BlockMergeSetRedsHashes),
            block_selected_parent_hash: !cli_args.is_excluded(CliField::BlockSelectedParentHash),
            block_bits: !cli_args.is_excluded(CliField::BlockBits),
            block_blue_work: !cli_args.is_excluded(CliField::BlockBlueWork),
            block_blue_score: !cli_args.is_excluded(CliField::BlockBlueScore),
            block_daa_score: !cli_args.is_excluded(CliField::BlockDaaScore),
            block_hash_merkle_root: !cli_args.is_excluded(CliField::BlockHashMerkleRoot),
            block_nonce: !cli_args.is_excluded(CliField::BlockNonce),
            block_pruning_point: !cli_args.is_excluded(CliField::BlockPruningPoint),
            block_timestamp: !cli_args.is_excluded(CliField::BlockTimestamp),
            block_utxo_commitment: !cli_args.is_excluded(CliField::BlockUtxoCommitment),
            block_version: !cli_args.is_excluded(CliField::BlockVersion),
            tx_subnetwork_id: !cli_args.is_excluded(CliField::TxSubnetworkId),
            tx_hash: !cli_args.is_excluded(CliField::TxHash),
            tx_mass: !cli_args.is_excluded(CliField::TxMass),
            tx_payload: !cli_args.is_excluded(CliField::TxPayload),
            tx_block_time: !cli_args.is_excluded(CliField::TxBlockTime),
            tx_in: !cli_args.is_disabled(CliDisable::TransactionsInputsTable),
            tx_in_previous_outpoint: !cli_args.is_excluded(CliField::TxInPreviousOutpoint),
            tx_in_signature_script: !cli_args.is_excluded(CliField::TxInSignatureScript),
            tx_in_sig_op_count: !cli_args.is_excluded(CliField::TxInSigOpCount),
            tx_out: !cli_args.is_disabled(CliDisable::TransactionsOutputsTable),
            tx_out_amount: !cli_args.is_excluded(CliField::TxOutAmount),
            tx_out_script_public_key: !cli_args.is_excluded(CliField::TxOutScriptPublicKey),
            tx_out_script_public_key_address: !cli_args.is_excluded(CliField::TxOutScriptPublicKeyAddress),
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
            self.block_blue_score,
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
        transactions::map_transaction(
            subnetwork_key,
            transaction,
            self.tx_subnetwork_id,
            self.tx_hash,
            self.tx_mass,
            self.tx_payload,
            self.tx_block_time,
            self.tx_in,
            self.tx_in_previous_outpoint,
            self.tx_in_signature_script,
            self.tx_in_sig_op_count,
            self.tx_out,
            self.tx_out_amount,
            self.tx_out_script_public_key,
            self.tx_out_script_public_key_address,
        )
    }

    pub fn map_block_transaction(&self, transaction: &RpcTransaction) -> SqlBlockTransaction {
        transactions::map_block_transaction(transaction)
    }

    pub fn map_transaction_outputs_address(&self, transaction: &RpcTransaction) -> Vec<SqlAddressTransaction> {
        transactions::map_transaction_outputs_address(transaction)
    }

    pub fn map_transaction_outputs_script(&self, transaction: &RpcTransaction) -> Vec<SqlScriptTransaction> {
        transactions::map_transaction_outputs_script(transaction)
    }
}
