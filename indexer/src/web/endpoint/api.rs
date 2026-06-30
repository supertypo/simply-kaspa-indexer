use crate::web::model::api::{
    AddressBalance, AddressTransactionSummary, AddressUtxo, BlockSummary, LimitQuery, SearchQuery, SearchResult, Status,
    TransactionSummary,
};
use crate::web::model::metrics::Metrics;
use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Extension, Json};
use kaspa_hashes::Hash as KaspaHash;
use simply_kaspa_database::client::KaspaDbClient;
use simply_kaspa_database::models::types::hash::Hash;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;

pub const RECENT_BLOCKS_PATH: &str = "/api/blocks/recent";
pub const BLOCK_PATH: &str = "/api/blocks/{hash}";
pub const TRANSACTION_PATH: &str = "/api/transactions/{transaction_id}";
pub const ADDRESS_TRANSACTIONS_PATH: &str = "/api/addresses/{address}/transactions";
pub const ADDRESS_BALANCE_PATH: &str = "/api/addresses/{address}/balance";
pub const ADDRESS_UTXOS_PATH: &str = "/api/addresses/{address}/utxos";
pub const SEARCH_PATH: &str = "/api/search";
pub const STATUS_PATH: &str = "/api/status";

#[utoipa::path(
    method(get),
    path = RECENT_BLOCKS_PATH,
    params(LimitQuery),
    responses((status = StatusCode::OK, body = Vec<BlockSummary>, content_type = "application/json"))
)]
pub async fn get_recent_blocks(
    Query(query): Query<LimitQuery>,
    Extension(database_client): Extension<KaspaDbClient>,
) -> impl IntoResponse {
    match database_client.select_recent_blocks(normalize_limit(query.limit, 50, 250)).await {
        Ok(blocks) => (StatusCode::OK, Json(blocks.into_iter().map(BlockSummary::from).collect::<Vec<_>>())).into_response(),
        Err(error) => api_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

#[utoipa::path(
    method(get),
    path = BLOCK_PATH,
    params(("hash" = String, Path, description = "Block hash")),
    responses(
        (status = StatusCode::OK, body = BlockSummary, content_type = "application/json"),
        (status = StatusCode::BAD_REQUEST, description = "Invalid hash"),
        (status = StatusCode::NOT_FOUND, description = "Block not found")
    )
)]
pub async fn get_block(Path(hash): Path<String>, Extension(database_client): Extension<KaspaDbClient>) -> impl IntoResponse {
    let hash = match parse_hash(&hash) {
        Ok(hash) => hash,
        Err(error) => return api_error(StatusCode::BAD_REQUEST, error),
    };
    match database_client.select_block(&hash).await {
        Ok(Some(block)) => (StatusCode::OK, Json(BlockSummary::from(block))).into_response(),
        Ok(None) => api_error(StatusCode::NOT_FOUND, "block not found"),
        Err(error) => api_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

#[utoipa::path(
    method(get),
    path = TRANSACTION_PATH,
    params(("transaction_id" = String, Path, description = "Transaction id")),
    responses(
        (status = StatusCode::OK, body = TransactionSummary, content_type = "application/json"),
        (status = StatusCode::BAD_REQUEST, description = "Invalid transaction id"),
        (status = StatusCode::NOT_FOUND, description = "Transaction not found")
    )
)]
pub async fn get_transaction(
    Path(transaction_id): Path<String>,
    Extension(database_client): Extension<KaspaDbClient>,
) -> impl IntoResponse {
    let transaction_id = match parse_hash(&transaction_id) {
        Ok(hash) => hash,
        Err(error) => return api_error(StatusCode::BAD_REQUEST, error),
    };
    match database_client.select_transaction(&transaction_id).await {
        Ok(Some(transaction)) => (StatusCode::OK, Json(TransactionSummary::from(transaction))).into_response(),
        Ok(None) => api_error(StatusCode::NOT_FOUND, "transaction not found"),
        Err(error) => api_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

#[utoipa::path(
    method(get),
    path = ADDRESS_TRANSACTIONS_PATH,
    params(("address" = String, Path, description = "Kaspa address"), LimitQuery),
    responses((status = StatusCode::OK, body = Vec<AddressTransactionSummary>, content_type = "application/json"))
)]
pub async fn get_address_transactions(
    Path(address): Path<String>,
    Query(query): Query<LimitQuery>,
    Extension(database_client): Extension<KaspaDbClient>,
) -> impl IntoResponse {
    match database_client.select_address_transactions(&address, normalize_limit(query.limit, 50, 500)).await {
        Ok(transactions) => {
            (StatusCode::OK, Json(transactions.into_iter().map(AddressTransactionSummary::from).collect::<Vec<_>>())).into_response()
        }
        Err(error) => api_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

#[utoipa::path(
    method(get),
    path = ADDRESS_BALANCE_PATH,
    params(("address" = String, Path, description = "Kaspa address")),
    responses((status = StatusCode::OK, body = AddressBalance, content_type = "application/json"))
)]
pub async fn get_address_balance(
    Path(address): Path<String>,
    Extension(database_client): Extension<KaspaDbClient>,
) -> impl IntoResponse {
    match database_client.select_address_balance(&address).await {
        Ok(balance) => (StatusCode::OK, Json(AddressBalance::from(balance))).into_response(),
        Err(error) => api_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

#[utoipa::path(
    method(get),
    path = ADDRESS_UTXOS_PATH,
    params(("address" = String, Path, description = "Kaspa address"), LimitQuery),
    responses((status = StatusCode::OK, body = Vec<AddressUtxo>, content_type = "application/json"))
)]
pub async fn get_address_utxos(
    Path(address): Path<String>,
    Query(query): Query<LimitQuery>,
    Extension(database_client): Extension<KaspaDbClient>,
) -> impl IntoResponse {
    match database_client.select_address_utxos(&address, normalize_limit(query.limit, 100, 1000)).await {
        Ok(utxos) => (StatusCode::OK, Json(utxos.into_iter().map(AddressUtxo::from).collect::<Vec<_>>())).into_response(),
        Err(error) => api_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

#[utoipa::path(
    method(get),
    path = SEARCH_PATH,
    params(SearchQuery),
    responses((status = StatusCode::OK, body = Vec<SearchResult>, content_type = "application/json"))
)]
pub async fn get_search(Query(query): Query<SearchQuery>, Extension(database_client): Extension<KaspaDbClient>) -> impl IntoResponse {
    match database_client.select_search(&query.q, normalize_limit(query.limit, 10, 50)).await {
        Ok(results) => (StatusCode::OK, Json(results.into_iter().map(SearchResult::from).collect::<Vec<_>>())).into_response(),
        Err(error) => api_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

#[utoipa::path(
    method(get),
    path = STATUS_PATH,
    responses((status = StatusCode::OK, body = Status, content_type = "application/json"))
)]
pub async fn get_status(
    Extension(metrics): Extension<Arc<RwLock<Metrics>>>,
    Extension(database_client): Extension<KaspaDbClient>,
) -> impl IntoResponse {
    let metrics = metrics.read().await.clone();
    let chain_tip = match database_client.select_recent_blocks(1).await {
        Ok(mut blocks) => blocks.pop().map(BlockSummary::from),
        Err(error) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    };
    let status = Status {
        chain_tip,
        checkpoint_hash: metrics.checkpoint.block.as_ref().map(|block| block.hash.clone()),
        checkpoint_daa_score: metrics.checkpoint.block.as_ref().map(|block| block.daa_score),
        virtual_chain_tip_distance: metrics.components.virtual_chain_processor.tip_distance,
        transaction_processor_enabled: metrics.components.transaction_processor.enabled,
        virtual_chain_processor_enabled: metrics.components.virtual_chain_processor.enabled,
    };
    (StatusCode::OK, Json(status)).into_response()
}

fn parse_hash(value: &str) -> Result<Hash, String> {
    KaspaHash::from_str(value).map(Hash::from).map_err(|_| "invalid kaspa hash".to_string())
}

fn normalize_limit(limit: Option<i64>, default: i64, max: i64) -> i64 {
    limit.unwrap_or(default).clamp(1, max)
}

fn api_error(status: StatusCode, message: impl ToString) -> axum::response::Response {
    (status, Json(serde_json::json!({ "error": message.to_string() }))).into_response()
}
