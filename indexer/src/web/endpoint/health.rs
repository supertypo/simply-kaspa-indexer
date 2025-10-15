use crate::web::endpoint::metrics::update_metrics;
use crate::web::model::health::{Health, HealthIndexer, HealthIndexerDetails, HealthIndexerInfo, HealthKaspad, HealthStatus};
use crate::web::model::metrics::{Metrics, MetricsBlock};
use crate::web::web_server;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Extension, Json};
use chrono::{DateTime, Utc};
use deadpool::managed::Pool;
use kaspa_rpc_core::api::rpc::RpcApi;
use simply_kaspa_database::client::KaspaDbClient;
use simply_kaspa_kaspad::manager::KaspadManager;
use std::sync::Arc;
use std::time::Duration;
use sysinfo::System;
use tokio::sync::RwLock;

pub const PATH: &str = "/api/health";

#[utoipa::path(
    method(get),
    path = PATH,
    tag = web_server::INFO_TAG,
    description = "Get health details",
    responses(
        (status = StatusCode::OK, description = "Success", body = Health, content_type = "application/json"),
        (status = StatusCode::SERVICE_UNAVAILABLE, description = "Failed", body = Health, content_type = "application/json")
    )
)]
pub async fn get_health(
    Extension(metrics): Extension<Arc<RwLock<Metrics>>>,
    Extension(kaspad_pool): Extension<Pool<KaspadManager>>,
    Extension(system): Extension<Arc<RwLock<System>>>,
    Extension(database_client): Extension<KaspaDbClient>,
) -> impl IntoResponse {
    let health_kaspad: HealthKaspad = match kaspad_pool.get().await {
        Ok(kaspad_client) => match kaspad_client.get_server_info().await {
            Ok(server_info) => server_info.into(),
            Err(e) => (HealthStatus::DOWN, e.to_string()).into(),
        },
        Err(e) => (HealthStatus::DOWN, e.to_string()).into(),
    };
    let metrics = update_metrics(metrics, system, database_client).await;

    let health_indexer = indexer_health(metrics, health_kaspad.virtual_daa_score).await;

    let mut status = health_indexer.status.clone();
    if health_kaspad.status == HealthStatus::DOWN {
        status = HealthStatus::DOWN;
    }

    let health = Health { status, last_updated: Utc::now().timestamp_millis() as u64, indexer: health_indexer, kaspad: health_kaspad };
    let status_code = if health.status != HealthStatus::DOWN { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE };
    (status_code, Json(&health)).into_response()
}

