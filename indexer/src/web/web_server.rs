use crate::settings::Settings;
use crate::web::endpoint;
use crate::web::endpoint::{health, metrics};
use crate::web::model::metrics::Metrics;
use axum::body::{Body, to_bytes};
use axum::http::{HeaderValue, Request, header};
use axum::middleware::Next;
use axum::response::Response;
use axum::{Extension, Router, middleware, routing::get};
use deadpool::managed::{Object, Pool};
use log::{Level, info, trace};
use simply_kaspa_database::client::KaspaDbClient;
use simply_kaspa_kaspad::manager::KaspadManager;
use simply_kaspa_signal::signal_handler::SignalHandler;
use std::io::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use std::vec;
use sysinfo::System;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use utoipa::OpenApi;
use utoipa::openapi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_swagger_ui::{Config, SwaggerUi};

pub const INFO_TAG: &str = "info";

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Simply Kaspa Indexer REST API",
        license(name = "LICENSE", url = "https://github.com/supertypo/simply-kaspa-indexer"),
    ),
    paths(
        endpoint::health::get_health,
        endpoint::metrics::get_metrics,
    ),
    tags(
        (name = INFO_TAG, description = "Info API endpoints"),
    ),
)]
struct ApiDoc;

pub struct WebServer {
    settings: Settings,
    signal: SignalHandler,
    metrics: Arc<RwLock<Metrics>>,
    kaspad_pool: Pool<KaspadManager, Object<KaspadManager>>,
    database_client: KaspaDbClient,
    system: Arc<RwLock<System>>,
}

impl WebServer {
    pub fn new(
        settings: Settings,
        signal: SignalHandler,
        metrics: Arc<RwLock<Metrics>>,
        kaspad_pool: Pool<KaspadManager, Object<KaspadManager>>,
        database_client: KaspaDbClient,
    ) -> Self {
        WebServer { settings, signal, metrics, kaspad_pool, database_client, system: Arc::new(RwLock::new(System::new())) }
    }

    pub async fn run(self: Arc<Self>) -> Result<(), Error> {
        let listen = &self.settings.cli_args.listen;
        let base_path = &self.settings.cli_args.base_path.trim_end_matches("/");

        let (api_router, api) = OpenApiRouter::with_openapi(set_server_path(base_path))
            .route(&format!("{}{}", base_path, health::PATH), get(health::get_health))
            .route(&format!("{}{}", base_path, metrics::PATH), get(metrics::get_metrics))
            .split_for_parts();
        let swagger_config = Config::default().use_base_layout().try_it_out_enabled(true).display_request_duration(true);
        let swagger =
            SwaggerUi::new(format!("{}/api", base_path)).url(format!("{}/api/openapi.json", base_path), api).config(swagger_config);

        let app = Router::new()
            .merge(api_router)
            .merge(swagger)
            .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
            .layer(middleware::from_fn(add_default_cache_control))
            .layer(middleware::from_fn(log_requests))
            .layer(middleware::from_fn(log_responses))
            .layer(Extension(self.kaspad_pool.clone()))
            .layer(Extension(self.database_client.clone()))
            .layer(Extension(self.metrics.clone()))
            .layer(Extension(self.system.clone()));

        info!("Starting web server listener on {}, api path: {}/api", listen, base_path);
        let listener = tokio::net::TcpListener::bind(listen).await.expect("Failed to open listener");
        axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
            .with_graceful_shutdown(async move {
                let _ = self.signal.subscribe().recv().await;
                info!("Web server shutdown")
            })
            .await
    }
}

async fn add_default_cache_control(req: Request<Body>, next: Next) -> Response {
    let mut response = next.run(req).await;
    if !response.headers().contains_key(header::CACHE_CONTROL) {
        response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=5"));
    }
    response
}

async fn log_requests(mut req: Request<Body>, next: Next) -> Response {
    if log::log_enabled!(Level::Trace) {
        let method = req.method().clone();
        let uri = req.uri().path().to_string();
        let original_body = std::mem::take(req.body_mut());
        if let Ok(body_bytes) = to_bytes(original_body, usize::MAX).await {
            let body_string = String::from_utf8_lossy(&body_bytes);
            let truncated_body =
                if body_string.len() > 1000 { format!("{}[...]", &body_string[..1000]) } else { body_string.to_string() };
            trace!("Request: {} - {} - {}", method, uri, truncated_body);
            let new_body = Body::from(body_bytes);
            *req.body_mut() = new_body;
        }
    }
    next.run(req).await
}

async fn log_responses(req: Request<Body>, next: Next) -> Response {
    let mut response = next.run(req).await;
    if log::log_enabled!(Level::Trace) {
        let original_body = std::mem::take(response.body_mut());
        if let Ok(body_bytes) = to_bytes(original_body, usize::MAX).await {
            let body_string = String::from_utf8_lossy(&body_bytes);
            let truncated_body =
                if body_string.len() > 1000 { format!("{}[...]", &body_string[..1000]) } else { body_string.to_string() };
            trace!("Response: {} - {}", response.status(), truncated_body);
            *response.body_mut() = Body::from(body_bytes);
        }
    }
    response
}

pub fn set_server_path(base_path: &str) -> openapi::OpenApi {
    let mut openapi = ApiDoc::openapi();
    if !base_path.trim_end_matches("/").is_empty() {
        openapi.servers = Some(vec![openapi::ServerBuilder::new().url(base_path).build()]);
    }
    openapi
}
