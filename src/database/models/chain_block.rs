use std::hash::Hash;

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct ChainBlock {
    pub block_hash: Vec<u8>,
}
