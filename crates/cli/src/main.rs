use anyhow::{bail, Context};
use audio2sub_core::service::EngineChoice;
use audio2sub_core::{
    Ffmpeg, HistoryStore, MediaInfo, ProgressEvent, ProgressSink, SubtitleFormat,
    TranscribeRequest, TranscriptionResult, TranscriptionService,
};
use clap::{Parser, Subcommand, ValueEnum};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Parser)]
#[command(name = "audio2sub", version, about = "本地音频/视频转字幕工具")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
#[allow(clippy::large_enum_variant)]
enum Command {
    #[command(about = "转写字幕")]
    Transcribe(TranscribeArgs),
    #[command(about = "查看识别历史")]
    History(HistoryArgs),
    #[command(about = "读取媒体信息")]
    Probe(ProbeArgs),
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum EngineArg {
    Bcut,
    Openai,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum FormatArg {
    Srt,
    Vtt,
    Txt,
}

impl From<FormatArg> for SubtitleFormat {
    fn from(value: FormatArg) -> Self {
        match value {
            FormatArg::Srt => Self::Srt,
            FormatArg::Vtt => Self::Vtt,
            FormatArg::Txt => Self::Txt,
        }
    }
}

#[derive(Debug, Parser)]
struct TranscribeArgs {
    input: PathBuf,
    #[arg(short, long)]
    output: Option<PathBuf>,
    #[arg(short, long, value_enum, default_value_t = FormatArg::Srt)]
    format: FormatArg,
    #[arg(short, long, value_enum, default_value_t = EngineArg::Bcut)]
    engine: EngineArg,
    #[arg(long, env = "AUDIO2SUB_BASE_URL")]
    base_url: Option<String>,
    #[arg(long, env = "AUDIO2SUB_API_KEY")]
    api_key: Option<String>,
    #[arg(long, env = "AUDIO2SUB_MODEL")]
    model: Option<String>,
    #[arg(short, long)]
    language: Option<String>,
    #[arg(long)]
    data_dir: Option<PathBuf>,
    #[arg(long, hide = true)]
    bcut_api_base: Option<String>,
    #[arg(long, hide = true)]
    bcut_bootstrap_url: Option<String>,
}

#[derive(Debug, Parser)]
struct HistoryArgs {
    #[arg(long)]
    data_dir: Option<PathBuf>,
    #[arg(short, long, default_value_t = 50)]
    limit: usize,
}

#[derive(Debug, Parser)]
struct ProbeArgs {
    input: PathBuf,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("错误: {error:#}");
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Transcribe(args) => transcribe(args).await,
        Command::History(args) => history(args),
        Command::Probe(args) => probe(args).await,
    }
}

async fn transcribe(args: TranscribeArgs) -> anyhow::Result<()> {
    if !args.input.exists() {
        bail!("文件不存在: {}", args.input.display());
    }

    let format: SubtitleFormat = args.format.into();
    let output_path = args
        .output
        .unwrap_or_else(|| args.input.with_extension(format.extension()));
    let data_dir = resolve_data_dir(args.data_dir);
    let store = HistoryStore::open(data_dir.join("audio2sub.db"))
        .with_context(|| format!("无法打开历史数据库: {}", data_dir.display()))?;
    let service = TranscriptionService::new(Ffmpeg::default(), store);

    let engine = match args.engine {
        EngineArg::Bcut => EngineChoice::Bcut {
            api_base: args.bcut_api_base,
            bootstrap_url: args.bcut_bootstrap_url,
        },
        EngineArg::Openai => {
            let api_key = args
                .api_key
                .or_else(|| std::env::var("OPENAI_API_KEY").ok())
                .filter(|key| !key.trim().is_empty())
                .context("请通过 --api-key、AUDIO2SUB_API_KEY 或 OPENAI_API_KEY 提供 API Key")?;
            EngineChoice::Openai {
                base_url: args
                    .base_url
                    .unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
                api_key,
                model: args.model.unwrap_or_else(|| "whisper-1".to_string()),
            }
        }
    };

    let cancel = CancellationToken::new();
    let cancel_signal = cancel.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            cancel_signal.cancel();
        }
    });

    let progress: ProgressSink = Arc::new(|event: ProgressEvent| {
        eprintln!("[{:3}%] {}", event.percent, event.message);
    });

    let request = TranscribeRequest {
        input: args.input.clone(),
        engine,
        language: args.language,
        cancel,
    };
    let result = service.transcribe(request, progress).await?;
    write_subtitle(&result, &output_path, format)?;
    println!("{}", output_path.display());
    Ok(())
}

fn history(args: HistoryArgs) -> anyhow::Result<()> {
    let data_dir = resolve_data_dir(args.data_dir);
    let store = HistoryStore::open(data_dir.join("audio2sub.db"))?;
    let projects = store.list_projects(args.limit)?;
    if projects.is_empty() {
        println!("暂无识别记录");
        return Ok(());
    }
    println!(
        "{:<36}  {:<25}  {:<20}  {:>5}  文件",
        "ID", "时间", "引擎", "条数"
    );
    for project in projects {
        println!(
            "{:<36}  {:<25}  {:<20}  {:>5}  {}",
            project.id, project.created_at, project.engine, project.segment_count, project.filename
        );
    }
    Ok(())
}

async fn probe(args: ProbeArgs) -> anyhow::Result<()> {
    if !args.input.exists() {
        bail!("文件不存在: {}", args.input.display());
    }
    let info: MediaInfo = Ffmpeg::default().probe(&args.input).await?;
    println!("{}", serde_json::to_string_pretty(&info)?);
    Ok(())
}

fn write_subtitle(
    result: &TranscriptionResult,
    output_path: &Path,
    format: SubtitleFormat,
) -> anyhow::Result<()> {
    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let text = audio2sub_core::subtitle::render(&result.segments, format);
    std::fs::write(output_path, text)
        .with_context(|| format!("无法写入字幕文件: {}", output_path.display()))?;
    Ok(())
}

fn resolve_data_dir(explicit: Option<PathBuf>) -> PathBuf {
    explicit
        .or_else(|| std::env::var_os("AUDIO2SUB_DATA_DIR").map(PathBuf::from))
        .or_else(|| dirs::data_dir().map(|path| path.join("Audio2Sub")))
        .unwrap_or_else(|| PathBuf::from("."))
}
