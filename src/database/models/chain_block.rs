use std::hash::Hash;

#[derive(Eq, PartialEq, Hash)]
pub struct ChainBlock {
    pub block_hash: [u8; 32],
}
