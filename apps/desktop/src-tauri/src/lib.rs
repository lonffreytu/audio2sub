mod tools;

use audio2sub_core::{
    AppSettings, EndpointConfig, EndpointKind, EngineChoice, Ffmpeg, HistoryStore, ProgressEvent,
    ProgressSink, ProjectSummary, Segment, SettingsStore, SubtitleFormat, TranscribeRequest,
    TranscriptionResult, TranscriptionService,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager, State};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const KEYRING_SERVICE: &str = "Audio2Sub";

struct AppState {
    store: HistoryStore,
    settings: SettingsStore,
    tasks: Arc<Mutex<HashMap<String, CancellationToken>>>,
    app_data_dir: PathBuf,
    install_cancel: Arc<Mutex<Option<CancellationToken>>>,
}

#[derive(Debug, Serialize)]
struct EndpointView {
    id: String,
    name: String,
    kind: EndpointKind,
    base_url: String,
    model: String,
    has_api_key: bool,
}

#[derive(Debug, Serialize)]
struct SettingsView {
    endpoints: Vec<EndpointView>,
}

#[derive(Debug, Clone, Serialize)]
struct ProgressPayload {
    task_id: String,
    stage: String,
    percent: u8,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct FinishedPayload {
    task_id: String,
    result: Option<TranscriptionResult>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EndpointInput {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: String,
    kind: EndpointKind,
    #[serde(default)]
    base_url: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    api_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TranscribeInput {
    path: String,
    engine: String,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    task_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExportInput {
    #[serde(default)]
    project_id: Option<String>,
    segments: Vec<Segment>,
    format: SubtitleFormat,
    output_path: String,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let store = HistoryStore::open(data_dir.join("audio2sub.db"))?;
            let settings = SettingsStore::new(data_dir.join("settings.json"));
            app.manage(AppState {
                store,
                settings,
                tasks: Arc::new(Mutex::new(HashMap::new())),
                app_data_dir: data_dir,
                install_cancel: Arc::new(Mutex::new(None)),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            check_environment,
            install_ffmpeg,
            cancel_ffmpeg_install,
            set_ffmpeg_path,
            reset_ffmpeg_path,
            get_settings,
            save_endpoint,
            delete_endpoint,
            test_endpoint,
            list_history,
            get_project,
            delete_project,
            save_project_segments,
            transcribe_file,
            cancel_task,
            export_subtitles
        ])
        .run(tauri::generate_context!())
        .expect("error while running Audio2Sub");
}

#[tauri::command]
async fn check_environment(state: State<'_, AppState>) -> Result<tools::ToolStatus, String> {
    let settings = state.settings.load().map_err(display_error)?;
    Ok(tools::status(&state.app_data_dir, &settings).await)
}

#[tauri::command]
async fn install_ffmpeg(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<tools::ToolStatus, String> {
    let cancel = CancellationToken::new();
    {
        let mut slot = state
            .install_cancel
            .lock()
            .map_err(|_| "安装锁错误".to_string())?;
        if slot.is_some() {
            return Err("已有 FFmpeg 安装任务正在进行".to_string());
        }
        *slot = Some(cancel.clone());
    }

    let app_data_dir = state.app_data_dir.clone();
    let settings_store = state.settings.clone();
    let cancel_slot = state.install_cancel.clone();
    let progress_handle = app.clone();
    let progress = move |event: tools::InstallProgress| {
        let _ = progress_handle.emit("ffmpeg-install-progress", event);
    };

    let result = tools::install(&app_data_dir, cancel, &progress).await;
    if let Ok(mut slot) = cancel_slot.lock() {
        *slot = None;
    }
    result?;

    let settings = settings_store.load().map_err(display_error)?;
    Ok(tools::status(&app_data_dir, &settings).await)
}

#[tauri::command]
fn cancel_ffmpeg_install(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(token) = state
        .install_cancel
        .lock()
        .map_err(|_| "安装锁错误".to_string())?
        .as_ref()
    {
        token.cancel();
    }
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
async fn set_ffmpeg_path(
    state: State<'_, AppState>,
    ffmpeg_path: String,
) -> Result<tools::ToolStatus, String> {
    let (ffmpeg, ffprobe) = tools::validate_custom(Path::new(&ffmpeg_path)).await?;
    let mut settings = state.settings.load().map_err(display_error)?;
    settings.ffmpeg_path = Some(ffmpeg.display().to_string());
    settings.ffprobe_path = Some(ffprobe.display().to_string());
    state.settings.save(&settings).map_err(display_error)?;
    Ok(tools::status(&state.app_data_dir, &settings).await)
}

#[tauri::command]
async fn reset_ffmpeg_path(state: State<'_, AppState>) -> Result<tools::ToolStatus, String> {
    let mut settings = state.settings.load().map_err(display_error)?;
    settings.ffmpeg_path = None;
    settings.ffprobe_path = None;
    state.settings.save(&settings).map_err(display_error)?;
    Ok(tools::status(&state.app_data_dir, &settings).await)
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Result<SettingsView, String> {
    let settings = state.settings.load().map_err(display_error)?;
    let endpoints = settings
        .endpoints
        .iter()
        .map(|endpoint| endpoint_view(&settings, endpoint))
        .collect();
    Ok(SettingsView { endpoints })
}

#[tauri::command(rename_all = "snake_case")]
fn save_endpoint(state: State<'_, AppState>, input: EndpointInput) -> Result<EndpointView, String> {
    let mut settings = state.settings.load().map_err(display_error)?;
    let (default_base, default_model) = input.kind.defaults();
    let id = input
        .id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let name = if input.name.trim().is_empty() {
        input.kind.label().to_string()
    } else {
        input.name.trim().to_string()
    };
    let base_url = if input.base_url.trim().is_empty() {
        default_base.to_string()
    } else {
        input.base_url.trim().to_string()
    };
    let model = if input.model.trim().is_empty() {
        default_model.to_string()
    } else {
        input.model.trim().to_string()
    };

    if let Some(api_key) = input
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|key| !key.is_empty())
    {
        save_api_key(&id, api_key)?;
    }

    let config = EndpointConfig {
        id: id.clone(),
        name,
        kind: input.kind,
        base_url,
        model,
        api_key: None,
    };
    if let Some(existing) = settings
        .endpoints
        .iter_mut()
        .find(|endpoint| endpoint.id == id)
    {
        *existing = config;
    } else {
        settings.endpoints.push(config);
    }
    state.settings.save(&settings).map_err(display_error)?;
    let endpoint = settings
        .find_endpoint(&id)
        .ok_or_else(|| "接口保存失败".to_string())?;
    Ok(endpoint_view(&settings, endpoint))
}

#[tauri::command(rename_all = "snake_case")]
fn delete_endpoint(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let mut settings = state.settings.load().map_err(display_error)?;
    settings.endpoints.retain(|endpoint| endpoint.id != id);
    state.settings.save(&settings).map_err(display_error)?;
    delete_api_key(&id);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
async fn test_endpoint(state: State<'_, AppState>, input: EndpointInput) -> Result<String, String> {
    let (default_base, default_model) = input.kind.defaults();
    let base_url = if input.base_url.trim().is_empty() {
        default_base
    } else {
        input.base_url.trim()
    };
    let model = if input.model.trim().is_empty() {
        default_model
    } else {
        input.model.trim()
    };
    let api_key = input
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            let id = input.id.as_deref()?;
            let settings = state.settings.load().ok()?;
            resolve_api_key(&settings, id)
        })
        .ok_or_else(|| "API Key 未配置".to_string())?;

    let service = TranscriptionService::new(Ffmpeg::default(), state.store.clone());
    service
        .test_openai(base_url, &api_key, model)
        .await
        .map_err(display_error)?;
    Ok("连接成功".to_string())
}

#[tauri::command]
fn list_history(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<ProjectSummary>, String> {
    state
        .store
        .list_projects(limit.unwrap_or(50))
        .map_err(display_error)
}

#[tauri::command(rename_all = "snake_case")]
fn get_project(
    state: State<'_, AppState>,
    id: String,
) -> Result<Option<TranscriptionResult>, String> {
    state.store.get_project(&id).map_err(display_error)
}

#[tauri::command(rename_all = "snake_case")]
fn delete_project(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.store.delete_project(&id).map_err(display_error)
}

#[tauri::command(rename_all = "snake_case")]
fn save_project_segments(
    state: State<'_, AppState>,
    project_id: String,
    segments: Vec<Segment>,
) -> Result<(), String> {
    state
        .store
        .update_segments(&project_id, &segments)
        .map_err(display_error)
}

#[tauri::command(rename_all = "snake_case")]
async fn transcribe_file(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    input: TranscribeInput,
) -> Result<String, String> {
    let path = PathBuf::from(&input.path);
    if !path.exists() {
        return Err(format!("文件不存在: {}", path.display()));
    }
    let engine = resolve_engine(state.inner(), &input.engine)?;
    let service = build_service(state.inner()).await?;
    let task_id = input
        .task_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let cancel = CancellationToken::new();
    state
        .tasks
        .lock()
        .map_err(|_| "任务锁错误".to_string())?
        .insert(task_id.clone(), cancel.clone());

    let tasks = state.tasks.clone();
    let event_task_id = task_id.clone();
    let handle = app.clone();

    tauri::async_runtime::spawn(async move {
        let progress_handle = handle.clone();
        let progress_task_id = event_task_id.clone();
        let progress: ProgressSink = Arc::new(move |event: ProgressEvent| {
            let _ = progress_handle.emit(
                "transcription-progress",
                ProgressPayload {
                    task_id: progress_task_id.clone(),
                    stage: event.stage,
                    percent: event.percent,
                    message: event.message,
                },
            );
        });

        let request = TranscribeRequest {
            input: path,
            engine,
            language: input.language,
            cancel,
        };
        let outcome = service.transcribe(request, progress).await;
        let payload = match outcome {
            Ok(result) => FinishedPayload {
                task_id: event_task_id.clone(),
                result: Some(result),
                error: None,
            },
            Err(error) => FinishedPayload {
                task_id: event_task_id.clone(),
                result: None,
                error: Some(error.to_string()),
            },
        };
        let _ = handle.emit("transcription-finished", payload);
        if let Ok(mut tasks) = tasks.lock() {
            tasks.remove(&event_task_id);
        }
    });

    Ok(task_id)
}

#[tauri::command(rename_all = "snake_case")]
fn cancel_task(state: State<'_, AppState>, task_id: String) -> Result<(), String> {
    if let Some(token) = state
        .tasks
        .lock()
        .map_err(|_| "任务锁错误".to_string())?
        .get(&task_id)
    {
        token.cancel();
    }
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
fn export_subtitles(state: State<'_, AppState>, input: ExportInput) -> Result<String, String> {
    if let Some(project_id) = &input.project_id {
        state
            .store
            .update_segments(project_id, &input.segments)
            .map_err(display_error)?;
    }
    let output_path = PathBuf::from(&input.output_path);
    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(display_error)?;
        }
    }
    let text = audio2sub_core::subtitle::render(&input.segments, input.format);
    std::fs::write(&output_path, text).map_err(display_error)?;
    Ok(output_path.display().to_string())
}

async fn build_service(state: &AppState) -> Result<TranscriptionService, String> {
    let settings = state.settings.load().map_err(display_error)?;
    let tools = tools::resolve(&state.app_data_dir, &settings).await;
    if !tools::tool_available(&tools.ffmpeg).await || !tools::tool_available(&tools.ffprobe).await {
        return Err("未检测到 FFmpeg，请先安装 FFmpeg 组件".to_string());
    }
    Ok(TranscriptionService::new(
        tools.to_ffmpeg(),
        state.store.clone(),
    ))
}

fn endpoint_view(settings: &AppSettings, endpoint: &EndpointConfig) -> EndpointView {
    EndpointView {
        id: endpoint.id.clone(),
        name: endpoint.name.clone(),
        kind: endpoint.kind,
        base_url: endpoint.base_url.clone(),
        model: endpoint.model.clone(),
        has_api_key: resolve_api_key(settings, &endpoint.id).is_some(),
    }
}

fn resolve_engine(state: &AppState, engine: &str) -> Result<EngineChoice, String> {
    if engine == "bcut" {
        return Ok(EngineChoice::Bcut {
            api_base: None,
            bootstrap_url: None,
        });
    }
    if let Some(id) = engine.strip_prefix("endpoint:") {
        let settings = state.settings.load().map_err(display_error)?;
        let endpoint = settings
            .find_endpoint(id)
            .ok_or_else(|| "接口不存在".to_string())?;
        let api_key = resolve_api_key(&settings, id)
            .ok_or_else(|| format!("接口「{}」未配置 API Key", endpoint.name))?;
        return Ok(EngineChoice::Openai {
            base_url: endpoint.base_url.clone(),
            api_key,
            model: endpoint.model.clone(),
        });
    }
    Err("未知引擎".to_string())
}

fn keyring_entry(id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, id).map_err(display_error)
}

fn stored_api_key(id: &str) -> Option<String> {
    keyring_entry(id)
        .ok()?
        .get_password()
        .ok()
        .filter(|key| !key.is_empty())
}

fn resolve_api_key(settings: &AppSettings, id: &str) -> Option<String> {
    stored_api_key(id).or_else(|| {
        settings
            .find_endpoint(id)
            .and_then(|endpoint| endpoint.api_key.clone())
            .filter(|key| !key.is_empty())
    })
}

fn save_api_key(id: &str, api_key: &str) -> Result<(), String> {
    keyring_entry(id)?
        .set_password(api_key)
        .map_err(display_error)
}

fn delete_api_key(id: &str) {
    if let Ok(entry) = keyring_entry(id) {
        let _ = entry.delete_credential();
    }
}

fn display_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
