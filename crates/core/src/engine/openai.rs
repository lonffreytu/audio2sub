use crate::error::{Error, Result};
use crate::media::PreparedAudio;
use crate::model::Segment;
use crate::progress::{ProgressEvent, ProgressSink};
use crate::segmentation::split_text_to_segments;
use async_trait::async_trait;
use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use std::time::Duration;

use super::{AsrEngine, TranscribeOptions};

#[derive(Debug, Clone)]
pub struct OpenAiEngine {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl OpenAiEngine {
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Result<Self> {
        let base_url = base_url.into().trim().trim_end_matches('/').to_string();
        let api_key = api_key.into();
        let model = model.into();
        if base_url.is_empty() {
            return Err(Error::InvalidInput("Base URL 不能为空".into()));
        }
        if api_key.is_empty() {
            return Err(Error::InvalidInput("API Key 不能为空".into()));
        }
        if model.is_empty() {
            return Err(Error::InvalidInput("模型名称不能为空".into()));
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .build()?;
        Ok(Self {
            client,
            base_url,
            api_key,
            model,
        })
    }

    fn endpoint(&self) -> String {
        format!("{}/audio/transcriptions", self.base_url)
    }
}

#[async_trait]
impl AsrEngine for OpenAiEngine {
    async fn transcribe(
        &self,
        audio: &PreparedAudio,
        options: &TranscribeOptions,
        progress: &ProgressSink,
    ) -> Result<Vec<Segment>> {
        if options.cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        progress(ProgressEvent::new(
            "upload",
            15,
            format!("上传音频到 {}...", self.base_url),
        ));

        let part = Part::bytes(audio.bytes.clone())
            .file_name(audio.filename.clone())
            .mime_str(&audio.content_type)?;
        let mut form = Form::new()
            .part("file", part)
            .text("model", self.model.clone())
            .text("response_format", "verbose_json")
            .text("timestamp_granularities[]", "segment");
        if let Some(language) = &options.language {
            if !language.trim().is_empty() {
                form = form.text("language", language.clone());
            }
        }

        progress(ProgressEvent::new("recognize", 30, "等待识别结果..."));
        let response = self
            .client
            .post(self.endpoint())
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .await?;
        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            return Err(Error::Asr(format!(
                "API 错误 (HTTP {}): {}",
                status.as_u16(),
                extract_error_message(&body)
            )));
        }

        progress(ProgressEvent::new("parse", 80, "解析识别结果..."));
        let payload: WhisperResponse = serde_json::from_str(&body)?;
        let segments = if let Some(segments) = payload.segments.filter(|items| !items.is_empty()) {
            segments
                .into_iter()
                .filter_map(|segment| {
                    let text = segment.text.trim();
                    if text.is_empty() {
                        None
                    } else {
                        let start_ms = (segment.start * 1000.0).round() as i64;
                        let end_ms = ((segment.end * 1000.0).round() as i64).max(start_ms);
                        Some(Segment::new(start_ms, end_ms, text))
                    }
                })
                .collect()
        } else {
            split_text_to_segments(payload.text.trim(), audio.duration_ms)
        };

        progress(ProgressEvent::new("done", 95, "字幕生成完成"));
        Ok(segments)
    }
}

fn extract_error_message(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(|error| error.get("message"))
                .or_else(|| value.get("message"))
                .and_then(|message| message.as_str())
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| body.chars().take(300).collect())
}

#[derive(Debug, Deserialize)]
struct WhisperResponse {
    #[serde(default)]
    text: String,
    #[serde(default)]
    segments: Option<Vec<WhisperSegment>>,
}

#[derive(Debug, Deserialize)]
struct WhisperSegment {
    #[serde(default)]
    text: String,
    #[serde(default)]
    start: f64,
    #[serde(default)]
    end: f64,
}
