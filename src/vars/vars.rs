use diesel::{insert_into, Insertable, OptionalExtension, PgConnection, QueryDsl, RunQueryDsl};
use diesel::expression_methods::ExpressionMethods;
use diesel::r2d2::{ConnectionManager, Pool};
use log::{debug, warn};

use crate::database::models::{Var, VAR_KEY_START_BLOCK_HASH};
use crate::database::schema::vars;

pub fn load_start_block_hash(db_pool: Pool<ConnectionManager<PgConnection>>) -> Option<String> {
    load(String::from(VAR_KEY_START_BLOCK_HASH), db_pool)
}

pub fn save_start_block_hash(start_point: String, db_pool: Pool<ConnectionManager<PgConnection>>) {
    save(String::from(VAR_KEY_START_BLOCK_HASH), start_point, db_pool)
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
        debug!("Database var with key '{}' loaded: {}", key, value);
        Some(value)
    } else {
        warn!("Database var with key '{}' not found", key);
        None
    }
}

pub fn save(key: String, value: String, db_pool: Pool<ConnectionManager<PgConnection>>) {
    let con = &mut db_pool.get().expect("Database connection FAILED");
    insert_into(vars::dsl::vars)
        .values(Var { key, value })
        .execute(con)
        .expect("Saving var to database FAILED");
}