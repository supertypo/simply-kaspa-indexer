use crate::web::model::metrics::Metrics;
use crate::web::web_server;
use axum::response::IntoResponse;
use axum::{Extension, Json};
use bytesize::ByteSize;
use log::warn;
use simply_kaspa_database::client::KaspaDbClient;
use std::sync::Arc;
use std::time::Duration;
use std::{fs, process};
use sysinfo::{Pid, ProcessRefreshKind, System};
use tokio::sync::{RwLock, RwLockWriteGuard};
use tokio::time::sleep;

pub const PATH: &str = "/api/metrics";

#[utoipa::path(
    method(get),
    path = PATH,
    tag = web_server::INFO_TAG,
    description = "Get metrics",
    responses(
        (status = StatusCode::OK, description = "Success", body = Metrics, content_type = "application/json"),
    )
)]
pub async fn get_metrics(
    Extension(metrics): Extension<Arc<RwLock<Metrics>>>,
    Extension(system): Extension<Arc<RwLock<System>>>,
    Extension(database_client): Extension<KaspaDbClient>,
) -> impl IntoResponse {
    Json(update_metrics(metrics, system, database_client).await)
}

pub async fn update_metrics(metrics: Arc<RwLock<Metrics>>, system: Arc<RwLock<System>>, database_client: KaspaDbClient) -> Metrics {
    let mut metrics = metrics.write().await;
    let mut system = system.write().await;
    let pid = Pid::from_u32(process::id());

    refresh_system(&mut system, &pid);
    if metrics.process.cpu_used_percent == 0.0 {
        // CPU usage computation is based on time diff
        sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL).await;
        refresh_system(&mut system, &pid);
    }

    if let Some(process) = system.process(pid) {
        let uptime_seconds = process.run_time();
        metrics.process.cpu_used_percent = (process.cpu_usage() * 10.0).round() / 10.0;
        metrics.process.memory_used = process.memory();
        metrics.process.memory_used_pretty = Some(ByteSize(metrics.process.memory_used).to_string());
        metrics.process.memory_free = get_free_memory(&mut system);
        metrics.process.memory_free_pretty = Some(ByteSize(metrics.process.memory_free).to_string());
        metrics.process.uptime = uptime_seconds * 1000;
        metrics.process.uptime_pretty = Some(humantime::format_duration(Duration::from_secs(uptime_seconds)).to_string());
    }
    match database_client.select_database_details().await {
        Ok(database_details) => metrics.database = database_details.into(),
        Err(e) => warn!("Failed to select database details: {:?}", e),
    }
    match database_client.select_all_table_details().await {
        Ok(all_table_details) => metrics.database.tables = Some(all_table_details.into_iter().map(|td| td.into()).collect()),
        Err(e) => warn!("Failed to select all table details: {:?}", e),
    }
    metrics.clone()
}

fn refresh_system(system: &mut RwLockWriteGuard<System>, pid: &Pid) {
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::Some(&[*pid]),
        false,
        ProcessRefreshKind::nothing().with_cpu().with_memory(),
    );
}

fn get_free_memory(system: &mut RwLockWriteGuard<System>) -> u64 {
    if let Some(max) = fs::read_to_string("/sys/fs/cgroup/memory.max").ok().and_then(|max| max.trim().parse::<u64>().ok())
        && let Some(current) =
            fs::read_to_string("/sys/fs/cgroup/memory.current").ok().and_then(|current| current.trim().parse::<u64>().ok())
    {
        return max - current;
    };
    system.refresh_memory();
    let mut free_memory = system.available_memory();
    if free_memory == 0 {
        free_memory = system.free_memory()
    }
    free_memory
}
