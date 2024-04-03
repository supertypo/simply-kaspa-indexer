use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_wrpc_client::{KaspaRpcClient, WrpcEncoding};
use kaspa_wrpc_client::client::ConnectOptions;
use kaspa_wrpc_client::error::Error;

pub async fn connect(url: &String, force_network: &Option<String>) -> Result<KaspaRpcClient, Error> {
    println!("Connecting to kaspad {}", url);
    let client = KaspaRpcClient::new(WrpcEncoding::Borsh, &url)?;
    client.connect(ConnectOptions::default()).await?;
    let server_info = client.get_server_info().await?;
    let network = format!("{}{}", server_info.network_id.network_type,
                          server_info.network_id.suffix.map(|s| format!("-{}", s.to_string())).unwrap_or_default());
    println!("Connected to kaspad version: {}, network: {}", server_info.server_version, network);

    if force_network.as_ref().map(|n| n.ne(&network)).unwrap_or_default() {
        return Err(Error::Custom("Network mismatch".to_string()));
    }
    if !server_info.is_synced {
        return Err(Error::Custom("Is not synced".to_string()));
    }
    return Ok(client);
}
