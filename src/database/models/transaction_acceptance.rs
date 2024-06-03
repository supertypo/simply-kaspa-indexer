use crate::database::models::sql_hash::SqlHash;
use std::hash::Hash;

#[derive(Eq, PartialEq, Hash)]
pub struct TransactionAcceptance {
    pub transaction_id: SqlHash,
    pub block_hash: SqlHash,
}
