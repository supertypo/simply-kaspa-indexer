use sqlx::{Error, Pool, Postgres};

pub async fn execute_ddl(ddl: &str, pool: &Pool<Postgres>) -> Result<(), Error> {
    for statement in ddl.split(";").filter(|stmt| !stmt.trim().is_empty()) {
        sqlx::query(statement).execute(pool).await?;
    }
    Ok(())
}
