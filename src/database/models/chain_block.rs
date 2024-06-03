use crate::database::models::sql_hash::SqlHash;
use std::hash::Hash;

#[derive(Eq, PartialEq, Hash)]
pub struct ChainBlock {
    pub block_hash: SqlHash,
}
