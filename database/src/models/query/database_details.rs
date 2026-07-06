#[derive(Clone, sqlx::FromRow)]
pub struct DatabaseDetails {
    pub database_name: String,
    pub schema_name: String,
    pub database_size: i64,
    pub active_queries: i64,
    pub blocked_queries: i64,
    pub active_connections: i64,
    pub max_connections: i32,
    pub deadlocks: i64,
}
