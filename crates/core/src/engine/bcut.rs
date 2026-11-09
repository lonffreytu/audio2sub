use crate::error::{Error, Result};
use crate::media::PreparedAudio;
use crate::model::Segment;
use crate::progress::{ProgressEvent, ProgressSink};
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE, ORIGIN, REFERER, USER_AGENT};
use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

use super::{AsrEngine, TranscribeOptions};

pub const DEFAULT_API_BASE: &str = "https://member.bilibili.com/x/bcut/rubick-interface";
pub const DEFAULT_BOOTSTRAP_URL: &str = "https://www.bilibili.com";
const BROWSER_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36";
const REQUEST_USER_AGENT: &str = "Bilibili/1.0.0 (https://www.bilibili.com)";

#[derive(Debug, Clone)]
pub struct BcutEngine {
    client: reqwest::Client,
    api_base: String,
    bootstrap_url: String,
}

impl Default for BcutEngine {
    fn default() -> Self {
        Self::new(DEFAULT_API_BASE, DEFAULT_BOOTSTRAP_URL)
    }
}

impl BcutEngine {
    pub fn new(api_base: impl Into<String>, bootstrap_url: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .cookie_store(true)
            .timeout(Duration::from_secs(300))
            .build()
            .expect("failed to build reqwest client");
        Self {
            client,
            api_base: api_base.into().trim_end_matches('/').to_string(),
            bootstrap_url: bootstrap_url.into(),
        }
    }

    fn headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(REQUEST_USER_AGENT));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            REFERER,
            HeaderValue::from_static("https://www.bilibili.com"),
        );
        headers.insert(ORIGIN, HeaderValue::from_static("https://www.bilibili.com"));
        headers
    }

    async fn bootstrap(&self) {
        let _ = self
            .client
            .get(&self.bootstrap_url)
            .header(USER_AGENT, BROWSER_USER_AGENT)
            .timeout(Duration::from_secs(10))
            .send()
            .await;
    }

    async fn post_data<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<T> {
        let response = self
            .client
            .post(format!("{}{}", self.api_base, path))
            .headers(Self::headers())
            .body(body.to_string())
            .timeout(Duration::from_secs(30))
            .send()
            .await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            return Err(Error::Asr(format!(
                "必剪请求失败 (HTTP {}): {}",
                status.as_u16(),
                truncate(&text, 300)
            )));
        }
        let envelope: Envelope<T> = serde_json::from_str(&text)?;
        if envelope.code != 0 {
            return Err(Error::Asr(format!(
                "必剪接口错误 ({}): {}",
                envelope.code,
                if envelope.message.is_empty() {
                    truncate(&text, 300)
                } else {
                    envelope.message
                }
            )));
        }
        envelope
            .data
            .ok_or_else(|| Error::Asr("必剪接口返回缺少 data".into()))
    }

    async fn create_resource(&self, size: usize) -> Result<CreateData> {
        let body = serde_json::json!({
            "type": 2,
            "name": "audio.mp3",
            "size": size,
            "ResourceFileType": "mp3",
            "model_id": "8",
        });
        self.post_data("/resource/create", &body).await
    }

    async fn upload_chunks(
        &self,
        bytes: &[u8],
        data: &CreateData,
        cancel: &CancellationToken,
        progress: &ProgressSink,
    ) -> Result<Vec<String>> {
        if data.per_size <= 0 {
            return Err(Error::Asr("必剪返回的分片大小无效".into()));
        }
        if data.upload_urls.is_empty() {
            return Err(Error::Asr("必剪未返回上传地址".into()));
        }
        let chunk_size = data.per_size as usize;
        let mut etags = Vec::new();
        for (index, url) in data.upload_urls.iter().enumerate() {
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            let start = index.saturating_mul(chunk_size);
            if start >= bytes.len() {
                break;
            }
            let end = (start + chunk_size).min(bytes.len());
            let percent = 20 + ((index + 1) * 20 / data.upload_urls.len()) as u8;
            progress(ProgressEvent::new(
                "upload",
                percent.min(40),
                format!("上传分片 {}/{}...", index + 1, data.upload_urls.len()),
            ));
            let response = self
                .client
                .put(url)
                .headers(Self::headers())
                .body(bytes[start..end].to_vec())
                .timeout(Duration::from_secs(120))
                .send()
                .await?;
            if !response.status().is_success() {
                return Err(Error::Asr(format!(
                    "必剪分片上传失败 (HTTP {})",
                    response.status().as_u16()
                )));
            }
            if let Some(etag) = response.headers().get("etag") {
                if let Ok(etag) = etag.to_str() {
                    etags.push(etag.to_string());
                }
            }
        }
        Ok(etags)
    }

    async fn commit_upload(&self, data: &CreateData, etags: &[String]) -> Result<String> {
        let body = serde_json::json!({
            "InBossKey": data.in_boss_key,
            "ResourceId": data.resource_id,
            "Etags": etags.join(","),
            "UploadId": data.upload_id,
            "model_id": "8",
        });
        let response: CompleteData = self.post_data("/resource/create/complete", &body).await?;
        Ok(response.download_url)
    }

    async fn create_task(&self, download_url: &str) -> Result<String> {
        let body = serde_json::json!({
            "resource": download_url,
            "model_id": "8",
        });
        let response: TaskData = self.post_data("/task", &body).await?;
        Ok(response.task_id)
    }

    async fn query_result(
        &self,
        task_id: &str,
        cancel: &CancellationToken,
        progress: &ProgressSink,
    ) -> Result<Vec<Segment>> {
        for attempt in 0..300 {
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            let response = self
                .client
                .get(format!("{}/task/result", self.api_base))
                .headers(Self::headers())
                .query(&[("model_id", "7"), ("task_id", task_id)])
                .timeout(Duration::from_secs(15))
                .send()
                .await?;

            if response.status().as_u16() == 412 {
                sleep_or_cancel(Duration::from_secs(3), cancel).await?;
                continue;
            }

            let status = response.status();
            let text = response.text().await?;
            if !status.is_success() {
                return Err(Error::Asr(format!(
                    "必剪查询失败 (HTTP {}): {}",
                    status.as_u16(),
                    truncate(&text, 300)
                )));
            }
            let envelope: Envelope<QueryData> = serde_json::from_str(&text)?;
            if envelope.code != 0 {
                return Err(Error::Asr(format!(
                    "必剪查询错误 ({}): {}",
                    envelope.code, envelope.message
                )));
            }
            let data = envelope
                .data
                .ok_or_else(|| Error::Asr("必剪查询缺少 data".into()))?;

            match data.state {
                4 => {
                    progress(ProgressEvent::new("parse", 90, "解析识别结果..."));
                    return parse_result(data.result);
                }
                3 if data.result.is_some() => {
                    progress(ProgressEvent::new("parse", 90, "解析识别结果..."));
                    return parse_result(data.result);
                }
                2 | 3 => {
                    let message = data
                        .err
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "未知错误".to_string());
                    return Err(Error::Asr(format!("必剪识别失败: {message}")));
                }
                _ => {
                    let percent = 50 + (attempt / 12).min(30) as u8;
                    progress(ProgressEvent::new(
                        "recognize",
                        percent,
                        "必剪识别中，请稍候...",
                    ));
                }
            }

            sleep_or_cancel(Duration::from_secs(1), cancel).await?;
        }
        Err(Error::Asr("必剪识别超时（300 秒）".into()))
    }
}

