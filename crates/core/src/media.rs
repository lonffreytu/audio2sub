use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioTarget {
    Wav16k,
    OpenAiCompatible,
}

#[derive(Debug, Clone)]
pub struct PreparedAudio {
    pub bytes: Vec<u8>,
    pub filename: String,
    pub content_type: String,
    pub duration_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaInfo {
    pub duration_ms: i64,
    pub format_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Ffmpeg {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}

impl Default for Ffmpeg {
    fn default() -> Self {
        Self {
            ffmpeg: std::env::var_os("AUDIO2SUB_FFMPEG")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("ffmpeg")),
            ffprobe: std::env::var_os("AUDIO2SUB_FFPROBE")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("ffprobe")),
        }
    }
}

impl Ffmpeg {
    pub fn new(ffmpeg: impl Into<PathBuf>, ffprobe: impl Into<PathBuf>) -> Self {
        Self {
            ffmpeg: ffmpeg.into(),
            ffprobe: ffprobe.into(),
        }
    }

    pub async fn probe(&self, input: &Path) -> Result<MediaInfo> {
        let output = Command::new(&self.ffprobe)
            .arg("-v")
            .arg("error")
            .arg("-show_entries")
            .arg("format=duration,format_name:stream=duration")
            .arg("-of")
            .arg("json")
            .arg(input)
            .output()
            .await
            .map_err(|error| Error::Ffmpeg(format!("无法执行 ffprobe: {error}")))?;

        if !output.status.success() {
            return Err(Error::Ffmpeg(format!(
                "ffprobe 失败: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }

        let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        let duration = value
            .get("format")
            .and_then(|format| format.get("duration"))
            .and_then(value_as_f64)
            .or_else(|| {
                value
                    .get("streams")
                    .and_then(|streams| streams.as_array())
                    .and_then(|streams| streams.first())
                    .and_then(|stream| stream.get("duration"))
                    .and_then(value_as_f64)
            })
            .unwrap_or(0.0);
        let format_name = value
            .get("format")
            .and_then(|format| format.get("format_name"))
            .and_then(|name| name.as_str())
            .map(ToString::to_string);

        Ok(MediaInfo {
            duration_ms: (duration * 1000.0).round() as i64,
            format_name,
        })
    }

    pub async fn extract_wav(&self, input: &Path) -> Result<Vec<u8>> {
        let directory = tempfile::tempdir()?;
        let output_path = directory.path().join("audio.wav");
        let output = Command::new(&self.ffmpeg)
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-nostdin")
            .arg("-y")
            .arg("-i")
            .arg(input)
            .arg("-vn")
            .arg("-acodec")
            .arg("pcm_s16le")
            .arg("-ar")
            .arg("16000")
            .arg("-ac")
            .arg("1")
            .arg("-f")
            .arg("wav")
            .arg(&output_path)
            .output()
            .await
            .map_err(|error| Error::Ffmpeg(format!("无法执行 ffmpeg: {error}")))?;

        if !output.status.success() {
            return Err(Error::Ffmpeg(format!(
                "ffmpeg 提取音频失败: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }

        Ok(tokio::fs::read(&output_path).await?)
    }

    pub async fn prepare(&self, input: &Path, target: AudioTarget) -> Result<PreparedAudio> {
        let info = self.probe(input).await?;
        let extension = extension_of(input);
        let duration_ms = if info.duration_ms > 0 {
            info.duration_ms
        } else {
            0
        };

        if target == AudioTarget::OpenAiCompatible && is_supported_audio(&extension) {
            let bytes = tokio::fs::read(input).await?;
            let filename = input
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("audio.wav")
                .to_string();
            return Ok(PreparedAudio {
                bytes,
                filename,
                content_type: content_type_for(&extension).to_string(),
                duration_ms,
            });
        }

        let bytes = self.extract_wav(input).await?;
        let duration_ms = if duration_ms > 0 {
            duration_ms
        } else {
            wav_duration_ms(&bytes).unwrap_or(0)
        };
        Ok(PreparedAudio {
            bytes,
            filename: "audio.wav".to_string(),
            content_type: "audio/wav".to_string(),
            duration_ms,
        })
    }
}

fn value_as_f64(value: &serde_json::Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|text| text.parse::<f64>().ok()))
}

fn extension_of(path: &Path) -> String {
    path.extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

pub fn is_supported_audio(extension: &str) -> bool {
    matches!(
        extension,
        "mp3" | "wav" | "m4a" | "ogg" | "flac" | "opus" | "mpeg" | "mpga" | "aac" | "webm"
    )
}

pub fn content_type_for(extension: &str) -> &'static str {
    match extension {
        "mp3" => "audio/mp3",
        "wav" => "audio/wav",
        "m4a" => "audio/mp4",
        "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        "opus" => "audio/opus",
        "mpeg" | "mpga" => "audio/mpeg",
        "aac" => "audio/aac",
        "webm" => "audio/webm",
        _ => "application/octet-stream",
    }
}

pub fn silent_wav(duration_ms: u64) -> Vec<u8> {
    let sample_rate = 16_000u32;
    let channels = 1u16;
    let bits_per_sample = 16u16;
    let bytes_per_sample = u32::from(bits_per_sample / 8);
    let samples = (u64::from(sample_rate) * duration_ms / 1000) as u32;
    let data_size = samples * u32::from(channels) * bytes_per_sample;
    let byte_rate = sample_rate * u32::from(channels) * bytes_per_sample;
    let block_align = channels * (bits_per_sample / 8);

    let mut bytes = Vec::with_capacity(44 + data_size as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_size).to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&byte_rate.to_le_bytes());
    bytes.extend_from_slice(&block_align.to_le_bytes());
    bytes.extend_from_slice(&bits_per_sample.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_size.to_le_bytes());
    bytes.resize(44 + data_size as usize, 0);
    bytes
}

pub fn wav_duration_ms(bytes: &[u8]) -> Option<i64> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let mut offset = 12usize;
    let mut byte_rate = None;
    let mut data_size = None;
    while offset + 8 <= bytes.len() {
        let chunk_id = &bytes[offset..offset + 4];
        let chunk_size =
            u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().ok()?) as usize;
        let data_start = offset + 8;
        if chunk_id == b"fmt " && data_start + 12 <= bytes.len() {
            byte_rate = Some(u32::from_le_bytes(
                bytes[data_start + 8..data_start + 12].try_into().ok()?,
            ) as u64);
        } else if chunk_id == b"data" {
            let available = bytes.len().saturating_sub(data_start);
            data_size = Some(if chunk_size == 0 || chunk_size == u32::MAX as usize {
                available as u64
            } else {
                chunk_size.min(available) as u64
            });
            break;
        }
        offset = data_start
            .saturating_add(chunk_size)
            .saturating_add(chunk_size % 2);
    }
    let byte_rate = byte_rate.filter(|value| *value > 0)?;
    let data_size = data_size?;
    Some((data_size * 1000 / byte_rate) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn tool_ready(path: &Path) -> bool {
        Command::new(path)
            .arg("-version")
            .output()
            .await
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    #[test]
    fn generates_valid_silent_wav() {
        let bytes = silent_wav(1_200);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(wav_duration_ms(&bytes), Some(1_200));
    }

    #[tokio::test]
    async fn probes_and_extracts_audio() {
        let ffmpeg = Ffmpeg::default();
        if !tool_ready(&ffmpeg.ffprobe).await || !tool_ready(&ffmpeg.ffmpeg).await {
            eprintln!("skip: ffmpeg/ffprobe not available");
            return;
        }

        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("input.wav");
        tokio::fs::write(&input, silent_wav(1_200)).await.unwrap();

        let info = ffmpeg.probe(&input).await.unwrap();
        assert_eq!(info.duration_ms, 1_200);
        assert!(info.format_name.unwrap_or_default().contains("wav"));

        let wav = ffmpeg.extract_wav(&input).await.unwrap();
        assert_eq!(&wav[0..4], b"RIFF");
        let duration = wav_duration_ms(&wav).unwrap();
        assert!((1_150..=1_250).contains(&duration));
    }

    #[tokio::test]
    async fn prepares_supported_audio_without_conversion() {
        let ffmpeg = Ffmpeg::default();
        if !tool_ready(&ffmpeg.ffprobe).await {
            eprintln!("skip: ffprobe not available");
            return;
        }

        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("sample.wav");
        tokio::fs::write(&input, silent_wav(800)).await.unwrap();

        let prepared = ffmpeg
            .prepare(&input, AudioTarget::OpenAiCompatible)
            .await
            .unwrap();
        assert_eq!(prepared.filename, "sample.wav");
        assert_eq!(prepared.content_type, "audio/wav");
        assert_eq!(prepared.duration_ms, 800);
        assert_eq!(&prepared.bytes[0..4], b"RIFF");
    }
}
