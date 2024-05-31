use std::fs;

use sqlx::{Error, Pool, Postgres};

pub async fn execute_ddl_from_file(path: &str, pool: &Pool<Postgres>) -> Result<(), Error> {
    let ddl = fs::read_to_string(path)?;
    let statements = ddl.split(";").filter(|stmt| !stmt.trim().is_empty());

    for statement in statements {
        sqlx::query(statement).execute(pool).await?;
    }
    Ok(())
}
