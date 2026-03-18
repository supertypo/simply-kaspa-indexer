use sqlx::{Error, Pool, Postgres};

pub async fn execute_ddl(ddl: &str, pool: &Pool<Postgres>) -> Result<(), Error> {
    sqlx::raw_sql(ddl).execute(pool).await?;
    Ok(())
}
