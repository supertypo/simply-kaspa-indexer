use crate::utxo_import::p2p_initializer::P2pInitializer;
use crate::web::model::metrics::Metrics;
use bigdecimal::ToPrimitive;
use kaspa_addresses::Prefix;
use kaspa_consensus_core::config::params::{MAINNET_PARAMS, TESTNET_PARAMS};
use kaspa_consensus_core::errors::consensus::ConsensusError::UnexpectedPruningPoint;
use kaspa_consensus_core::tx::ScriptPublicKey;
use kaspa_hashes::Hash as KaspaHash;
use kaspa_p2p_lib::common::ProtocolError;
use kaspa_p2p_lib::pb::kaspad_message::Payload;
use kaspa_p2p_lib::pb::{
    AddressesMessage, KaspadMessage, OutpointAndUtxoEntryPair, PongMessage, RequestNextPruningPointUtxoSetChunkMessage,
    RequestPruningPointUtxoSetMessage,
};
use kaspa_p2p_lib::{Adaptor, Hub, PeerKey, make_message};
use kaspa_txscript::extract_script_pub_key_address;
use kaspa_wrpc_client::prelude::{NetworkId, NetworkType};
use log::{debug, info, trace, warn};
use rand::prelude::IndexedRandom;
use rand::rng;
use simply_kaspa_cli::cli_args::{CliArgs, CliField};
use simply_kaspa_database::client::KaspaDbClient;
use simply_kaspa_database::models::transaction_acceptance::TransactionAcceptance;
use simply_kaspa_database::models::transaction_output::TransactionOutput;
use simply_kaspa_signal::signal_handler::SignalHandler;
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::Receiver;
use tokio::sync::{RwLock, mpsc};
use tokio::time::{sleep, timeout};
use url::Url;

pub const IBD_BATCH_SIZE: u32 = 99;
pub const IBD_TIMEOUT_SECONDS: u64 = 30;
pub const IBD_RETRIES: u32 = 10;

pub struct UtxoSetImporter {
    cli_args: CliArgs,
    signal_handler: SignalHandler,
    metrics: Arc<RwLock<Metrics>>,
    pruning_point_hash: KaspaHash,
    database: KaspaDbClient,
    network_id: NetworkId,
    prefix: Prefix,
    include_amount: bool,
    include_script_public_key: bool,
    include_script_public_key_address: bool,
    include_block_time: bool,
}

impl UtxoSetImporter {
    pub fn new(
        cli_args: CliArgs,
        signal_handler: SignalHandler,
        metrics: Arc<RwLock<Metrics>>,
        pruning_point_hash: KaspaHash,
        database: KaspaDbClient,
    ) -> UtxoSetImporter {
        let network_id = NetworkId::from_str(&cli_args.network).unwrap();
        let prefix = Prefix::from(network_id);
        let include_amount = !cli_args.is_excluded(CliField::TxOutAmount);
        let include_script_public_key = !cli_args.is_excluded(CliField::TxOutScriptPublicKey);
        let include_script_public_key_address = !cli_args.is_excluded(CliField::TxOutScriptPublicKeyAddress);
        let include_block_time = !cli_args.is_excluded(CliField::TxOutBlockTime);
        UtxoSetImporter {
            cli_args,
            signal_handler,
            metrics,
            pruning_point_hash,
            database,
            network_id,
            prefix,
            include_amount,
            include_script_public_key,
            include_script_public_key_address,
            include_block_time,
        }
    }

