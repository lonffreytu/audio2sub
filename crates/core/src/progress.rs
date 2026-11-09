use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressEvent {
    pub stage: String,
    pub percent: u8,
    pub message: String,
}

impl ProgressEvent {
    pub fn new(stage: impl Into<String>, percent: u8, message: impl Into<String>) -> Self {
        Self {
            stage: stage.into(),
            percent,
            message: message.into(),
        }
    }
}

pub type ProgressSink = Arc<dyn Fn(ProgressEvent) + Send + Sync>;

pub fn noop_progress() -> ProgressSink {
    Arc::new(|_| {})
}
