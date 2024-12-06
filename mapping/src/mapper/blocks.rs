use kaspa_rpc_core::RpcBlock;

use kaspa_database::models::block::Block as SqlBlock;
use kaspa_database::models::block_parent::BlockParent as SqlBlockParent;
use kaspa_database::models::types::hash::Hash as SqlHash;

pub fn map_block(block: &RpcBlock) -> SqlBlock {
    let verbose_data = block.verbose_data.as_ref().expect("Block verbose_data is missing");
    SqlBlock {
        hash: block.header.hash.into(),
        accepted_id_merkle_root: block.header.accepted_id_merkle_root.into(),
        merge_set_blues_hashes: verbose_data.merge_set_blues_hashes.iter().map(|v| v.to_owned().into()).collect(),
        merge_set_reds_hashes: verbose_data.merge_set_reds_hashes.iter().map(|v| v.to_owned().into()).collect(),
        selected_parent_hash: verbose_data.selected_parent_hash.into(),
        bits: block.header.bits as i64,
        blue_score: block.header.blue_score as i64,
        blue_work: block.header.blue_work.to_be_bytes_var(),
        daa_score: block.header.daa_score as i64,
        hash_merkle_root: block.header.hash_merkle_root.into(),
        nonce: block.header.nonce.to_be_bytes().to_vec(),
        pruning_point: block.header.pruning_point.into(),
        timestamp: block.header.timestamp as i64,
        utxo_commitment: block.header.utxo_commitment.into(),
        version: block.header.version as i16,
    }
}

pub fn map_block_parents(block: &RpcBlock) -> Vec<SqlBlockParent> {
    block.header.parents_by_level[0]
        .iter()
        .map(|v| SqlBlockParent { block_hash: block.header.hash.into(), parent_hash: v.to_owned().into() })
        .collect()
}

pub fn map_block_transaction_ids(block: &RpcBlock) -> Vec<SqlHash> {
    let verbose_data = block.verbose_data.as_ref().expect("Block verbose_data is missing");
    verbose_data.transaction_ids.iter().map(|t| t.to_owned().into()).collect()
}