    pub async fn start(&self) {
        let mut completed = false;
        while !self.signal_handler.is_shutdown() && !completed {
            let address = if let Some(p2p_url) = &self.cli_args.p2p_url {
                Some(p2p_url.clone())
            } else {
                let params = match self.network_id {
                    NetworkId { network_type: NetworkType::Mainnet, suffix: None } => Some(MAINNET_PARAMS),
                    NetworkId { network_type: NetworkType::Testnet, suffix: Some(10) } => Some(TESTNET_PARAMS),
                    _ => None,
                };
                if let Some(params) = params {
                    if let Some(rpc_url) = &self.cli_args.rpc_url {
                        Some(format!("{}:{}", Url::parse(rpc_url).unwrap().host().unwrap(), params.default_p2p_port()))
                    } else {
                        Some(format!("{}:{}", params.dns_seeders.choose(&mut rng()).unwrap(), params.default_p2p_port()))
                    }
                } else {
                    None
                }
            };
            if let Some(address) = address {
                info!("Connecting P2P for UTXO set import using {}", address);
                let (sender, receiver) = mpsc::channel(10000);
                let initializer = Arc::new(P2pInitializer::new(self.cli_args.clone(), sender));
                let adaptor = Adaptor::client_only(Hub::new(), initializer, Default::default());
                {
                    let mut metrics = self.metrics.write().await;
                    metrics.components.utxo_importer.enabled = true;
                    metrics.components.utxo_importer.completed = Some(false);
                }
                match adaptor.connect_peer(address).await {
                    Ok(peer_key) => {
                        match self.receive_and_handle(adaptor.clone(), peer_key, self.pruning_point_hash, receiver).await {
                            Ok(_) => completed = true,
                            Err(_) => sleep(Duration::from_secs(5)).await,
                        }
                        adaptor.terminate_all_peers().await;
                    }
                    Err(e) => warn!("Peer connection failed: {e}, retrying..."),
                }
            } else {
                info!("UTXO set import skipped for network {}", self.network_id);
                completed = true;
            }
        }
        let mut metrics = self.metrics.write().await;
        metrics.components.utxo_importer.completed = Some(completed);
    }

    async fn receive_and_handle(
        &self,
        adaptor: Arc<Adaptor>,
        peer_key: PeerKey,
        pruning_point_hash: KaspaHash,
        mut receiver: Receiver<KaspadMessage>,
    ) -> Result<(), ProtocolError> {
        let mut acceptance_committed_count = 0;
        let mut outputs_committed_count = 0;
        let mut utxo_chunk_count = 0;
        let mut utxos_count: u64 = 0;
        while !self.signal_handler.is_shutdown() {
            match timeout(Duration::from_secs(IBD_TIMEOUT_SECONDS), receiver.recv()).await {
                Ok(op) => match op {
                    Some(msg) => match msg.payload {
                        Some(Payload::Version(msg)) => {
                            debug!("P2P: ua: {}, proto: {}, network: {}", msg.user_agent, msg.protocol_version, msg.network);
                        }
                        Some(Payload::RequestAddresses(_)) => {
                            debug!("Got addresses request, responding with empty list");
                            adaptor
                                .send(peer_key, make_message!(Payload::Addresses, AddressesMessage { address_list: vec![] }))
                                .await?;
                            // Peer is alive and ready, continue requesting UTXO set...
                            adaptor
                                .send(
                                    peer_key,
                                    make_message!(
                                        Payload::RequestPruningPointUtxoSet,
                                        RequestPruningPointUtxoSetMessage { pruning_point_hash: Some(pruning_point_hash.into()) }
                                    ),
                                )
                                .await?;
                        }
                        Some(Payload::PruningPointUtxoSetChunk(msg)) => {
                            utxo_chunk_count += 1;
                            utxos_count += msg.outpoint_and_utxo_entry_pairs.len() as u64;
                            let (acceptance_count, output_count) = self.persist_utxos(msg.outpoint_and_utxo_entry_pairs).await;
                            acceptance_committed_count += acceptance_count;
                            outputs_committed_count += output_count;
                            if utxo_chunk_count % IBD_BATCH_SIZE == 0 {
                                self.print_progress(utxo_chunk_count, acceptance_committed_count, outputs_committed_count);
                                adaptor
                                    .send(
                                        peer_key,
                                        make_message!(
                                            Payload::RequestNextPruningPointUtxoSetChunk,
                                            RequestNextPruningPointUtxoSetChunkMessage {}
                                        ),
                                    )
                                    .await?;
                                let mut metrics = self.metrics.write().await;
                                metrics.components.utxo_importer.utxos_imported = Some(utxos_count);
                                metrics.components.utxo_importer.acceptances_committed = Some(acceptance_committed_count);
                                metrics.components.utxo_importer.outputs_committed = Some(outputs_committed_count);
                            }
                        }
                        Some(Payload::DonePruningPointUtxoSetChunks(_)) => {
                            self.print_progress(utxo_chunk_count, acceptance_committed_count, outputs_committed_count);
                            info!("Pruning point UTXO set import completed successfully!");
                            let mut metrics = self.metrics.write().await;
                            metrics.components.utxo_importer.utxos_imported = Some(utxos_count);
                            metrics.components.utxo_importer.acceptances_committed = Some(acceptance_committed_count);
                            metrics.components.utxo_importer.outputs_committed = Some(outputs_committed_count);
                            return Ok(());
                        }
                        Some(Payload::UnexpectedPruningPoint(_)) => {
                            warn!("Got unexpected pruning point");
                            return Err(ProtocolError::ConsensusError(UnexpectedPruningPoint));
                        }
                        Some(Payload::Ping(msg)) => {
                            debug!("Got ping (nonce={}), responding with pong", msg.nonce);
                            adaptor.send(peer_key, make_message!(Payload::Pong, PongMessage { nonce: msg.nonce })).await?;
                        }
                        Some(msg) => trace!("Ignoring message: {:?}", msg),
                        None => warn!("Got message with empty payload"),
                    },
                    None => {
                        warn!("Channel unexpectedly closed");
                        return Err(ProtocolError::ConnectionClosed);
                    }
                },
                Err(_) => {
                    warn!("Peer timed out after {} seconds", IBD_TIMEOUT_SECONDS);
                    return Err(ProtocolError::Timeout(Duration::from_secs(IBD_TIMEOUT_SECONDS)));
                }
            }
        }
        Err(ProtocolError::Other("Aborted"))
    }

