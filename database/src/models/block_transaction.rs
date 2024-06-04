use std::hash::Hash;
use crate::models::sql_hash::SqlHash;

#[derive(Eq, PartialEq, Hash)]
pub struct BlockTransaction {
    pub block_hash: SqlHash,
    pub transaction_id: SqlHash,
}
