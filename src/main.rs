use std::net::{Ipv4Addr, SocketAddr};

use anyhow::Context as _;
use tracing_subscriber::EnvFilter;

use crate::monitor::{
    config::{Config, ServiceConfig},
    history,
};

use self::state::AppState;

mod monitor;
mod router;
mod state;
mod view;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new("info")))
        .init();
    let path = std::env::args()
        .nth(1)
        .context("usage: shisutemu <config.toml>")?;
    let config = Config::load(&path)?;

    let port = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .or(config.port)
        .unwrap_or(3000);

    log_overview(&path, &config.services);

    let state_file = config.state_file.clone();
    let saved = match history::load(&state_file) {
        Ok(saved) => {
            if !saved.is_empty() {
                tracing::info!("restored history from {}", state_file.display());
            }
            saved
        }
        Err(err) => {
            tracing::warn!("{err:#}, starting with empty history");
            Default::default()
        }
    };

    let state = AppState::from_config(&config);
    let monitors = monitor::spawn(config.services, saved);
    tokio::spawn(history::run(state_file.clone(), monitors.clone()));
    let state = state.with_monitors(monitors.clone());

    let address = SocketAddr::from((Ipv4Addr::UNSPECIFIED, port));
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .with_context(|| format!("failed to bind {address}"))?;
    tracing::info!("listening on http://{address}");
    axum::serve(listener, router::router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    history::save(&state_file, &monitors)?;
    tracing::info!("saved history to {}", state_file.display());

    Ok(())
}

/// Handles on Ctrl-C, or SIGTERM on Unix.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to listen for Ctrl-C");
    };
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to listen for SIGTERM")
            .recv()
            .await;
    };
    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
    tracing::info!("shutting down");
}

fn log_overview(path: &str, services: &[ServiceConfig]) {
    let count = services.len();
    let noun = if count == 1 { "monitor" } else { "monitors" };
    tracing::info!("loaded {count} {noun} from {path}");

    let name_width = services.iter().map(|s| s.name.len()).max().unwrap_or(0);
    let url_width = services.iter().map(|s| s.url.len()).max().unwrap_or(0);
    for service in services {
        tracing::info!("  {:name_width$}  {:url_width$}", service.name, service.url,);
    }
}
