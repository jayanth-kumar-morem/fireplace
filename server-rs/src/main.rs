mod config;
mod aggregator;
mod feeds;
mod types;
mod ws_server;

use axum::{routing::get, Router};
use std::net::SocketAddr;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, mpsc, watch, RwLock};
use tracing::{error, info};

use crate::aggregator::{run_aggregator, AppState};
use crate::config::AppConfig;
use crate::feeds::kalshi::run_kalshi_feed;
use crate::feeds::polymarket::run_polymarket_feed;
use crate::ws_server::{health_handler, run_heartbeat, snapshot_handler, ws_handler, AppContext};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let cfg = AppConfig::from_env()?;
    let http_client = reqwest::Client::new();
    let state = Arc::new(RwLock::new(AppState::default()));
    let (feed_tx, feed_rx) = mpsc::channel(1024);
    let (broadcast_tx, _) = broadcast::channel::<Arc<str>>(1024);
    let client_count = Arc::new(AtomicUsize::new(0));
    let started_at = Arc::new(Instant::now());
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let ctx = AppContext {
        state: state.clone(),
        broadcast_tx: broadcast_tx.clone(),
        client_count,
        started_at,
    };

    let polymarket_handle = tokio::spawn(run_polymarket_feed(
        feed_tx.clone(),
        http_client.clone(),
        shutdown_rx.clone(),
    ));
    let kalshi_handle = tokio::spawn(run_kalshi_feed(
        feed_tx.clone(),
        http_client.clone(),
        cfg.kalshi_api_key.clone(),
        cfg.kalshi_private_key_path.clone(),
        shutdown_rx.clone(),
    ));
    drop(feed_tx);
    let aggregator_handle = tokio::spawn(run_aggregator(
        feed_rx,
        state.clone(),
        broadcast_tx.clone(),
        http_client.clone(),
        shutdown_rx.clone(),
    ));
    let heartbeat_handle = tokio::spawn(run_heartbeat(ctx.clone(), shutdown_rx.clone()));

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/api/snapshot", get(snapshot_handler))
        .route("/ws", get(ws_handler))
        .with_state(ctx);
    let addr = SocketAddr::from(([0, 0, 0, 0], cfg.port));

    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("server-rs listening on http://{}", listener.local_addr()?);

    let shutdown_tx_for_server = shutdown_tx.clone();
    let server_result = axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            info!("shutdown signal received");
            let _ = shutdown_tx_for_server.send(true);
        })
        .await;

    let _ = shutdown_tx.send(true);
    if let Err(err) = server_result {
        error!("axum server exited with error: {err}");
    } else {
        info!("axum server stopped");
    }

    match polymarket_handle.await {
        Ok(_) => info!("polymarket feed task stopped"),
        Err(err) => error!("polymarket feed task join error: {err}"),
    }
    match kalshi_handle.await {
        Ok(_) => info!("kalshi feed task stopped"),
        Err(err) => error!("kalshi feed task join error: {err}"),
    }
    match aggregator_handle.await {
        Ok(_) => info!("aggregator task stopped"),
        Err(err) => error!("aggregator task join error: {err}"),
    }
    match heartbeat_handle.await {
        Ok(_) => info!("heartbeat task stopped"),
        Err(err) => error!("heartbeat task join error: {err}"),
    }

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sigterm) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            let _ = sigterm.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}
