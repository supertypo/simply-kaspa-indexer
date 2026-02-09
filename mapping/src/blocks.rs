use kaspa_rpc_core::RpcBlock;

use simply_kaspa_database::models::block::Block as SqlBlock;
use simply_kaspa_database::models::block_parent::BlockParent as SqlBlockParent;
use simply_kaspa_database::models::types::hash::Hash as SqlHash;

pub fn map_block(
    block: &RpcBlock,
    include_accepted_id_merkle_root: bool,
    include_merge_set_blues_hashes: bool,
    include_merge_set_reds_hashes: bool,
    include_selected_parent_hash: bool,
    include_bits: bool,
    include_blue_work: bool,
    include_blue_score: bool,
    include_daa_score: bool,
    include_hash_merkle_root: bool,
    include_nonce: bool,
    include_pruning_point: bool,
    include_timestamp: bool,
    include_utxo_commitment: bool,
    include_version: bool,
) -> SqlBlock {
    let verbose_data = block.verbose_data.as_ref().expect("Block verbose_data is missing");
    SqlBlock {
        hash: block.header.hash.into(),
        accepted_id_merkle_root: include_accepted_id_merkle_root.then_some(block.header.accepted_id_merkle_root.into()),
        merge_set_blues_hashes: (include_merge_set_blues_hashes && !verbose_data.merge_set_blues_hashes.is_empty())
            .then_some(verbose_data.merge_set_blues_hashes.iter().map(|v| v.to_owned().into()).collect()),
        merge_set_reds_hashes: (include_merge_set_reds_hashes && !verbose_data.merge_set_reds_hashes.is_empty())
            .then_some(verbose_data.merge_set_reds_hashes.iter().map(|v| v.to_owned().into()).collect()),
        selected_parent_hash: include_selected_parent_hash.then_some(verbose_data.selected_parent_hash.into()),
        bits: include_bits.then_some(block.header.bits as i64),
        blue_score: include_blue_score.then_some(block.header.blue_score as i64),
        blue_work: include_blue_work.then_some(block.header.blue_work.to_be_bytes_var()),
        daa_score: include_daa_score.then_some(block.header.daa_score as i64),
        hash_merkle_root: include_hash_merkle_root.then_some(block.header.hash_merkle_root.into()),
        nonce: include_nonce.then_some(block.header.nonce.to_be_bytes().to_vec()),
        pruning_point: include_pruning_point.then_some(block.header.pruning_point.into()),
        timestamp: include_timestamp.then_some(block.header.timestamp as i64),
        utxo_commitment: include_utxo_commitment.then_some(block.header.utxo_commitment.into()),
        version: include_version.then_some(block.header.version as i16),
    }
}

pub fn map_block_parents(block: &RpcBlock) -> Vec<SqlBlockParent> {
    if block.header.parents_by_level.len() == 0 {
        return vec![];
    }
    block.header.parents_by_level[0]
        .iter()
        .map(|v| SqlBlockParent { block_hash: block.header.hash.into(), parent_hash: v.to_owned().into() })
        .collect()
}

pub fn map_block_transaction_ids(block: &RpcBlock) -> Vec<SqlHash> {
    let verbose_data = block.verbose_data.as_ref().expect("Block verbose_data is missing");
    verbose_data.transaction_ids.iter().map(|t| t.to_owned().into()).collect()
}
