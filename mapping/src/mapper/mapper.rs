use kaspa_rpc_core::RpcBlock;

use kaspa_database::models::block::Block as DbBlock;
use kaspa_database::models::block_parent::BlockParent;
use kaspa_database::models::types::hash::Hash as SqlHash;

use crate::mapper::blocks;

#[derive(Clone)]
pub struct KaspaDbMapper {}

impl KaspaDbMapper {
    pub fn new() -> KaspaDbMapper {
        KaspaDbMapper {}
    }

    pub fn map_block(&self, block: &RpcBlock) -> DbBlock {
        blocks::map_block(block)
    }

    pub fn map_block_parents(&self, block: &RpcBlock) -> Vec<BlockParent> {
        blocks::map_block_parents(block)
    }

    pub fn map_block_transaction_ids(&self, block: &RpcBlock) -> Vec<SqlHash> {
        blocks::map_block_transaction_ids(block)
    }

    pub fn count_transactions(&self, block: &RpcBlock) -> usize {
        block.verbose_data.as_ref().expect("Block verbose_data is missing").transaction_ids.len()
    }
}
