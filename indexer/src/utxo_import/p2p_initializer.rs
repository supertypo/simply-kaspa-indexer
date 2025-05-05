use kaspa_p2p_lib::common::ProtocolError;
use kaspa_p2p_lib::pb::kaspad_message::Payload;
use kaspa_p2p_lib::pb::{KaspadMessage, VersionMessage};
use kaspa_p2p_lib::{ConnectionInitializer, IncomingRoute, KaspadHandshake, KaspadMessagePayloadType, Router};
use simply_kaspa_cli::cli_args::CliArgs;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::Sender;
use tonic::async_trait;
use uuid::Uuid;

pub struct P2pInitializer {
    cli_args: CliArgs,
    sender: Sender<KaspadMessage>,
}

impl P2pInitializer {
    pub fn new(cli_args: CliArgs, sender: Sender<KaspadMessage>) -> Self {
        P2pInitializer { cli_args, sender }
    }
}

#[async_trait]
impl ConnectionInitializer for P2pInitializer {
    async fn initialize_connection(&self, router: Arc<Router>) -> Result<(), ProtocolError> {
        let mut handshake = KaspadHandshake::new(&router);
        router.start();
        let sender = self.sender.clone();
        let version_msg = handshake.handshake(self.version_message()).await?;
        let mut incoming_route = subscribe_all(&router);
        tokio::spawn(async move {
            while let Some(msg) = incoming_route.recv().await {
                let _ = sender.send(msg).await;
            }
        });
        handshake.exchange_ready_messages().await?;
        self.sender.send(KaspadMessage { request_id: 0, response_id: 0, payload: Some(Payload::Version(version_msg)) }).await.unwrap();
        Ok(())
    }
}

impl P2pInitializer {
    pub fn version_message(&self) -> VersionMessage {
        VersionMessage {
            protocol_version: 7,
            services: 0,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64,
            address: None,
            id: Vec::from(Uuid::new_v4().as_bytes()),
            user_agent: format!("/{}:{}/", env!("CARGO_PKG_NAME"), self.cli_args.version()),
            disable_relay_tx: true,
            subnetwork_id: None,
            network: format!("kaspa-{}", self.cli_args.network.to_lowercase()),
        }
    }
}

fn subscribe_all(router: &Arc<Router>) -> IncomingRoute {
    router.subscribe(vec![
        KaspadMessagePayloadType::Addresses,
        KaspadMessagePayloadType::Block,
        KaspadMessagePayloadType::Transaction,
        KaspadMessagePayloadType::BlockLocator,
        KaspadMessagePayloadType::RequestAddresses,
        KaspadMessagePayloadType::RequestRelayBlocks,
        KaspadMessagePayloadType::RequestTransactions,
        KaspadMessagePayloadType::IbdBlock,
        KaspadMessagePayloadType::InvRelayBlock,
        KaspadMessagePayloadType::InvTransactions,
        KaspadMessagePayloadType::Ping,
        KaspadMessagePayloadType::Pong,
        // KaspadMessagePayloadType::Verack,
        // KaspadMessagePayloadType::Version,
        KaspadMessagePayloadType::TransactionNotFound,
        KaspadMessagePayloadType::Reject,
        KaspadMessagePayloadType::PruningPointUtxoSetChunk,
        KaspadMessagePayloadType::RequestIbdBlocks,
        KaspadMessagePayloadType::UnexpectedPruningPoint,
        KaspadMessagePayloadType::IbdBlockLocator,
        KaspadMessagePayloadType::IbdBlockLocatorHighestHash,
        KaspadMessagePayloadType::RequestNextPruningPointUtxoSetChunk,
        KaspadMessagePayloadType::DonePruningPointUtxoSetChunks,
        KaspadMessagePayloadType::IbdBlockLocatorHighestHashNotFound,
        KaspadMessagePayloadType::BlockWithTrustedData,
        KaspadMessagePayloadType::DoneBlocksWithTrustedData,
        KaspadMessagePayloadType::RequestPruningPointAndItsAnticone,
        KaspadMessagePayloadType::BlockHeaders,
        KaspadMessagePayloadType::RequestNextHeaders,
        KaspadMessagePayloadType::DoneHeaders,
        KaspadMessagePayloadType::RequestPruningPointUtxoSet,
        KaspadMessagePayloadType::RequestHeaders,
        KaspadMessagePayloadType::RequestBlockLocator,
        KaspadMessagePayloadType::PruningPoints,
        KaspadMessagePayloadType::RequestPruningPointProof,
        KaspadMessagePayloadType::PruningPointProof,
        // KaspadMessagePayloadType::Ready,
        KaspadMessagePayloadType::BlockWithTrustedDataV4,
        KaspadMessagePayloadType::TrustedData,
        KaspadMessagePayloadType::RequestIbdChainBlockLocator,
        KaspadMessagePayloadType::IbdChainBlockLocator,
        KaspadMessagePayloadType::RequestAntipast,
        KaspadMessagePayloadType::RequestNextPruningPointAndItsAnticoneBlocks,
    ])
}
