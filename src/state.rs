use std::sync::Arc;

use crate::monitor::{self, Monitors};

#[derive(Clone)]
pub struct AppState {
    pub name: Arc<str>,
    pub description: Option<Arc<str>>,
    pub banner: Option<Arc<str>>,
    pub monitors: Monitors,
}

impl AppState {
    pub fn from_config(config: &monitor::Config) -> Self {
        Self {
            name: config.name.as_str().into(),
            description: config.description.as_deref().map(Into::into),
            banner: config.banner.as_deref().map(Into::into),
            monitors: Monitors::default(),
        }
    }

    pub fn with_monitors(mut self, monitors: Monitors) -> Self {
        self.monitors = monitors;
        self
    }
}
