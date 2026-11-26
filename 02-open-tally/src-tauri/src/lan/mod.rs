pub mod auth;
pub mod models;
pub mod server;

pub use auth::*;
pub use models::*;
pub use server::*;

use crate::errors::OpenTallyError;
use sqlx::{Pool, Sqlite};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::oneshot;
use tracing::info;

/// Handle to a running LAN Host server instance.
pub struct LanServerHandle {
    pub shutdown_tx: Option<oneshot::Sender<()>>,
    pub bind_address: String,
    pub port: u16,
    pub session_store: Arc<LanSessionStore>,
}

/// Spawns the embedded Axum LAN server on a background Tokio task.
pub async fn start_lan_listener(
    pool: Pool<Sqlite>,
    bind_address: Option<String>,
    requested_port: Option<u16>,
) -> Result<LanServerHandle, OpenTallyError> {
    let addr_str = bind_address.unwrap_or_else(|| "0.0.0.0".to_string());
    let port = requested_port.unwrap_or(7900);
    let socket_addr: SocketAddr = format!("{}:{}", addr_str, port).parse().map_err(|e| {
        OpenTallyError::Config(format!(
            "Invalid LAN bind address '{}:{}': {}",
            addr_str, port, e
        ))
    })?;

    let listener = tokio::net::TcpListener::bind(socket_addr)
        .await
        .map_err(OpenTallyError::Io)?;

    let actual_port = listener.local_addr().map_err(OpenTallyError::Io)?.port();

    let session_store = Arc::new(LanSessionStore::new());
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    let ctx = LanServerContext {
        pool,
        session_store: session_store.clone(),
        write_mutex: Arc::new(tokio::sync::Mutex::new(())),
    };

    let router = create_lan_router(ctx);

    info!(
        "OpenTally LAN Host Server listening on {}:{} (opt-in multi-user mode)",
        addr_str, actual_port
    );

    tokio::spawn(async move {
        let make_service = router.into_make_service_with_connect_info::<SocketAddr>();
        if let Err(e) = axum::serve(listener, make_service)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
                info!("OpenTally LAN Host Server received graceful shutdown signal");
            })
            .await
        {
            tracing::error!("LAN server error: {}", e);
        }
    });

    Ok(LanServerHandle {
        shutdown_tx: Some(shutdown_tx),
        bind_address: addr_str,
        port: actual_port,
        session_store,
    })
}
