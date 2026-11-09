use serde_json::json;
use std::process::Stdio;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn tool_ready(name: &str) -> bool {
    tokio::process::Command::new(name)
        .arg("-version")
        .output()
        .await
        .map(|output| output.status.success())
        .unwrap_or(false)
}

async fn mount_bcut_mocks(server: &MockServer) {
    let upload_url = format!("{}/upload/0", server.uri());
    let download_url = format!("{}/download/audio.wav", server.uri());

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .mount(server)
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
        .mount(server)
        .await;

    Mock::given(method("PUT"))
        .and(path("/upload/0"))
        .respond_with(ResponseTemplate::new(200).insert_header("ETag", "\"etag-1\""))
        .mount(server)
        .await;

    Mock::given(method("POST"))
        .and(path("/x/bcut/rubick-interface/resource/create/complete"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "code": 0,
            "data": {"download_url": download_url}
        })))
        .mount(server)
        .await;

    Mock::given(method("POST"))
        .and(path("/x/bcut/rubick-interface/task"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "code": 0,
            "data": {"task_id": "task-1"}
        })))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/x/bcut/rubick-interface/task/result"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "code": 0,
            "data": {
                "state": 4,
                "result": "{\"utterances\":[{\"transcript\":\"必剪结果\",\"start_time\":0,\"end_time\":1200}]}"
            }
        })))
        .mount(server)
        .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn cli_transcribes_and_lists_history() {
    if !tool_ready("ffprobe").await {
        eprintln!("skip: ffprobe not available");
        return;
    }

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

    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("demo.wav");
    let output = directory.path().join("demo.srt");
    let data_dir = directory.path().join("data");
    tokio::fs::write(&input, audio2sub_core::media::silent_wav(1_200))
        .await
        .unwrap();

    let binary = env!("CARGO_BIN_EXE_audio2sub");
    let result = tokio::process::Command::new(binary)
        .arg("transcribe")
        .arg(&input)
        .arg("--engine")
        .arg("openai")
        .arg("--base-url")
        .arg(format!("{}/v1", server.uri()))
        .arg("--api-key")
        .arg("test-key")
        .arg("--model")
        .arg("whisper-1")
        .arg("--output")
        .arg(&output)
        .arg("--data-dir")
        .arg(&data_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .unwrap();

    assert!(
        result.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let srt = tokio::fs::read_to_string(&output).await.unwrap();
    assert!(srt.contains("Hello"));
    assert!(srt.contains("00:00:00,000 --> 00:00:01,200"));

    let history = tokio::process::Command::new(binary)
        .arg("history")
        .arg("--data-dir")
        .arg(&data_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .unwrap();
    assert!(
        history.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&history.stderr)
    );
    let history_text = String::from_utf8_lossy(&history.stdout);
    assert!(history_text.contains("demo.wav"));
    assert!(history_text.contains("openai:whisper-1"));
}

#[tokio::test(flavor = "multi_thread")]
async fn cli_transcribes_with_bcut_mock() {
    if !tool_ready("ffprobe").await || !tool_ready("ffmpeg").await {
        eprintln!("skip: ffmpeg/ffprobe not available");
        return;
    }

    let server = MockServer::start().await;
    mount_bcut_mocks(&server).await;

    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("bcut-demo.wav");
    let output = directory.path().join("bcut-demo.srt");
    let data_dir = directory.path().join("data");
    tokio::fs::write(&input, audio2sub_core::media::silent_wav(1_200))
        .await
        .unwrap();

    let binary = env!("CARGO_BIN_EXE_audio2sub");
    let result = tokio::process::Command::new(binary)
        .arg("transcribe")
        .arg(&input)
        .arg("--engine")
        .arg("bcut")
        .arg("--bcut-api-base")
        .arg(format!("{}/x/bcut/rubick-interface", server.uri()))
        .arg("--bcut-bootstrap-url")
        .arg(server.uri())
        .arg("--output")
        .arg(&output)
        .arg("--data-dir")
        .arg(&data_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .unwrap();

    assert!(
        result.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let srt = tokio::fs::read_to_string(&output).await.unwrap();
    assert!(srt.contains("必剪结果"));
    assert!(srt.contains("00:00:00,000 --> 00:00:01,200"));
}
