use audio2sub_core::engine::bcut::BcutEngine;
use audio2sub_core::engine::openai::OpenAiEngine;
use audio2sub_core::{
    noop_progress, AsrEngine, EngineChoice, Ffmpeg, HistoryStore, PreparedAudio, ProgressEvent,
    TranscribeOptions, TranscribeRequest, TranscriptionService,
};
use serde_json::json;
use std::sync::{Arc, Mutex};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn sample_audio() -> PreparedAudio {
    PreparedAudio {
        bytes: audio2sub_core::media::silent_wav(1_200),
        filename: "audio.wav".into(),
        content_type: "audio/wav".into(),
        duration_ms: 1_200,
    }
}

#[tokio::test]
async fn openai_engine_parses_segments() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/audio/transcriptions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "segments": [
                {"text": "Hello", "start": 0.0, "end": 1.2}
            ]
        })))
        .mount(&server)
        .await;

    let engine =
        OpenAiEngine::new(format!("{}/v1", server.uri()), "test-key", "whisper-1").unwrap();
    let segments = engine
        .transcribe(
            &sample_audio(),
            &TranscribeOptions {
                language: Some("en".into()),
                cancel: CancellationToken::new(),
            },
            &noop_progress(),
        )
        .await
        .unwrap();

    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].text, "Hello");
    assert_eq!(segments[0].start_ms, 0);
    assert_eq!(segments[0].end_ms, 1_200);
}

#[tokio::test]
async fn openai_engine_falls_back_to_plain_text() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/audio/transcriptions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "text": "你好。世界！",
            "segments": []
        })))
        .mount(&server)
        .await;

    let engine =
        OpenAiEngine::new(format!("{}/v1", server.uri()), "test-key", "whisper-1").unwrap();
    let mut audio = sample_audio();
    audio.duration_ms = 2_000;
    let segments = engine
        .transcribe(&audio, &TranscribeOptions::default(), &noop_progress())
        .await
        .unwrap();

    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0].text, "你好");
    assert_eq!(segments[1].text, "世界");
    assert_eq!(segments.last().unwrap().end_ms, 2_000);
}

#[tokio::test]
async fn openai_engine_reports_api_errors() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/audio/transcriptions"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "error": {"message": "invalid api key"}
        })))
        .mount(&server)
        .await;

    let engine = OpenAiEngine::new(format!("{}/v1", server.uri()), "bad-key", "whisper-1").unwrap();
    let error = engine
        .transcribe(
            &sample_audio(),
            &TranscribeOptions::default(),
            &noop_progress(),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("invalid api key"));
}

#[tokio::test]
async fn bcut_engine_completes_full_flow() {
    let server = MockServer::start().await;
    let api_base = format!("{}/x/bcut/rubick-interface", server.uri());
    let upload_url = format!("{}/upload/0", server.uri());
    let download_url = format!("{}/download/audio.wav", server.uri());

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/x/bcut/rubick-interface/resource/create"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "code": 0,
            "data": {
                "in_boss_key": "boss-key",
                "resource_id": "resource-id",
                "upload_id": "upload-id",
                "upload_urls": [upload_url],
                "per_size": 1_000_000
            }
        })))
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/upload/0"))
        .respond_with(ResponseTemplate::new(200).insert_header("ETag", "\"etag-1\""))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/x/bcut/rubick-interface/resource/create/complete"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "code": 0,
            "data": {"download_url": download_url}
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/x/bcut/rubick-interface/task"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "code": 0,
            "data": {"task_id": "task-1"}
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/x/bcut/rubick-interface/task/result"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "code": 0,
            "data": {
                "state": 4,
                "result": "{\"utterances\":[{\"transcript\":\"你好\",\"start_time\":0,\"end_time\":1200}]}"
            }
        })))
        .mount(&server)
        .await;

    let engine = BcutEngine::new(api_base, server.uri());
    let segments = engine
        .transcribe(
            &sample_audio(),
            &TranscribeOptions::default(),
            &noop_progress(),
        )
        .await
        .unwrap();

    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].text, "你好");
    assert_eq!(segments[0].end_ms, 1_200);
}

#[tokio::test]
async fn bcut_engine_reports_api_errors() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/x/bcut/rubick-interface/resource/create"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "code": -1,
            "message": "bcut unavailable"
        })))
        .mount(&server)
        .await;

    let engine = BcutEngine::new(
        format!("{}/x/bcut/rubick-interface", server.uri()),
        server.uri(),
    );
    let error = engine
        .transcribe(
            &sample_audio(),
            &TranscribeOptions::default(),
            &noop_progress(),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("bcut unavailable"));
}

async fn tool_ready(name: &str) -> bool {
    Command::new(name)
        .arg("-version")
        .output()
        .await
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[tokio::test]
async fn service_transcribes_and_persists() {
    if !tool_ready("ffprobe").await {
        eprintln!("skip: ffprobe not available");
        return;
    }

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/audio/transcriptions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "segments": [
                {"text": "第一句", "start": 0.0, "end": 0.8},
                {"text": "第二句", "start": 0.8, "end": 2.0}
            ]
        })))
        .mount(&server)
        .await;

    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("demo.wav");
    tokio::fs::write(&input, audio2sub_core::media::silent_wav(2_000))
        .await
        .unwrap();
    let store = HistoryStore::open(directory.path().join("history.db")).unwrap();
    let service = TranscriptionService::new(Ffmpeg::default(), store);

    let events = Arc::new(Mutex::new(Vec::<ProgressEvent>::new()));
    let progress_events = events.clone();
    let progress: audio2sub_core::ProgressSink = Arc::new(move |event| {
        progress_events.lock().unwrap().push(event);
    });

    let result = service
        .transcribe(
            TranscribeRequest {
                input,
                engine: EngineChoice::Openai {
                    base_url: format!("{}/v1", server.uri()),
                    api_key: "test-key".into(),
                    model: "whisper-1".into(),
                },
                language: None,
                cancel: CancellationToken::new(),
            },
            progress,
        )
        .await
        .unwrap();

    assert_eq!(result.segments.len(), 2);
    assert_eq!(result.filename, "demo.wav");
    assert_eq!(result.duration_ms, 2_000);
    let loaded = service
        .store()
        .get_project(&result.project_id)
        .unwrap()
        .unwrap();
    assert_eq!(loaded.segments, result.segments);
    assert!(events
        .lock()
        .unwrap()
        .iter()
        .any(|event| event.percent == 100));
}

#[tokio::test]
async fn service_respects_cancellation() {
    let directory = tempfile::tempdir().unwrap();
    let store = HistoryStore::open(directory.path().join("history.db")).unwrap();
    let service = TranscriptionService::new(Ffmpeg::default(), store);
    let cancel = CancellationToken::new();
    cancel.cancel();

    let error = service
        .transcribe(
            TranscribeRequest {
                input: directory.path().join("missing.wav"),
                engine: EngineChoice::Bcut {
                    api_base: None,
                    bootstrap_url: None,
                },
                language: None,
                cancel,
            },
            noop_progress(),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, audio2sub_core::Error::Cancelled));
}