    async fn persist_utxos(&self, outpoint_and_utxo_entry_pair: Vec<OutpointAndUtxoEntryPair>) -> (u64, u64) {
        let transaction_outputs: Vec<TransactionOutput> = outpoint_and_utxo_entry_pair
            .into_iter()
            .map(|u| {
                let outpoint = u.outpoint.unwrap();
                let utxo_entry = u.utxo_entry.unwrap();
                let script_public_key: ScriptPublicKey = utxo_entry.script_public_key.unwrap().try_into().unwrap();
                TransactionOutput {
                    transaction_id: KaspaHash::from_slice(outpoint.transaction_id.unwrap().bytes.as_slice()).into(),
                    index: outpoint.index.to_i16().unwrap(),
                    amount: self.include_amount.then_some(utxo_entry.amount as i64),
                    script_public_key: self.include_script_public_key.then_some(script_public_key.script().to_vec()),
                    script_public_key_address: self
                        .include_script_public_key_address
                        .then(|| extract_script_pub_key_address(&script_public_key, self.prefix).map(|a| a.payload_to_string()).ok())
                        .flatten(),
                    block_time: self.include_block_time.then_some(0),
                }
            })
            .collect();
        let mut unique_acceptances = HashSet::new();
        let tx_acceptances: Vec<TransactionAcceptance> = transaction_outputs
            .iter()
            .filter(|o| unique_acceptances.insert(&o.transaction_id))
            .map(|o| TransactionAcceptance { transaction_id: Some(o.transaction_id.clone()), block_hash: None })
            .collect();
        let acceptance_count = self.database.insert_transaction_acceptances(&tx_acceptances).await.unwrap();
        let output_count = self.database.insert_transaction_outputs(&transaction_outputs).await.unwrap();
        (acceptance_count, output_count)
    }

    fn print_progress(&self, utxo_chunk_count: u32, acceptance_committed_count: u64, outputs_committed_count: u64) {
        info!(
            "Imported {} UTXO chunks. Committed {} accepted transactions, {} outputs",
            utxo_chunk_count, acceptance_committed_count, outputs_committed_count,
        );
    }
}
