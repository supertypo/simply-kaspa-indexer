use std::hash::Hash;
use crate::models::sql_hash::SqlHash;

#[derive(Eq, PartialEq, Hash)]
pub struct ChainBlock {
    pub block_hash: SqlHash,
}
