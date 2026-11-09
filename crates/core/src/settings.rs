use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndpointKind {
    Openai,
    Groq,
    #[serde(rename = "siliconflow")]
    SiliconFlow,
}

impl EndpointKind {
    pub fn defaults(self) -> (&'static str, &'static str) {
        match self {
            Self::Openai => ("https://api.openai.com/v1", "whisper-1"),
            Self::Groq => ("https://api.groq.com/openai/v1", "whisper-large-v3"),
            Self::SiliconFlow => (
                "https://api.siliconflow.cn/v1",
                "FunAudioLLM/SenseVoiceSmall",
            ),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Openai => "OpenAI",
            Self::Groq => "Groq",
            Self::SiliconFlow => "硅基流动",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointConfig {
    pub id: String,
    pub name: String,
    pub kind: EndpointKind,
    pub base_url: String,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

impl EndpointConfig {
    pub fn masked_key(&self) -> String {
        match self.api_key.as_deref().filter(|key| !key.is_empty()) {
            Some(key) if key.chars().count() > 12 => {
                let chars: Vec<char> = key.chars().collect();
                let prefix: String = chars.iter().take(8).collect();
                let suffix: String = chars.iter().skip(chars.len() - 4).collect();
                format!("{prefix}****{suffix}")
            }
            Some(_) => "****".to_string(),
            None => String::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default)]
    pub endpoints: Vec<EndpointConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ffmpeg_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ffprobe_path: Option<String>,
}

impl AppSettings {
    pub fn find_endpoint(&self, id: &str) -> Option<&EndpointConfig> {
        self.endpoints.iter().find(|endpoint| endpoint.id == id)
    }
}

#[derive(Debug, Clone)]
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<AppSettings> {
        if !self.path.exists() {
            return Ok(AppSettings::default());
        }
        let text = std::fs::read_to_string(&self.path)?;
        if text.trim().is_empty() {
            return Ok(AppSettings::default());
        }
        Ok(serde_json::from_str(&text)?)
    }

    pub fn save(&self, settings: &AppSettings) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(settings)?;
        std::fs::write(&self.path, text)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_settings() {
        let directory = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(directory.path().join("settings.json"));
        let mut settings = AppSettings::default();
        settings.endpoints.push(EndpointConfig {
            id: "ep1".into(),
            name: "Groq".into(),
            kind: EndpointKind::Groq,
            base_url: EndpointKind::Groq.defaults().0.into(),
            model: EndpointKind::Groq.defaults().1.into(),
            api_key: Some("gsk_1234567890abcdef".into()),
        });
        store.save(&settings).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.endpoints.len(), 1);
        assert_eq!(loaded.endpoints[0].name, "Groq");
        assert_eq!(loaded.endpoints[0].masked_key(), "gsk_1234****cdef");
    }

    #[test]
    fn returns_default_when_missing() {
        let directory = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(directory.path().join("missing.json"));
        assert!(store.load().unwrap().endpoints.is_empty());
    }
}
