use std::hash::Hash;
use crate::models::sql_hash::SqlHash;

#[derive(Eq, PartialEq, Hash)]
pub struct TransactionAcceptance {
    pub transaction_id: SqlHash,
    pub block_hash: SqlHash,
}
