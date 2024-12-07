use deadpool::managed::{Manager, Metrics, RecycleError, RecycleResult};
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_rpc_core::RpcNetworkType;
use kaspa_wrpc_client::client::ConnectOptions;
use kaspa_wrpc_client::error::Error;
use kaspa_wrpc_client::prelude::*;
use kaspa_wrpc_client::{KaspaRpcClient, WrpcEncoding};
use log::{debug, error};
use std::sync::Arc;
use std::time::Duration;

pub struct KaspadManager {
    pub network_id: NetworkId,
    pub rpc_url: Option<String>,
}

impl Manager for KaspadManager {
    type Type = Arc<KaspaRpcClient>;
    type Error = Error;

    async fn create(&self) -> Result<Self::Type, Self::Error> {
        debug!("Creating connection");
        Ok(Arc::new(connect_client(self.network_id.clone(), self.rpc_url.clone()).await?))
    }

    async fn recycle(&self, conn: &mut Self::Type, _: &Metrics) -> RecycleResult<Self::Error> {
        debug!("Recycling connection");
        if conn.get_server_info().await.map(|s| s.is_synced).unwrap_or(false) {
            Ok(())
        } else {
            Err(RecycleError::Message("Kaspad connection is not alive.".into()))
        }
    }
}

pub async fn connect_client(network_id: NetworkId, rpc_url: Option<String>) -> Result<KaspaRpcClient, Error> {

    let url = if let Some(url) = &rpc_url { url } else { &Resolver::default().get_url(WrpcEncoding::Borsh, network_id).await? };

    debug!("Connecting to Kaspad {}", url);
    let client = KaspaRpcClient::new_with_args(WrpcEncoding::Borsh, Some(url), None, Some(network_id), None)?;
    client.connect(Some(connect_options())).await?;

    let server_info = client.get_server_info().await?;
    let connected_network = format!(
        "{}{}",
        server_info.network_id.network_type,
        server_info.network_id.suffix.map(|s| format!("-{}", s.to_string())).unwrap_or_default()
    );
    debug!("Connected to Kaspad {} version: {}, network: {}", url, server_info.server_version, connected_network);

    if network_id != server_info.network_id {
        let err_msg = format!("Network mismatch, expected '{}', actual '{}'", network_id, connected_network);
        error!("{err_msg}");
        Err(Error::Custom(err_msg))
    } else if server_info.network_id.network_type == RpcNetworkType::Mainnet && server_info.virtual_daa_score < 76902846 {
        let err_msg = "Invalid network".to_string();
        error!("{err_msg}");
        Err(Error::Custom(err_msg))
    } else if !server_info.is_synced {
        let err_msg = format!("Kaspad {} is NOT synced", server_info.server_version);
        error!("{err_msg}");
        Err(Error::Custom(err_msg))
    } else {
        Ok(client)
    }
}

fn connect_options() -> ConnectOptions {
    ConnectOptions {
        block_async_connect: true,
        strategy: ConnectStrategy::Fallback,
        url: None,
        connect_timeout: Some(Duration::from_secs(5)),
        retry_interval: None,
    }
}
