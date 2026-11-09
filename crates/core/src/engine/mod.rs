pub mod bcut;
pub mod openai;

use crate::error::Result;
use crate::media::PreparedAudio;
use crate::model::Segment;
use crate::progress::ProgressSink;
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct TranscribeOptions {
    pub language: Option<String>,
    pub cancel: CancellationToken,
}

impl Default for TranscribeOptions {
    fn default() -> Self {
        Self {
            language: None,
            cancel: CancellationToken::new(),
        }
    }
}

#[async_trait]
pub trait AsrEngine: Send + Sync {
    async fn transcribe(
        &self,
        audio: &PreparedAudio,
        options: &TranscribeOptions,
        progress: &ProgressSink,
    ) -> Result<Vec<Segment>>;
}
