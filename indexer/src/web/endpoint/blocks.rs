use crate::utils::fifo_cache::FifoCache;
use crate::web::web_server;
use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Extension, Json};
use kaspa_rpc_core::RpcBlock;
use serde::Deserialize;
use std::sync::Arc;

pub const PATH: &str = "/api/blocks";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlocksQuery {
    pub blue_score: u64,
}

#[utoipa::path(
    method(get),
    path = PATH,
    tag = web_server::BLOCKS_TAG,
    description = "Get blocks by blue score",
    params(("blueScore" = u64, Query, description = "Get blocks by blue score from indexer cache, only recent blocks are available")),
    responses((status = StatusCode::OK, description = "Success", content_type = "application/json"))
)]
pub async fn get_blocks(
    Query(query): Query<BlocksQuery>,
    Extension(block_store): Extension<Arc<FifoCache<u64, Vec<RpcBlock>>>>,
    Extension(block_store_ttl): Extension<u64>,
) -> impl IntoResponse {
    if block_store_ttl == 0 {
        return (StatusCode::SERVICE_UNAVAILABLE, "Blocks endpoint is disabled (block_store_ttl is 0)").into_response();
    }
    let blocks = block_store.get(&query.blue_score).await.unwrap_or_default();
    Json(blocks).into_response()
}