async fn indexer_health(metrics: Metrics, current_daa: Option<u64>) -> HealthIndexer {
    let mut health = HealthIndexer {
        status: HealthStatus::UP,
        info: HealthIndexerInfo {
            name: metrics.name,
            version: metrics.version,
            commit_id: metrics.commit_id,
            uptime: metrics.process.uptime_pretty,
        },
        details: None,
    };
    let mut health_details = vec![];

    health_details.push(HealthIndexerDetails {
        name: "process.memory_free".to_string(),
        status: if metrics.process.memory_free > 524288000 { HealthStatus::UP } else { HealthStatus::WARN },
        reason: format!("Free memory: {}", metrics.process.memory_free_pretty.as_ref().unwrap()),
    });

    let queue_utilization = percent_allocation(metrics.queues.blocks, metrics.queues.blocks_capacity);
    health_details.push(HealthIndexerDetails {
        name: "queue.blocks".to_string(),
        status: if queue_utilization < 90 { HealthStatus::UP } else { HealthStatus::WARN },
        reason: format!("Utilization: {}%", queue_utilization),
    });
    let queue_utilization = percent_allocation(metrics.queues.transactions, metrics.queues.transactions_capacity);
    health_details.push(HealthIndexerDetails {
        name: "queue.transactions".to_string(),
        status: if queue_utilization < 90 { HealthStatus::UP } else { HealthStatus::WARN },
        reason: format!("Utilization: {}%", queue_utilization),
    });

    let net_bps = metrics.settings.as_ref().map(|s| s.net_bps as u64).unwrap_or(10);
    health_details.push(indexer_details("checkpoint".to_string(), net_bps, current_daa, 120, 600, metrics.checkpoint.block.as_ref()));

    if metrics.components.utxo_importer.enabled {
        let completed = metrics.components.utxo_importer.completed.unwrap_or(false);
        let utxos = metrics.components.utxo_importer.utxos_imported.unwrap_or(0);
        health_details.push(HealthIndexerDetails {
            name: "component.utxo_importer".to_string(),
            status: if completed { HealthStatus::UP } else { HealthStatus::WARN },
            reason: format!("{} ({utxos} utxos imported)", if completed { "Completed" } else { "In progress" }),
        });
    } else {
        health_details.push(HealthIndexerDetails {
            name: "component.utxo_importer".to_string(),
            status: HealthStatus::UP,
            reason: "Disabled".to_string(),
        });
    }

    health_details.push(indexer_details(
        "component.block_fetcher".to_string(),
        net_bps,
        current_daa,
        60,
        600,
        metrics.components.block_fetcher.last_block.as_ref(),
    ));

    health_details.push(indexer_details(
        "component.block_processor".to_string(),
        net_bps,
        current_daa,
        60,
        600,
        metrics.components.block_processor.last_block.as_ref(),
    ));

    if metrics.components.transaction_processor.enabled {
        health_details.push(indexer_details(
            "component.transaction_processor".to_string(),
            net_bps,
            current_daa,
            60,
            600,
            metrics.components.transaction_processor.last_block.as_ref(),
        ));
    }

    if metrics.components.virtual_chain_processor.enabled {
        health_details.push(indexer_details(
            "component.virtual_chain_processor".to_string(),
            net_bps,
            current_daa,
            60,
            600,
            metrics.components.virtual_chain_processor.last_block.as_ref(),
        ));
    }

    if metrics.components.db_pruner.enabled {
        let ok = metrics.components.db_pruner.completed_successfully;
        let start_time = metrics.components.db_pruner.start_time.unwrap_or(DateTime::UNIX_EPOCH);
        health_details.push(HealthIndexerDetails {
            name: "component.db_pruner".to_string(),
            status: ok.map(|ok| if ok { HealthStatus::UP } else { HealthStatus::WARN }).unwrap_or(HealthStatus::UP),
            reason: ok
                .map(|ok| format!("{}: {}", start_time, if ok { "Success" } else { "Failure" }))
                .unwrap_or("Not yet run".to_string()),
        });
    } else {
        health_details.push(HealthIndexerDetails {
            name: "component.db_pruner".to_string(),
            status: HealthStatus::UP,
            reason: "Disabled".to_string(),
        });
    }

    if health_details.iter().any(|h| h.status == HealthStatus::DOWN) {
        health.status = HealthStatus::DOWN;
    } else if health_details.iter().any(|h| h.status == HealthStatus::WARN) {
        health.status = HealthStatus::WARN;
    }
    health.details = Some(health_details);
    health
}

fn percent_allocation(alloc: u64, cap: u64) -> u32 {
    ((alloc as f32 / cap as f32) * 100.0).round() as u32
}

fn indexer_details(
    component: String,
    net_bps: u64,
    current_daa: Option<u64>,
    warn_lag_seconds: u64,
    down_lag_seconds: u64,
    block: Option<&MetricsBlock>,
) -> HealthIndexerDetails {
    let daa_lag_seconds = block
        .map(|b| b.daa_score)
        .and_then(|component_daa| current_daa.map(|current_daa| current_daa.saturating_sub(component_daa)))
        .map(|component_daa_lag| component_daa_lag / net_bps);
    let time_lag_seconds = block
        .map(|b| b.timestamp)
        .map(|component_timestamp| ((Utc::now().timestamp_millis() as u64).saturating_sub(component_timestamp)) / 1000);

    let status = daa_lag_seconds
        .or(time_lag_seconds)
        .map(|lag| {
            if lag < warn_lag_seconds {
                HealthStatus::UP
            } else if lag < down_lag_seconds {
                HealthStatus::WARN
            } else {
                HealthStatus::DOWN
            }
        })
        .unwrap_or(HealthStatus::DOWN);

    HealthIndexerDetails {
        name: component.to_string(),
        status,
        reason: daa_lag_seconds
            .map(|lag| format!("{} behind", humantime::format_duration(Duration::from_secs(lag))))
            .or(time_lag_seconds.map(|lag| format!("{} behind", humantime::format_duration(Duration::from_secs(lag)))))
            .unwrap_or("No data".to_string())
            .to_string(),
    }
}
