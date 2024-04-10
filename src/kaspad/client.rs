use std::time::Duration;
use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_wrpc_client::{KaspaRpcClient, WrpcEncoding};
use kaspa_wrpc_client::client::ConnectOptions;
use kaspa_wrpc_client::error::Error;
use log::{info, warn};
use tokio::time::sleep;

pub async fn connect_kaspad(url: String, force_network: String) -> Result<KaspaRpcClient, Error> {
    info!("Connecting to kaspad {}", url);
    let client = KaspaRpcClient::new(WrpcEncoding::Borsh, &url)?;
    client.connect(ConnectOptions::fallback()).await?;
    let server_info = client.get_server_info().await?;
    let network = format!("{}{}", server_info.network_id.network_type,
                          server_info.network_id.suffix.map(|s| format!("-{}", s.to_string())).unwrap_or_default());
    info!("Connected to kaspad version: {}, network: {}", server_info.server_version, network);

    if network != force_network {
        return Err(Error::Custom(format!("Network mismatch, expected '{}', actual '{}'", force_network, network)));
    }
    if !server_info.is_synced {
        while !client.get_server_info().await?.is_synced {
            warn!("Kaspad {} is NOT synced, retrying in 10 seconds...", server_info.server_version);
            sleep(Duration::from_secs(10)).await;
        }
    }
    return Ok(client);
}
