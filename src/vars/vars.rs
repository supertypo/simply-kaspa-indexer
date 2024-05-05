use diesel::{insert_into, OptionalExtension, PgConnection, QueryDsl, RunQueryDsl};
use diesel::expression_methods::ExpressionMethods;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::upsert::excluded;
use log::trace;

use crate::database::models::{Var, VAR_KEY_BLOCK_CHECKPOINT, VAR_KEY_LEGACY_CHECKPOINT, VAR_KEY_VIRTUAL_CHECKPOINT};
use crate::database::schema::vars;

pub fn load_block_checkpoint(db_pool: Pool<ConnectionManager<PgConnection>>) -> Option<String> {
    load(String::from(VAR_KEY_BLOCK_CHECKPOINT), db_pool.clone())
        .or_else(|| load(String::from(VAR_KEY_LEGACY_CHECKPOINT), db_pool))
}

pub fn load_virtual_checkpoint(db_pool: Pool<ConnectionManager<PgConnection>>) -> Option<String> {
    load(String::from(VAR_KEY_VIRTUAL_CHECKPOINT), db_pool.clone())
        .or_else(|| load(String::from(VAR_KEY_LEGACY_CHECKPOINT), db_pool))
}

pub fn save_block_checkpoint(start_point: String, db_pool: Pool<ConnectionManager<PgConnection>>) {
    let con = &mut db_pool.get().expect("Database connection FAILED");
    save(String::from(VAR_KEY_BLOCK_CHECKPOINT), start_point, con)
}

pub fn save_virtual_checkpoint(start_point: String, con: &mut PooledConnection<ConnectionManager<PgConnection>>) {
    save(String::from(VAR_KEY_VIRTUAL_CHECKPOINT), start_point, con)
}

pub fn load(key: String, db_pool: Pool<ConnectionManager<PgConnection>>) -> Option<String> {
    let con = &mut db_pool.get().expect("Database connection FAILED");
    let option = vars::dsl::vars
        .select(vars::value)
        .filter(vars::key.eq(key.clone()))
        .first::<String>(con)
        .optional()
        .expect(format!("Loading var '{}' from database FAILED", key).as_str());
    if option.is_some() {
        let value = option.unwrap();
        trace!("Database var with key '{}' loaded: {}", key, value);
        Some(value)
    } else {
        trace!("Database var with key '{}' not found", key);
        None
    }
}

pub fn save(key: String, value: String, con: &mut PooledConnection<ConnectionManager<PgConnection>>) {
    trace!("Saving database var with key '{}' value: {}", key, value);
    insert_into(vars::dsl::vars)
        .values(Var { key, value })
        .on_conflict(vars::key)
        .do_update()
        .set(vars::value.eq(excluded(vars::value)))
        .execute(con)
        .expect("Saving var FAILED");
}