#[async_trait]
impl AsrEngine for BcutEngine {
    async fn transcribe(
        &self,
        audio: &PreparedAudio,
        options: &TranscribeOptions,
        progress: &ProgressSink,
    ) -> Result<Vec<Segment>> {
        if audio.bytes.is_empty() {
            return Err(Error::InvalidInput("音频内容为空".into()));
        }
        if options.cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }

        progress(ProgressEvent::new("prepare", 5, "准备必剪识别..."));
        self.bootstrap().await;

        progress(ProgressEvent::new("upload", 10, "请求上传凭证..."));
        let create = self.create_resource(audio.bytes.len()).await?;
        let etags = self
            .upload_chunks(&audio.bytes, &create, &options.cancel, progress)
            .await?;

        if options.cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        progress(ProgressEvent::new("upload", 40, "提交识别任务..."));
        let download_url = self.commit_upload(&create, &etags).await?;
        let task_id = self.create_task(&download_url).await?;

        progress(ProgressEvent::new("recognize", 50, "必剪识别中，请稍候..."));
        self.query_result(&task_id, &options.cancel, progress).await
    }
}

async fn sleep_or_cancel(duration: Duration, cancel: &CancellationToken) -> Result<()> {
    tokio::select! {
        _ = tokio::time::sleep(duration) => Ok(()),
        _ = cancel.cancelled() => Err(Error::Cancelled),
    }
}

fn parse_result(result: Option<serde_json::Value>) -> Result<Vec<Segment>> {
    let Some(result) = result else {
        return Ok(Vec::new());
    };
    let data: ResultData = match result {
        serde_json::Value::String(text) => serde_json::from_str(&text)?,
        other => serde_json::from_value(other)?,
    };
    let segments = data
        .utterances
        .into_iter()
        .filter_map(|utterance| {
            let text = utterance.transcript.trim();
            if text.is_empty() {
                None
            } else {
                let start_ms = utterance.start_time as i64;
                let end_ms = (utterance.end_time as i64).max(start_ms);
                Some(Segment::new(start_ms, end_ms, text))
            }
        })
        .collect();
    Ok(segments)
}

fn truncate(value: &str, length: usize) -> String {
    value.chars().take(length).collect()
}

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    code: i64,
    #[serde(default, alias = "msg")]
    message: String,
    data: Option<T>,
}

#[derive(Debug, Deserialize)]
struct CreateData {
    in_boss_key: String,
    resource_id: String,
    upload_id: String,
    upload_urls: Vec<String>,
    #[serde(deserialize_with = "deserialize_i64")]
    per_size: i64,
}

#[derive(Debug, Deserialize)]
struct CompleteData {
    download_url: String,
}

#[derive(Debug, Deserialize)]
struct TaskData {
    task_id: String,
}

#[derive(Debug, Deserialize)]
struct QueryData {
    #[serde(default)]
    state: i64,
    #[serde(default)]
    result: Option<serde_json::Value>,
    #[serde(default)]
    err: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct ResultData {
    #[serde(default)]
    utterances: Vec<Utterance>,
}

#[derive(Debug, Deserialize)]
struct Utterance {
    #[serde(default)]
    transcript: String,
    #[serde(default)]
    start_time: f64,
    #[serde(default)]
    end_time: f64,
}

fn deserialize_i64<'de, D>(deserializer: D) -> std::result::Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_f64().map(|value| value as i64))
            .ok_or_else(|| D::Error::custom("invalid integer")),
        serde_json::Value::String(text) => text
            .parse::<i64>()
            .map_err(|error| D::Error::custom(error.to_string())),
        _ => Err(D::Error::custom("invalid integer")),
    }
}
