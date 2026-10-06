use std::{
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use tokio::time::MissedTickBehavior;

use self::{
    config::ServiceConfig,
    history::{Bucket, HOURS, History, Saved},
};

pub use config::Config;

pub mod config;
pub mod history;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Health {
    /// Responded without a 4XX/5XX, within the slow threshold.
    Up,
    /// Not checked yet.
    Pending,
    /// Responded without a 4XX/5XX, but slower than the slow threshold.
    Slow,
    /// Responded with a 4XX/5XX, timed out, or could not be reached.
    Down,
}

impl Health {
    pub fn severity(self) -> u8 {
        match self {
            Health::Up => 0,
            Health::Pending => 1,
            Health::Slow => 2,
            Health::Down => 3,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Check {
    pub health: Health,
    /// Status code, or short reason why it failed.
    pub detail: String,
    pub latency: Option<Duration>,
}

pub struct Monitor {
    pub config: ServiceConfig,
    last: RwLock<Option<Check>>,
    history: RwLock<History>,
}

impl Monitor {
    pub fn last(&self) -> Option<Check> {
        self.last.read().unwrap().clone()
    }

    pub fn health(&self) -> Health {
        self.last().map_or(Health::Pending, |check| check.health)
    }

    /// Worst health per hour for the last 24 hours, oldest first.
    pub fn history(&self) -> [Option<Health>; HOURS] {
        self.history.read().unwrap().hours()
    }

    /// History buckets to save to disk.
    pub fn history_buckets(&self) -> Vec<Bucket> {
        self.history.read().unwrap().buckets()
    }

    /// Merges buckets read from disk into the in-memory history.
    pub fn merge_history(&self, buckets: impl IntoIterator<Item = Bucket>) {
        self.history.write().unwrap().merge(buckets);
    }
}

pub type Monitors = Arc<[Arc<Monitor>]>;

/// Starts a background task per service and returns their shared state.
/// History saved under a service's name is restored into that service.
pub fn spawn(services: Vec<ServiceConfig>, mut saved: Saved) -> Monitors {
    let client = reqwest::Client::new();

    services
        .into_iter()
        .map(|config| {
            let history = saved
                .remove(&config.name)
                .map_or_else(History::new, History::restore);
            let monitor = Arc::new(Monitor {
                config,
                last: RwLock::new(None),
                history: RwLock::new(history),
            });
            tokio::spawn(run(client.clone(), monitor.clone()));
            monitor
        })
        .collect()
}

async fn run(client: reqwest::Client, monitor: Arc<Monitor>) {
    let mut interval = tokio::time::interval(monitor.config.interval());
    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        interval.tick().await;
        let check = check(&client, &monitor.config).await;

        let previous = monitor.health();
        if check.health != previous {
            let name = &monitor.config.name;
            match check.health {
                Health::Up => {
                    tracing::info!(service = %name, detail = %check.detail, "service is up")
                }
                Health::Slow => {
                    tracing::warn!(service = %name, latency = ?check.latency, "service is slow")
                }
                Health::Down => {
                    tracing::error!(service = %name, detail = %check.detail, "service is down")
                }
                Health::Pending => {}
            }
        }

        monitor.history.write().unwrap().record(check.health);
        *monitor.last.write().unwrap() = Some(check);
    }
}

async fn check(client: &reqwest::Client, config: &ServiceConfig) -> Check {
    let start = Instant::now();
    let result = client
        .get(&config.url)
        .timeout(config.timeout())
        .send()
        .await;
    let latency = start.elapsed();
    let (health, detail, latency) = match result {
        Ok(response) => {
            // `to_string` includes the canonical reason phrase, e.g. "404 Not Found",
            // but we only want the code.
            let status = response.status().as_str().to_owned();
            let health = response.into();
            (health, status, Some(latency))
        }
        Err(err) if err.is_timeout() => (Health::Down, "timed out".into(), None),
        Err(err) if err.is_connect() => (Health::Down, "conn failed".into(), None),
        Err(err) => (Health::Down, err.to_string(), None),
    };

    Check {
        health,
        detail,
        latency,
    }
}

impl From<reqwest::Response> for Health {
    fn from(response: reqwest::Response) -> Self {
        match response.status() {
            StatusCode::OK => Health::Up,
            status if status.is_client_error() || status.is_server_error() => Health::Down,
            _ => Health::Pending,
        }
    }
}
