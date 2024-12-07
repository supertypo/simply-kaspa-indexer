use crate::pool::manager::KaspadManager;
use broadcast::Sender;
use deadpool::managed::Pool;
use futures::{select_biased, FutureExt};
use kaspa_wrpc_client::error::Error;
use kaspa_wrpc_client::prelude::*;
use log::{error, info, trace, warn};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::sync::Mutex;
use tokio::time::sleep;
use workflow_core::channel::{Channel, DuplexChannel};

struct Inner {
    shutdown_sender: Sender<()>,
    pool: Pool<KaspadManager>,
    notification_sender: Sender<Arc<Notification>>,
    listen_on_scopes: Mutex<Vec<Scope>>,
    reconnect_interval: Option<Duration>,
    task_ctl: DuplexChannel<()>,
    notification_channel: Channel<Notification>,
    listener_id: Mutex<Option<ListenerId>>,
}

#[derive(Clone)]
pub struct Listener {
    inner: Arc<Inner>,
}

impl Listener {
    pub fn new(
        shutdown_sender: Sender<()>,
        pool: Pool<KaspadManager>,
        notification_sender: Sender<Arc<Notification>>,
        listen_on_scopes: Vec<Scope>,
        reconnect_interval: Option<Duration>,
    ) -> Self {
        let inner = Inner {
            shutdown_sender,
            pool,
            notification_sender,
            listen_on_scopes: Mutex::new(listen_on_scopes),
            reconnect_interval,
            task_ctl: DuplexChannel::oneshot(),
            notification_channel: Channel::unbounded(),
            listener_id: Mutex::new(None),
        };
        Self { inner: Arc::new(inner) }
    }

    pub async fn start(&mut self) -> kaspa_wrpc_client::result::Result<()> {
        self.start_event_task().await?;
        Ok(())
    }

    pub async fn stop(&mut self) -> kaspa_wrpc_client::result::Result<()> {
        self.stop_event_task().await?;
        Ok(())
    }

    async fn start_event_task(&mut self) -> kaspa_wrpc_client::result::Result<()> {
        let mut listener = self.clone();
        let mut shutdown_receiver = self.inner.shutdown_sender.subscribe();
        let task_ctl_receiver = self.inner.task_ctl.request.receiver.clone();
        let notification_receiver = self.inner.notification_channel.receiver.clone();

        tokio::task::spawn(async move {
            let mut listen = true;
            while listen && shutdown_receiver.is_empty() {
                if let Ok(client) = listener.client().await {
                    let mut client_ready = true;
                    let rpc_ctl_channel = client.rpc_ctl().multiplexer().channel();
                    if client.is_connected() && listener.handle_connect(client.clone()).await.is_err() {
                        client_ready = false;
                    }
                    while listen && client_ready {
                        select_biased! {
                            _ = shutdown_receiver.recv().fuse() => {
                                listener.handle_disconnect(client.clone()).await.unwrap_or_default();
                                listen = false;
                            },
                            _ = task_ctl_receiver.recv().fuse() => {
                                listener.handle_disconnect(client.clone()).await.unwrap_or_default();
                                listen = false;
                            },
                            _ = sleep(Duration::from_secs(30)).fuse() => {
                                warn!("No notifications received within 30 seconds, reconnecting...");
                                listener.handle_disconnect(client.clone()).await.unwrap_or_default();
                                client_ready = false;
                            },
                            msg = rpc_ctl_channel.receiver.recv().fuse() => {
                                match msg {
                                    Ok(msg) => {
                                        match msg {
                                            RpcState::Connected => {
                                                if listener.handle_connect(client.clone()).await.is_err() {
                                                    client_ready = false;
                                                }
                                            },
                                            RpcState::Disconnected => {
                                                client_ready = false;
                                            }
                                        }
                                    }
                                    Err(err) => {
                                        warn!("RPC CTL channel error: {err}");
                                        client_ready = false;
                                    }
                                }
                            }
                            notification = notification_receiver.recv().fuse() => {
                                match notification {
                                    Ok(notification) => {
                                        if let Err(err) = listener.handle_notification(notification).await {
                                            error!("Error while handling notification: {err}");
                                        }
                                    }
                                    Err(err) => {
                                        warn!("Kaspad RPC notification channel error: {err}");
                                        client_ready = false;
                                    }
                                }
                            },
                        }
                    }
                } else {
                    warn!("Kaspad connection failed")
                }
                if listen {
                    if let Some(interval) = listener.inner.reconnect_interval {
                        info!("Sleeping {} seconds before attempting Kaspad reconnect", interval.as_secs());
                        sleep(interval).await;
                    } else {
                        listen = false;
                    }
                }
            }
        })
        .await
        .unwrap_or_default();
        info!("Kaspad listener shutdown");
        Ok(())
    }

    async fn stop_event_task(&self) -> kaspa_wrpc_client::result::Result<()> {
        self.inner.task_ctl.signal(()).await?;
        Ok(())
    }

    async fn handle_connect(&self, client: Arc<KaspaRpcClient>) -> kaspa_wrpc_client::result::Result<()> {
        info!("Registering Kaspad listeners on {}", client.url().unwrap_or("unknown".to_string()));
        self.register_notification_listeners(client).await?;
        Ok(())
    }

    async fn handle_disconnect(&self, client: Arc<KaspaRpcClient>) -> kaspa_wrpc_client::result::Result<()> {
        info!("Unregistering Kaspad listeners on {}", client.url().unwrap_or("unknown".to_string()));
        self.unregister_notification_listener(client).await?;
        Ok(())
    }

    async fn handle_notification(&self, notification: Notification) -> kaspa_wrpc_client::result::Result<()> {
        trace!("Received Kaspad notification: {}", notification);
        let notification = Arc::new(notification);
        if let Ok(count) = self.inner.notification_sender.send(notification) {
            trace!("Kaspad notification broadcast to {} receivers", count);
        } else {
            trace!("Kaspad notification not broadcast, due to 0 receivers");
        }
        Ok(())
    }

    async fn register_notification_listeners(&self, client: Arc<KaspaRpcClient>) -> kaspa_wrpc_client::result::Result<()> {
        let listener_id = client.rpc_api().register_new_listener(ChannelConnection::new(
            "client listener",
            self.inner.notification_channel.sender.clone(),
            ChannelType::Persistent,
        ));
        *self.inner.listener_id.lock().await = Some(listener_id);
        for s in self.inner.listen_on_scopes.lock().await.iter() {
            client.rpc_api().start_notify(listener_id, s.clone()).await?;
        }
        Ok(())
    }

    async fn unregister_notification_listener(&self, client: Arc<KaspaRpcClient>) -> kaspa_wrpc_client::result::Result<()> {
        if let Some(id) = self.inner.listener_id.lock().await.take() {
            client.rpc_api().unregister_listener(id).await.unwrap_or_default();
        }
        Ok(())
    }

    pub async fn client(&mut self) -> kaspa_wrpc_client::result::Result<Arc<KaspaRpcClient>> {
        let client = self
            .inner
            .pool
            .get()
            .await
            .map(|c| c.clone())
            .map_err(|e| Error::Custom(format!("Failed to get Kaspad connection from pool: {:?}", e)))?;
        Ok(client)
    }
}
