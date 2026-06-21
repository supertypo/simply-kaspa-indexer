use sqlx::{AssertSqlSafe, Error, Pool, Postgres};

pub async fn execute_ddl(ddl: &str, pool: &Pool<Postgres>) -> Result<(), Error> {
    sqlx::raw_sql(AssertSqlSafe(ddl)).execute(pool).await?;
    Ok(())
}
