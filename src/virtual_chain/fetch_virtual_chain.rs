extern crate diesel;

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crossbeam_queue::ArrayQueue;
use diesel::{ExpressionMethods, PgConnection, QueryDsl, select};
use diesel::r2d2::{ConnectionManager, Pool};
use kaspa_rpc_core::{RpcBlock, RpcTransaction};
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_wrpc_client::KaspaRpcClient;
use log::{debug, trace, warn};
use log::info;
use tokio::time::sleep;
use crate::database::models::{Subnetwork, Var};
use crate::database::schema::{subnetworks, vars};
use crate::vars::vars::load_start_point;

pub async fn fetch_virtual_chains(kaspad_client: KaspaRpcClient,
                                  db_pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), ()> {

//    load_start_point(db_pool).await.unwrap();
    Ok(())
}
