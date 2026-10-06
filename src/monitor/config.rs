use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Context as _;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Title of the status page.
    #[serde(default = "default_name")]
    pub name: String,
    /// Optional text shown under the title.
    pub description: Option<String>,
    /// Optional announcement shown above the status, e.g. planned maintenance.
    pub banner: Option<String>,
    /// Port for the status page. The `PORT` env var takes precedence.
    pub port: Option<u16>,
    /// Where the 24 hour history is saved, so it survives restarts.
    #[serde(default = "default_state_file")]
    pub state_file: PathBuf,
    #[serde(rename = "service", default)]
    pub services: Vec<ServiceConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServiceConfig {
    /// Name of the service, e.g. `API` or `Database`.
    pub name: String,
    /// URL to check. Must be a full URL including scheme, e.g. `https://example.com/health`.
    pub url: String,
    /// How often to check the service.
    ///
    /// Default is 30 seconds. Must be > 0.
    #[serde(default = "default_interval_secs")]
    pub interval_secs: u64,
    /// Responses slower than this are reported as slow.
    ///
    /// Default is 1000 ms. Must be > 0.
    #[serde(default = "default_slow_ms")]
    pub slow_ms: u64,
    /// Requests taking longer than this are reported as down.
    ///
    /// Default is 10,000 ms. Must be > 0.
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_name() -> String {
    "Status".into()
}

fn default_state_file() -> PathBuf {
    "shisutemu.json".into()
}

fn default_interval_secs() -> u64 {
    30
}

fn default_slow_ms() -> u64 {
    1000
}

fn default_timeout_ms() -> u64 {
    10_000
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref();
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let config: Self =
            toml::from_str(&raw).with_context(|| format!("failed to parse {}", path.display()))?;

        // No reason to run if there are no services to monitor.
        anyhow::ensure!(
            !config.services.is_empty(),
            "no service entries in {}",
            path.display()
        );

        // Validate the config
        let mut names = HashSet::new();
        for service in &config.services {
            // History is saved by name, so names must be unique.
            anyhow::ensure!(
                names.insert(&service.name),
                "duplicate service name {:?}",
                service.name
            );
            anyhow::ensure!(
                service.interval_secs > 0,
                "{}: interval_secs must be > 0",
                service.name
            );
            anyhow::ensure!(
                service.timeout_ms > 0,
                "{}: timeout_ms must be > 0",
                service.name
            );
        }

        Ok(config)
    }
}

impl ServiceConfig {
    pub fn interval(&self) -> Duration {
        Duration::from_secs(self.interval_secs)
    }

    pub fn slow(&self) -> Duration {
        Duration::from_millis(self.slow_ms)
    }

    pub fn timeout(&self) -> Duration {
        Duration::from_millis(self.timeout_ms)
    }
}
