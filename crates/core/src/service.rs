use crate::engine::bcut::BcutEngine;
use crate::engine::openai::OpenAiEngine;
use crate::engine::{AsrEngine, TranscribeOptions};
use crate::error::{Error, Result};
use crate::media::{AudioTarget, Ffmpeg, PreparedAudio};
use crate::model::TranscriptionResult;
use crate::progress::{noop_progress, ProgressEvent, ProgressSink};
use crate::storage::HistoryStore;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "engine", rename_all = "snake_case")]
pub enum EngineChoice {
    Bcut {
        #[serde(default)]
        api_base: Option<String>,
        #[serde(default)]
        bootstrap_url: Option<String>,
    },
    Openai {
        base_url: String,
        api_key: String,
        model: String,
    },
}

impl EngineChoice {
    pub fn label(&self) -> String {
        match self {
            Self::Bcut { .. } => "bcut".to_string(),
            Self::Openai { model, .. } => format!("openai:{model}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TranscribeRequest {
    pub input: PathBuf,
    pub engine: EngineChoice,
    pub language: Option<String>,
    pub cancel: CancellationToken,
}

impl TranscribeRequest {
    pub fn new(input: impl Into<PathBuf>, engine: EngineChoice) -> Self {
        Self {
            input: input.into(),
            engine,
            language: None,
            cancel: CancellationToken::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TranscriptionService {
    ffmpeg: Ffmpeg,
    store: HistoryStore,
}

impl TranscriptionService {
    pub fn new(ffmpeg: Ffmpeg, store: HistoryStore) -> Self {
        Self { ffmpeg, store }
    }

    pub fn store(&self) -> &HistoryStore {
        &self.store
    }

    pub fn ffmpeg(&self) -> &Ffmpeg {
        &self.ffmpeg
    }

    pub async fn transcribe(
        &self,
        request: TranscribeRequest,
        progress: ProgressSink,
    ) -> Result<TranscriptionResult> {
        if request.cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        if !request.input.exists() {
            return Err(Error::InvalidInput(format!(
                "文件不存在: {}",
                request.input.display()
            )));
        }

        progress(ProgressEvent::new("probe", 5, "读取媒体信息..."));
        let target = match &request.engine {
            EngineChoice::Bcut { .. } => AudioTarget::Wav16k,
            EngineChoice::Openai { .. } => AudioTarget::OpenAiCompatible,
        };
        let audio = self.ffmpeg.prepare(&request.input, target).await?;
        if request.cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        if audio.bytes.is_empty() {
            return Err(Error::InvalidInput("音频内容为空".into()));
        }

        progress(ProgressEvent::new("prepare", 15, "音频准备完成"));
        let options = TranscribeOptions {
            language: request.language.clone(),
            cancel: request.cancel.clone(),
        };
        let engine: Box<dyn AsrEngine> = match &request.engine {
            EngineChoice::Bcut {
                api_base,
                bootstrap_url,
            } => Box::new(BcutEngine::new(
                api_base
                    .clone()
                    .unwrap_or_else(|| crate::engine::bcut::DEFAULT_API_BASE.to_string()),
                bootstrap_url
                    .clone()
                    .unwrap_or_else(|| crate::engine::bcut::DEFAULT_BOOTSTRAP_URL.to_string()),
            )),
            EngineChoice::Openai {
                base_url,
                api_key,
                model,
            } => Box::new(OpenAiEngine::new(base_url, api_key, model)?),
        };

        let segments = engine.transcribe(&audio, &options, &progress).await?;
        if request.cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        if segments.is_empty() {
            return Err(Error::Asr("未识别到语音内容".into()));
        }

        let filename = request
            .input
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("audio")
            .to_string();
        let result = TranscriptionResult {
            project_id: Uuid::new_v4().to_string(),
            filename,
            engine: request.engine.label(),
            duration_ms: audio.duration_ms,
            segments,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        self.store.save_project(&result)?;
        progress(ProgressEvent::new("done", 100, "识别完成"));
        Ok(result)
    }

    pub async fn test_openai(&self, base_url: &str, api_key: &str, model: &str) -> Result<()> {
        let engine = OpenAiEngine::new(base_url, api_key, model)?;
        let audio = PreparedAudio {
            bytes: crate::media::silent_wav(500),
            filename: "test.wav".to_string(),
            content_type: "audio/wav".to_string(),
            duration_ms: 500,
        };
        let options = TranscribeOptions::default();
        engine
            .transcribe(&audio, &options, &noop_progress())
            .await?;
        Ok(())
    }
}
