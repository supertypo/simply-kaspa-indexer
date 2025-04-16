use crate::settings::Settings;
use kaspa_consensus_core::config::params::MAINNET_PARAMS;
use kaspa_wrpc_client::prelude::{NetworkId, NetworkType};
use log::info;
use rand::prelude::IndexedRandom;
use rand::random;
use std::str::FromStr;
use url::Url;

pub async fn import_utxo_set(settings: Settings) {
    let network_id = NetworkId::from_str(&settings.cli_args.network).unwrap();
    let p2p_socket = if let Some(p2p_url) = settings.cli_args.p2p_url {
        Some(p2p_url)
    } else if let Some(rpc_url) = settings.cli_args.rpc_url {
        Some(format!("{}:{}", Url::parse(&rpc_url).unwrap().host().unwrap().to_string(), MAINNET_PARAMS.default_p2p_port()))
    } else {
        match network_id {
            NetworkId { network_type: NetworkType::Mainnet, suffix: None } => {
                Some(format!("{}:{}", MAINNET_PARAMS.dns_seeders.choose(&mut random()).unwrap().to_string(), ))
            }
            NetworkId { network_type: NetworkType::Testnet, suffix: Some(10) } => {
                Some(MAINNET_PARAMS.dns_seeders.choose(&mut random()).unwrap().to_string())
            }
            _ => {
                info!("Skipping UTXO import for unsupported network {}", network_id);
                None
            }
        }
    };
}
