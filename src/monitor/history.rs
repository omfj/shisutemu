use std::{
    collections::HashMap,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

use super::{Health, Monitors};

pub const HOURS: usize = 24;

const SAVE_INTERVAL: Duration = Duration::from_mins(1);

/// History buckets per service name, as stored on disk.
pub type Saved = HashMap<String, Vec<Bucket>>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Bucket {
    /// Hours since the Unix epoch.
    hour: u64,
    worst: Health,
}

/// Worst health per hour for the last [`HOURS`] hours, kept in a ring buffer.
#[derive(Debug)]
pub struct History {
    buckets: [Option<Bucket>; HOURS],
}

impl History {
    pub fn new() -> Self {
        Self {
            buckets: [None; HOURS],
        }
    }

    /// Rebuilds a history from saved buckets, dropping any older than a day.
    pub fn restore(buckets: impl IntoIterator<Item = Bucket>) -> Self {
        let mut history = Self::new();
        history.merge(buckets);
        history
    }

    /// Merges saved buckets into this history, keeping the worst health per hour
    /// and dropping any older than a day.
    pub fn merge(&mut self, buckets: impl IntoIterator<Item = Bucket>) {
        self.merge_at(current_hour(), buckets);
    }

    /// The buckets from the last [`HOURS`] hours, for saving.
    pub fn buckets(&self) -> Vec<Bucket> {
        let now = current_hour();
        self.buckets
            .iter()
            .flatten()
            .filter(|bucket| is_recent(bucket.hour, now))
            .copied()
            .collect()
    }

    pub fn record(&mut self, health: Health) {
        self.record_at(current_hour(), health);
    }

    /// Worst health per hour, oldest first and ending with the current hour.
    /// `None` means there were no checks that hour.
    pub fn hours(&self) -> [Option<Health>; HOURS] {
        self.hours_at(current_hour())
    }

    fn record_at(&mut self, hour: u64, health: Health) {
        let slot = &mut self.buckets[hour as usize % HOURS];
        match slot {
            Some(bucket) if bucket.hour == hour => {
                if health.severity() > bucket.worst.severity() {
                    bucket.worst = health;
                }
            }
            // Empty, or left over from a previous day.
            _ => {
                *slot = Some(Bucket {
                    hour,
                    worst: health,
                })
            }
        }
    }

    fn merge_at(&mut self, now: u64, buckets: impl IntoIterator<Item = Bucket>) {
        for bucket in buckets {
            if !is_recent(bucket.hour, now) {
                continue;
            }
            // Don't let an older bucket overwrite a newer one in the same slot.
            let slot = self.buckets[bucket.hour as usize % HOURS];
            if slot.is_none_or(|existing| existing.hour <= bucket.hour) {
                self.record_at(bucket.hour, bucket.worst);
            }
        }
    }

    fn hours_at(&self, now: u64) -> [Option<Health>; HOURS] {
        std::array::from_fn(|i| {
            let hour = now - (HOURS - 1 - i) as u64;
            self.buckets[hour as usize % HOURS]
                .filter(|bucket| bucket.hour == hour)
                .map(|bucket| bucket.worst)
        })
    }
}

/// Reads saved history. If the file is missing, we treat it as empty history.
///
/// # Errors
///
/// This function returns and error if the file exists but cannot be read or parsed.
pub fn load(path: &Path) -> anyhow::Result<Saved> {
    match fs::read_to_string(path) {
        Ok(raw) => serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse {}", path.display())),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(Saved::new()),
        Err(err) => Err(err).with_context(|| format!("failed to read {}", path.display())),
    }
}

/// Writes every monitor's history. What is already in the file is merged in first.
pub fn save(path: &Path, monitors: &Monitors) -> anyhow::Result<()> {
    let mut saved = load(path).unwrap_or_else(|err| {
        tracing::warn!("{err:#}, overwriting it");
        Saved::new()
    });

    for monitor in monitors.iter() {
        if let Some(buckets) = saved.remove(&monitor.config.name) {
            monitor.merge_history(buckets);
        }
        saved.insert(monitor.config.name.clone(), monitor.history_buckets());
    }

    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    fs::write(&tmp, serde_json::to_vec(&saved)?)
        .with_context(|| format!("failed to write {}", Path::new(&tmp).display()))?;
    fs::rename(&tmp, path).with_context(|| format!("failed to replace {}", path.display()))
}

/// Saves history every [`SAVE_INTERVAL`] until the task is dropped.
pub async fn run(path: PathBuf, monitors: Monitors) {
    let mut interval = tokio::time::interval(SAVE_INTERVAL);
    interval.tick().await;
    loop {
        interval.tick().await;
        if let Err(err) = save(&path, &monitors) {
            tracing::warn!("failed to save history: {err:#}");
        }
    }
}

fn is_recent(hour: u64, now: u64) -> bool {
    hour <= now && now - hour < HOURS as u64
}

fn current_hour() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("should be able to get current time")
        .as_secs()
        / 3600
}
