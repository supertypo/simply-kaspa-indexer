use diesel::prelude::*;

pub const VAR_KEY_BLOCK_CHECKPOINT: &str = "block_checkpoint";
pub const VAR_KEY_LEGACY_CHECKPOINT: &str = "vspc_last_start_hash";

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::database::schema::vars)]
#[diesel(primary_key(key))]
pub struct Var {
    pub key: String,
    pub value: String,
}
