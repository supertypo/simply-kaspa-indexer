use std::hash::Hash;

use diesel::prelude::*;

#[derive(Queryable, Selectable, Insertable, Clone, Eq, PartialEq, Hash)]
#[diesel(table_name = crate::database::schema::chain_blocks)]
#[diesel(primary_key(block_hash))]
pub struct ChainBlock {
    pub block_hash: Vec<u8>,
}
