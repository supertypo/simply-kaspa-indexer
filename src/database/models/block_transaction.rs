use crate::database::models::sql_hash::SqlHash;
use std::hash::Hash;

#[derive(Eq, PartialEq, Hash)]
pub struct BlockTransaction {
    pub block_hash: SqlHash,
    pub transaction_id: SqlHash,
}