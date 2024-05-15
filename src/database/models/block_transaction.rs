use std::hash::Hash;

use diesel::prelude::*;

#[derive(Queryable, Selectable, Insertable, Clone, Eq, PartialEq, Hash)]
#[diesel(table_name = crate::database::schema::blocks_transactions)]
#[diesel(primary_key(block_hash, transaction_id))]
pub struct BlockTransaction {
    pub block_hash: Vec<u8>,
    pub transaction_id: Vec<u8>,
}
