use std::fmt::Debug;
use std::future::Future;
use std::str::FromStr;
use std::time::Duration;

use kaspa_rpc_core::api::rpc::RpcApi;
use kaspa_rpc_core::RpcNetworkType;
use kaspa_wrpc_client::client::ConnectOptions;
use kaspa_wrpc_client::error::Error;
use kaspa_wrpc_client::{KaspaRpcClient, WrpcEncoding};
use log::{error, info, warn};

use crate::cli::cli_args::CliArgs;
use kaspa_wrpc_client::prelude::*;
use tokio::time::sleep;

pub async fn with_retry<F, Fut, T, E>(mut f: F) -> Result<T, E>
where
    F: FnMut() -> Fut + Send,
    Fut: Future<Output = Result<T, E>> + Send,
    T: Send,
    E: Debug,
{
    const MAX_RETRIES: usize = 10;
    const RETRY_INTERVAL: u64 = 3000;
    for i in 0..MAX_RETRIES {
        let rpc_result = f().await;
        if rpc_result.is_ok() {
            return rpc_result;
        } else if i == MAX_RETRIES - 1 {
            if let Err(ref err) = rpc_result {
                error!("Function still failing after {} retries: {:?}", MAX_RETRIES, err);
            }
            return rpc_result;
        } else {
            if let Err(ref err) = rpc_result {
                warn!("{:?}", err);
            }

            tokio::time::sleep(Duration::from_millis(RETRY_INTERVAL)).await;
        }
    }
    unreachable!();
}

pub async fn connect_kaspad(cli_args: &CliArgs) -> Result<KaspaRpcClient, Error> {
    let network_id = cli_args
        .network
        .as_str()
        .split_once("-")
        .and_then(|(nt, ns)| {
            let network_type = NetworkType::from_str(nt).ok()?;
            let suffix = ns.parse().ok()?;
            Some(NetworkId::with_suffix(network_type, suffix))
        })
        .unwrap_or_else(|| NetworkId::new(NetworkType::from_str(cli_args.network.as_str()).unwrap()));

    let url = if let Some(url) = cli_args.rpc_url.as_ref() {
        url
    } else {
        &Resolver::default().get_url(WrpcEncoding::Borsh, network_id).await?
    };

    info!("Connecting to Kaspad {}", url);
    let client = KaspaRpcClient::new_with_args(WrpcEncoding::Borsh, Some(&url), None, Some(network_id), None)?;
    client.connect(Some(ConnectOptions::blocking_fallback())).await?;

    let server_info = client.get_server_info().await?;
    let network = format!(
        "{}{}",
        server_info.network_id.network_type,
        server_info.network_id.suffix.map(|s| format!("-{}", s.to_string())).unwrap_or_default()
    );
    info!("Connected to Kaspad {} version: {}, network: {}", url, server_info.server_version, network);

    if network != cli_args.network {
        return Err(Error::Custom(format!("Network mismatch, expected '{}', actual '{}'", cli_args.network, network)));
    } else if server_info.network_id.network_type == RpcNetworkType::Mainnet && server_info.virtual_daa_score < 76902846 {
        return Err(Error::Custom("Invalid network".to_string()));
    } else if !server_info.is_synced {
        while !client.get_server_info().await?.is_synced {
            warn!("Kaspad {} is NOT synced, retrying in 10 seconds...", server_info.server_version);
            sleep(Duration::from_secs(10)).await;
        }
    }
    Ok(client)
}
