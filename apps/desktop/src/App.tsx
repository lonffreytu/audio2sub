import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { ask, open, save } from "@tauri-apps/plugin-dialog";
import type {
  EndpointDraft,
  EndpointKind,
  EndpointView,
  EnvironmentStatus,
  FinishedPayload,
  InstallProgress,
  Message,
  ProgressPayload,
  ProjectSummary,
  Segment,
  SettingsView,
  SubtitleFormat,
  TranscriptionResult,
} from "./types";

const KIND_DEFAULTS: Record<
  EndpointKind,
  { label: string; base_url: string; model: string }
> = {
  openai: {
    label: "OpenAI",
    base_url: "https://api.openai.com/v1",
    model: "whisper-1",
  },
  groq: {
    label: "Groq",
    base_url: "https://api.groq.com/openai/v1",
    model: "whisper-large-v3",
  },
  siliconflow: {
    label: "硅基流动",
    base_url: "https://api.siliconflow.cn/v1",
    model: "FunAudioLLM/SenseVoiceSmall",
  },
};

const MEDIA_EXTENSIONS = [
  "mp3",
  "wav",
  "m4a",
  "flac",
  "ogg",
  "opus",
  "aac",
  "mp4",
  "mkv",
  "mov",
  "webm",
  "avi",
  "flv",
  "wmv",
  "ts",
  "m4v",
];

const LANGUAGES = [
  ["", "自动检测"],
  ["zh", "中文"],
  ["en", "English"],
  ["ja", "日本語"],
  ["ko", "한국어"],
  ["yue", "粤语"],
  ["fr", "Français"],
  ["de", "Deutsch"],
  ["es", "Español"],
  ["ru", "Русский"],
];

function basename(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

function formatDuration(ms: number): string {
  const total = Math.max(0, Math.round(ms / 1000));
  const minutes = Math.floor(total / 60);
  const seconds = total % 60;
  return `${minutes}:${String(seconds).padStart(2, "0")}`;
}

function formatTime(ms: number): string {
  const value = Math.max(0, Math.round(ms));
  const hours = Math.floor(value / 3_600_000);
  const minutes = Math.floor((value % 3_600_000) / 60_000);
  const seconds = Math.floor((value % 60_000) / 1000);
  const millis = value % 1000;
  return `${String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}.${String(millis).padStart(3, "0")}`;
}

function parseTime(text: string): number | null {
  const match = text
    .trim()
    .match(/^(?:(\d+):)?(\d{1,2}):(\d{1,2})(?:[.,](\d{1,3}))?$/);
  if (!match) return null;
  const hours = Number(match[1] ?? 0);
  const minutes = Number(match[2]);
  const seconds = Number(match[3]);
  const millis = Number((match[4] ?? "0").padEnd(3, "0"));
  return ((hours * 60 + minutes) * 60 + seconds) * 1000 + millis;
}

function formatBytes(bytes: number): string {
  if (!bytes) return "未知大小";
  const megabytes = bytes / 1_000_000;
  return `${megabytes.toFixed(megabytes >= 100 ? 0 : 1)} MB`;
}

function sourceLabel(source?: string): string {
  switch (source) {
    case "env":
      return "环境变量";
    case "custom":
      return "自定义路径";
    case "managed":
      return "应用内安装";
    case "system":
      return "系统 PATH";
    default:
      return "未检测到";
  }
}

function formatDate(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString("zh-CN", { hour12: false });
}

function TimeField({
  value,
  onChange,
}: {
  value: number;
  onChange: (value: number) => void;
}) {
  const [text, setText] = useState(formatTime(value));
  useEffect(() => setText(formatTime(value)), [value]);
  return (
    <input
      className="time-field"
      value={text}
      onChange={(event) => setText(event.target.value)}
      onBlur={() => {
        const parsed = parseTime(text);
        if (parsed === null) {
          setText(formatTime(value));
        } else {
          onChange(parsed);
        }
      }}
    />
  );
}

function ContactRow({
  label,
  value,
  onCopy,
}: {
  label: string;
  value: string;
  onCopy: () => void;
}) {
  return (
    <div className="contact-row">
      <div className="contact-main">
        <div className="contact-label">{label}</div>
        <div className="contact-value">{value}</div>
      </div>
      <button className="mini-button" onClick={onCopy}>
        复制
      </button>
    </div>
  );
}

export default function App() {
  const [theme, setTheme] = useState<"dark" | "light">(() =>
    localStorage.getItem("theme") === "light" ? "light" : "dark",
  );
  const [endpoints, setEndpoints] = useState<EndpointView[]>([]);
  const [engine, setEngine] = useState("bcut");
  const [language, setLanguage] = useState("");
  const [filePath, setFilePath] = useState("");
  const [taskId, setTaskId] = useState("");
  const [progress, setProgress] = useState<ProgressPayload | null>(null);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<TranscriptionResult | null>(null);
  const [segments, setSegments] = useState<Segment[]>([]);
  const [history, setHistory] = useState<ProjectSummary[]>([]);
  const [message, setMessage] = useState<Message | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [aboutOpen, setAboutOpen] = useState(false);
  const [environment, setEnvironment] = useState<EnvironmentStatus | null>(null);
  const [ffmpegSetupOpen, setFfmpegSetupOpen] = useState(false);
  const [ffmpegInstalling, setFfmpegInstalling] = useState(false);
  const [ffmpegProgress, setFfmpegProgress] = useState<InstallProgress | null>(null);
  const [ffmpegError, setFfmpegError] = useState("");
  const [ffmpegDismissed, setFfmpegDismissed] = useState(
    () => localStorage.getItem("ffmpeg-setup-dismissed") === "1",
  );
  const [draft, setDraft] = useState<EndpointDraft | null>(null);
  const taskIdRef = useRef("");
  const runningRef = useRef(false);

  const refreshEnvironment = useCallback(async () => {
    const status = await invoke<EnvironmentStatus>("check_environment");
    setEnvironment(status);
    return status;
  }, []);

  const refreshSettings = useCallback(async () => {
    const view = await invoke<SettingsView>("get_settings");
    setEndpoints(view.endpoints);
  }, []);

  const refreshHistory = useCallback(async () => {
    const items = await invoke<ProjectSummary[]>("list_history", { limit: 50 });
    setHistory(items);
  }, []);

  useEffect(() => {
    refreshSettings().catch((error) => {
      setMessage({ type: "error", text: String(error) });
    });
    refreshHistory().catch(() => {});
    refreshEnvironment().catch(() => {});
  }, [refreshSettings, refreshHistory, refreshEnvironment]);

  useEffect(() => {
    if (
      environment &&
      (!environment.ffmpeg || !environment.ffprobe) &&
      !ffmpegDismissed
    ) {
      setFfmpegSetupOpen(true);
    }
  }, [environment, ffmpegDismissed]);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    localStorage.setItem("theme", theme);
  }, [theme]);

  useEffect(() => {
    runningRef.current = running;
  }, [running]);

  useEffect(() => {
    if (!message) return;
    const timer = window.setTimeout(() => setMessage(null), 4200);
    return () => window.clearTimeout(timer);
  }, [message]);

  useEffect(() => {
    if (!settingsOpen && !aboutOpen && !ffmpegSetupOpen) return;
    const handler = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setSettingsOpen(false);
        setAboutOpen(false);
        if (!ffmpegInstalling) setFfmpegSetupOpen(false);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [settingsOpen, aboutOpen, ffmpegSetupOpen, ffmpegInstalling]);

  useEffect(() => {
    const listener = listen<InstallProgress>(
      "ffmpeg-install-progress",
      (event) => {
        setFfmpegProgress(event.payload);
        setFfmpegInstalling(true);
      },
    );
    return () => {
      listener.then((unlisten) => unlisten()).catch(() => {});
    };
  }, []);

  useEffect(() => {
    const progressListener = listen<ProgressPayload>(
      "transcription-progress",
      (event) => {
        if (event.payload.task_id === taskIdRef.current) {
          setProgress(event.payload);
        }
      },
    );
    const finishedListener = listen<FinishedPayload>(
      "transcription-finished",
      (event) => {
        if (event.payload.task_id !== taskIdRef.current) return;
        setRunning(false);
        setTaskId("");
        taskIdRef.current = "";
        if (event.payload.error) {
          setProgress(null);
          setMessage({ type: "error", text: event.payload.error });
          return;
        }
        const payload = event.payload.result;
        if (payload) {
          setResult(payload);
          setSegments(payload.segments);
          setProgress({
            task_id: payload.project_id,
            stage: "done",
            percent: 100,
            message: "识别完成",
          });
          setMessage({
            type: "success",
            text: `识别完成，共 ${payload.segments.length} 条字幕`,
          });
          refreshHistory().catch(() => {});
        }
      },
    );
    return () => {
      progressListener.then((unlisten) => unlisten()).catch(() => {});
      finishedListener.then((unlisten) => unlisten()).catch(() => {});
    };
  }, [refreshHistory]);

  const selectFile = useCallback(
    (path: string) => {
      if (runningRef.current) return;
      setFilePath(path);
      setResult(null);
      setSegments([]);
      setProgress(null);
      setMessage(null);
    },
    [],
  );

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "drop" && event.payload.paths.length > 0) {
          selectFile(event.payload.paths[0]);
        }
      })
      .then((dispose) => {
        unlisten = dispose;
      })
      .catch(() => {});
    return () => {
      unlisten?.();
    };
  }, [selectFile]);

  useEffect(() => {
    if (!engine.startsWith("endpoint:")) return;
    if (!endpoints.some((endpoint) => `endpoint:${endpoint.id}` === engine)) {
      setEngine("bcut");
    }
  }, [endpoints, engine]);

  const pickFile = async () => {
    if (running) return;
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "音频/视频", extensions: MEDIA_EXTENSIONS }],
      });
      if (typeof selected === "string") selectFile(selected);
    } catch (error) {
      setMessage({ type: "error", text: String(error) });
    }
  };

  const openFfmpegSetup = () => {
    setFfmpegError("");
    setFfmpegSetupOpen(true);
  };

  const dismissFfmpegSetup = () => {
    if (ffmpegInstalling) return;
    setFfmpegSetupOpen(false);
    setFfmpegDismissed(true);
    localStorage.setItem("ffmpeg-setup-dismissed", "1");
  };

  const installFfmpeg = async () => {
    setFfmpegError("");
    setFfmpegInstalling(true);
    setFfmpegProgress(null);
    try {
      const status = await invoke<EnvironmentStatus>("install_ffmpeg");
      setEnvironment(status);
      setFfmpegInstalling(false);
      setFfmpegProgress(null);
      setFfmpegSetupOpen(false);
      setFfmpegDismissed(false);
      localStorage.removeItem("ffmpeg-setup-dismissed");
      setMessage({ type: "success", text: "FFmpeg 安装完成" });
    } catch (error) {
      setFfmpegInstalling(false);
      setFfmpegError(String(error));
    }
  };

  const cancelFfmpegInstall = async () => {
    try {
      await invoke("cancel_ffmpeg_install");
      setMessage({ type: "info", text: "正在取消 FFmpeg 安装..." });
    } catch (error) {
      setFfmpegError(String(error));
    }
  };

  const chooseFfmpegPath = async () => {
    try {
      const selected = await open({ multiple: false, directory: false });
      if (typeof selected !== "string") return;
      const status = await invoke<EnvironmentStatus>("set_ffmpeg_path", {
        ffmpeg_path: selected,
      });
      setEnvironment(status);
      setFfmpegError("");
      setFfmpegSetupOpen(false);
      setMessage({ type: "success", text: "FFmpeg 路径已更新" });
    } catch (error) {
      setFfmpegError(String(error));
    }
  };

  const resetFfmpegPath = async () => {
    try {
      const status = await invoke<EnvironmentStatus>("reset_ffmpeg_path");
      setEnvironment(status);
      setMessage({ type: "success", text: "已恢复自动检测" });
    } catch (error) {
      setFfmpegError(String(error));
    }
  };

  const startTranscription = async () => {
    if (!filePath) {
      setMessage({ type: "error", text: "请先选择音频或视频文件" });
      return;
    }
    if (!environment?.ffmpeg || !environment?.ffprobe) {
      setMessage({ type: "error", text: "请先安装 FFmpeg" });
      openFfmpegSetup();
      return;
    }
    const endpoint = endpoints.find((item) => `endpoint:${item.id}` === engine);
    if (endpoint && !endpoint.has_api_key) {
      setMessage({ type: "error", text: "该接口尚未配置 API Key" });
      return;
    }
    const id = crypto.randomUUID();
    taskIdRef.current = id;
    setTaskId(id);
    setRunning(true);
    setResult(null);
    setSegments([]);
    setProgress({ task_id: id, stage: "start", percent: 0, message: "正在启动..." });
    setMessage(null);
    try {
      await invoke<string>("transcribe_file", {
        input: {
          path: filePath,
          engine,
          language: language || null,
          task_id: id,
        },
      });
    } catch (error) {
      taskIdRef.current = "";
      setTaskId("");
      setRunning(false);
      setProgress(null);
      setMessage({ type: "error", text: String(error) });
    }
  };

  const cancelTranscription = async () => {
    if (!taskId) return;
    try {
      await invoke("cancel_task", { task_id: taskId });
      setMessage({ type: "info", text: "正在取消任务..." });
    } catch (error) {
      setMessage({ type: "error", text: String(error) });
    }
  };

  const updateSegment = (index: number, patch: Partial<Segment>) => {
    setSegments((current) =>
      current.map((segment, position) =>
        position === index ? { ...segment, ...patch } : segment,
      ),
    );
  };

  const addSegment = (afterIndex?: number) => {
    setSegments((current) => {
      const position = afterIndex === undefined ? current.length : afterIndex + 1;
      const before = current[position - 1];
      const after = current[position];
      const start = before ? before.end_ms : 0;
      const end = after ? Math.max(after.start_ms, start + 1000) : start + 1500;
      const next = [...current];
      next.splice(position, 0, { start_ms: start, end_ms: end, text: "" });
      return next;
    });
  };

  const removeSegment = (index: number) => {
    setSegments((current) => current.filter((_, position) => position !== index));
  };

  const saveEdits = async () => {
    if (!result) return;
    try {
      await invoke("save_project_segments", {
        project_id: result.project_id,
        segments,
      });
      setMessage({ type: "success", text: "修改已保存" });
    } catch (error) {
      setMessage({ type: "error", text: String(error) });
    }
  };

  const exportSubtitles = async (format: SubtitleFormat) => {
    if (!result) return;
    const base = (result.filename || "subtitle").replace(/\.[^.]+$/, "");
    try {
      const outputPath = await save({
        defaultPath: `${base}.${format}`,
        filters: [{ name: format.toUpperCase(), extensions: [format] }],
      });
      if (!outputPath) return;
      const written = await invoke<string>("export_subtitles", {
        input: {
          project_id: result.project_id,
          segments,
          format,
          output_path: outputPath,
        },
      });
      setMessage({ type: "success", text: `已导出 ${written}` });
    } catch (error) {
      setMessage({ type: "error", text: String(error) });
    }
  };

  const loadProject = async (id: string) => {
    if (runningRef.current) return;
    try {
      const project = await invoke<TranscriptionResult | null>("get_project", {
        id,
      });
      if (!project) {
        setMessage({ type: "error", text: "项目不存在" });
        return;
      }
      setResult(project);
      setSegments(project.segments);
      setFilePath("");
      setProgress(null);
      setRunning(false);
    } catch (error) {
      setMessage({ type: "error", text: String(error) });
    }
  };

  const deleteProject = async (id: string, filename: string) => {
    const confirmed = await ask(`删除历史记录「${filename}」？`, {
      title: "确认删除",
      kind: "warning",
    });
    if (!confirmed) return;
    try {
      await invoke("delete_project", { id });
      await refreshHistory();
      if (result?.project_id === id) {
        setResult(null);
        setSegments([]);
      }
      setMessage({ type: "success", text: "历史记录已删除" });
    } catch (error) {
      setMessage({ type: "error", text: String(error) });
    }
  };

  const startNewEndpoint = () => {
    setDraft({
      name: "",
      kind: "openai",
      base_url: KIND_DEFAULTS.openai.base_url,
      model: KIND_DEFAULTS.openai.model,
      api_key: "",
    });
  };

  const editEndpoint = (endpoint: EndpointView) => {
    setDraft({
      id: endpoint.id,
      name: endpoint.name,
      kind: endpoint.kind,
      base_url: endpoint.base_url,
      model: endpoint.model,
      api_key: "",
    });
  };

  const changeDraftKind = (kind: EndpointKind) => {
    setDraft((current) =>
      current
        ? {
            ...current,
            kind,
            base_url: KIND_DEFAULTS[kind].base_url,
            model: KIND_DEFAULTS[kind].model,
          }
        : current,
    );
  };

  const saveEndpoint = async () => {
    if (!draft) return;
    try {
      await invoke<EndpointView>("save_endpoint", {
        input: {
          id: draft.id ?? null,
          name: draft.name,
          kind: draft.kind,
          base_url: draft.base_url,
          model: draft.model,
          api_key: draft.api_key.trim() || null,
        },
      });
      await refreshSettings();
      setDraft(null);
      setMessage({ type: "success", text: "接口已保存" });
    } catch (error) {
      setMessage({ type: "error", text: String(error) });
    }
  };

  const testEndpoint = async (input: EndpointDraft | EndpointView) => {
    try {
      const message = await invoke<string>("test_endpoint", {
        input: {
          id: input.id ?? null,
          name: input.name,
          kind: input.kind,
          base_url: input.base_url,
          model: input.model,
          api_key: "api_key" in input ? input.api_key.trim() || null : null,
        },
      });
      setMessage({ type: "success", text: `${input.name || "接口"}: ${message}` });
    } catch (error) {
      setMessage({ type: "error", text: `${input.name || "接口"}: ${String(error)}` });
    }
  };

  const deleteEndpoint = async (endpoint: EndpointView) => {
    const confirmed = await ask(`删除接口「${endpoint.name}」？`, {
      title: "确认删除",
      kind: "warning",
    });
    if (!confirmed) return;
    try {
      await invoke("delete_endpoint", { id: endpoint.id });
      await refreshSettings();
      if (draft?.id === endpoint.id) setDraft(null);
      setMessage({ type: "success", text: "接口已删除" });
    } catch (error) {
      setMessage({ type: "error", text: String(error) });
    }
  };

  const copyText = async (text: string, label: string) => {
    try {
      if (!navigator.clipboard) {
        throw new Error("clipboard unavailable");
      }
      await navigator.clipboard.writeText(text);
      setMessage({ type: "success", text: `已复制${label}` });
    } catch {
      setMessage({ type: "error", text: "复制失败，请手动选择文本" });
    }
  };

  const selectedEndpoint = endpoints.find(
    (endpoint) => `endpoint:${endpoint.id}` === engine,
  );
  const startDisabled =
    !filePath ||
    running ||
    (selectedEndpoint !== undefined && !selectedEndpoint.has_api_key);

  return (
    <div className="app">
      <header className="header">
        <div className="brand">
          <div className="brand-mark">A2S</div>
          <div>
            <h1>Audio2Sub</h1>
            <p>本地字幕工作台</p>
          </div>
        </div>
        <div className="header-controls">
          <label className="control">
            <span>识别引擎</span>
            <select value={engine} onChange={(event) => setEngine(event.target.value)}>
              <option value="bcut">必剪（B站，免费）</option>
              {endpoints.map((endpoint) => (
                <option key={endpoint.id} value={`endpoint:${endpoint.id}`}>
                  {endpoint.name}
                  {endpoint.has_api_key ? "" : "（未配置 Key）"}
                </option>
              ))}
            </select>
          </label>
          <label className="control language-control">
            <span>语言</span>
            <select value={language} onChange={(event) => setLanguage(event.target.value)}>
              {LANGUAGES.map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </select>
          </label>
          <button
            className="btn secondary"
            onClick={() => {
              setDraft(null);
              setSettingsOpen(true);
            }}
          >
            设置
          </button>
          <button className="btn secondary" onClick={() => setAboutOpen(true)}>
            关于
          </button>
          <button
            className="theme-button"
            title="切换主题"
            onClick={() => setTheme(theme === "dark" ? "light" : "dark")}
          >
            {theme === "dark" ? "浅色" : "深色"}
          </button>
        </div>
      </header>

      {environment && (!environment.ffmpeg || !environment.ffprobe) && (
        <div className="env-warning">
          <span>未检测到 FFmpeg，音视频处理功能不可用。</span>
          <button className="mini-button" onClick={openFfmpegSetup}>
            安装 FFmpeg
          </button>
        </div>
      )}

      <main className="layout">
        <aside className="sidebar">
          <section className="card">
            <div className="card-title">输入文件</div>
            <div
              className={`drop-zone ${filePath ? "has-file" : ""} ${running ? "disabled" : ""}`}
              onClick={running ? undefined : pickFile}
            >
              {filePath ? (
                <>
                  <div className="file-name">{basename(filePath)}</div>
                  <div className="file-path">{filePath}</div>
                </>
              ) : (
                <>
                  <div className="drop-mark">A2S</div>
                  <div className="drop-text">拖入音频或视频文件</div>
                  <div className="hint">或点击选择文件</div>
                </>
              )}
            </div>
            {selectedEndpoint && !selectedEndpoint.has_api_key && (
              <div className="warning">该接口尚未配置 API Key，请先到设置中填写。</div>
            )}
            <div className="action-row">
              <button
                className="btn"
                disabled={startDisabled}
                onClick={startTranscription}
              >
                {running ? "识别中..." : "开始识别"}
              </button>
              {running && (
                <button className="btn secondary" onClick={cancelTranscription}>
                  取消
                </button>
              )}
            </div>
            {progress && (
              <div className="progress-block">
                <div className="progress-bar">
                  <div style={{ width: `${progress.percent}%` }} />
                </div>
                <div className="progress-text">
                  {progress.percent}% · {progress.message}
                </div>
              </div>
            )}
          </section>

          <section className="card history-card">
            <div className="card-title">历史记录</div>
            <div className="history-list">
              {history.length === 0 && <div className="empty-small">暂无记录</div>}
              {history.map((item) => (
                <div
                  key={item.id}
                  className={`history-item ${result?.project_id === item.id ? "active" : ""}`}
                  onClick={() => loadProject(item.id)}
                >
                  <div className="history-main">
                    <div className="history-name">{item.filename}</div>
                    <div className="history-meta">
                      {item.engine} · {item.segment_count} 条 · {formatDate(item.created_at)}
                    </div>
                  </div>
                  <button
                    className="icon-button"
                    title="删除"
                    onClick={(event) => {
                      event.stopPropagation();
                      deleteProject(item.id, item.filename);
                    }}
                  >
                    ×
                  </button>
                </div>
              ))}
            </div>
          </section>
        </aside>

        <section className="workspace">
          {result ? (
            <div className="editor">
              <div className="editor-header">
                <div className="editor-title">
                  <h2>{result.filename}</h2>
                  <p>
                    {result.engine} · {segments.length} 条字幕 ·{" "}
                    {formatDuration(result.duration_ms)}
                  </p>
                </div>
                <div className="editor-actions">
                  <button className="btn secondary" onClick={() => addSegment()}>
                    新增字幕
                  </button>
                  <button className="btn secondary" onClick={saveEdits}>
                    保存修改
                  </button>
                  <button className="btn" onClick={() => exportSubtitles("srt")}>
                    导出 SRT
                  </button>
                  <button
                    className="btn secondary"
                    onClick={() => exportSubtitles("vtt")}
                  >
                    VTT
                  </button>
                  <button
                    className="btn secondary"
                    onClick={() => exportSubtitles("txt")}
                  >
                    TXT
                  </button>
                </div>
              </div>
              <div className="table-wrap">
                <table className="editor-table">
                  <thead>
                    <tr>
                      <th className="index-column">#</th>
                      <th className="time-column">开始</th>
                      <th className="time-column">结束</th>
                      <th>字幕文本</th>
                      <th className="actions-column"></th>
                    </tr>
                  </thead>
                  <tbody>
                    {segments.map((segment, index) => (
                      <tr key={index}>
                        <td className="index-column">{index + 1}</td>
                        <td className="time-column">
                          <TimeField
                            value={segment.start_ms}
                            onChange={(value) =>
                              updateSegment(index, { start_ms: value })
                            }
                          />
                        </td>
                        <td className="time-column">
                          <TimeField
                            value={segment.end_ms}
                            onChange={(value) =>
                              updateSegment(index, { end_ms: value })
                            }
                          />
                        </td>
                        <td>
                          <textarea
                            rows={2}
                            value={segment.text}
                            onChange={(event) =>
                              updateSegment(index, { text: event.target.value })
                            }
                          />
                        </td>
                        <td className="actions-column">
                          <button
                            className="mini-button"
                            onClick={() => addSegment(index)}
                          >
                            插入
                          </button>
                          <button
                            className="mini-button danger"
                            onClick={() => removeSegment(index)}
                          >
                            删除
                          </button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
                {segments.length === 0 && (
                  <div className="empty-small">还没有字幕，点击“新增字幕”开始编辑。</div>
                )}
              </div>
            </div>
          ) : (
            <div className="empty-state">
              <div className="empty-mark">A2S</div>
              <h2>还没有字幕</h2>
              <p>选择文件并开始识别，或从左侧历史记录打开项目。</p>
            </div>
          )}
        </section>
      </main>

      {settingsOpen && (
        <div className="modal-backdrop" onClick={() => setSettingsOpen(false)}>
          <div className="modal" onClick={(event) => event.stopPropagation()}>
            <div className="modal-header">
              <div>
                <h2>识别接口</h2>
                <p className="hint">
                  必剪无需配置。其他接口使用 OpenAI 兼容的 /audio/transcriptions，
                  API Key 保存在系统凭据管理器。
                </p>
              </div>
              <button className="icon-button" onClick={() => setSettingsOpen(false)}>
                ×
              </button>
            </div>

            <div className="ffmpeg-settings">
              <div className="ffmpeg-settings-head">
                <div>
                  <h3>FFmpeg 工具</h3>
                  <p className="hint">用于提取音轨和读取媒体信息。</p>
                </div>
                <span
                  className={`status-pill ${
                    environment?.ffmpeg && environment?.ffprobe ? "ok" : "missing"
                  }`}
                >
                  {environment?.ffmpeg && environment?.ffprobe ? "已就绪" : "缺失"}
                </span>
              </div>
              <div className="ffmpeg-meta">
                <span>{sourceLabel(environment?.source)}</span>
                {environment?.version && (
                  <span className="ffmpeg-version">{environment.version}</span>
                )}
                {environment?.ffmpeg_path && (
                  <span className="ffmpeg-path">{environment.ffmpeg_path}</span>
                )}
              </div>
              <div className="form-actions">
                <button className="mini-button" onClick={openFfmpegSetup}>
                  {environment?.ffmpeg && environment?.ffprobe ? "管理" : "安装"}
                </button>
                <button className="mini-button" onClick={chooseFfmpegPath}>
                  选择路径
                </button>
                {environment?.source === "custom" && (
                  <button className="mini-button" onClick={resetFfmpegPath}>
                    恢复自动
                  </button>
                )}
                <button
                  className="mini-button"
                  onClick={() =>
                    refreshEnvironment().catch((error) =>
                      setMessage({ type: "error", text: String(error) }),
                    )
                  }
                >
                  重新检测
                </button>
              </div>
            </div>

            <div className="endpoint-list">
              {endpoints.length === 0 && (
                <div className="empty-small">还没有自定义接口</div>
              )}
              {endpoints.map((endpoint) => (
                <div className="endpoint-row" key={endpoint.id}>
                  <div className="endpoint-main">
                    <div className="endpoint-name">
                      {endpoint.name}
                      <span className={`status-dot ${endpoint.has_api_key ? "ok" : ""}`} />
                    </div>
                    <div className="endpoint-meta">
                      {KIND_DEFAULTS[endpoint.kind].label} · {endpoint.model}
                    </div>
                    <div className="endpoint-url">{endpoint.base_url}</div>
                  </div>
                  <div className="endpoint-actions">
                    <button className="mini-button" onClick={() => testEndpoint(endpoint)}>
                      测试
                    </button>
                    <button className="mini-button" onClick={() => editEndpoint(endpoint)}>
                      编辑
                    </button>
                    <button
                      className="mini-button danger"
                      onClick={() => deleteEndpoint(endpoint)}
                    >
                      删除
                    </button>
                  </div>
                </div>
              ))}
            </div>

            {draft ? (
              <div className="endpoint-form">
                <div className="form-row">
                  <label>
                    名称
                    <input
                      value={draft.name}
                      placeholder="例如 Groq 主账号"
                      onChange={(event) =>
                        setDraft({ ...draft, name: event.target.value })
                      }
                    />
                  </label>
                  <label>
                    类型
                    <select
                      value={draft.kind}
                      onChange={(event) =>
                        changeDraftKind(event.target.value as EndpointKind)
                      }
                    >
                      {Object.entries(KIND_DEFAULTS).map(([value, config]) => (
                        <option key={value} value={value}>
                          {config.label}
                        </option>
                      ))}
                    </select>
                  </label>
                </div>
                <label>
                  Base URL
                  <input
                    value={draft.base_url}
                    onChange={(event) =>
                      setDraft({ ...draft, base_url: event.target.value })
                    }
                  />
                </label>
                <label>
                  模型
                  <input
                    value={draft.model}
                    onChange={(event) =>
                      setDraft({ ...draft, model: event.target.value })
                    }
                  />
                </label>
                <label>
                  API Key
                  <input
                    type="password"
                    value={draft.api_key}
                    placeholder={draft.id ? "留空则保留已保存的 Key" : "粘贴 API Key"}
                    onChange={(event) =>
                      setDraft({ ...draft, api_key: event.target.value })
                    }
                  />
                </label>
                <div className="form-actions">
                  <button className="btn" onClick={saveEndpoint}>
                    保存
                  </button>
                  <button className="btn secondary" onClick={() => testEndpoint(draft)}>
                    测试连接
                  </button>
                  <button className="btn secondary" onClick={() => setDraft(null)}>
                    取消
                  </button>
                </div>
              </div>
            ) : (
              <button className="btn" onClick={startNewEndpoint}>
                添加接口
              </button>
            )}
          </div>
        </div>
      )}

      {ffmpegSetupOpen && (
        <div
          className="modal-backdrop"
          onClick={ffmpegInstalling ? undefined : dismissFfmpegSetup}
        >
          <div
            className="modal ffmpeg-modal"
            onClick={(event) => event.stopPropagation()}
          >
            <div className="modal-header">
              <div>
                <h2>需要 FFmpeg</h2>
                <p className="hint">
                  Audio2Sub 使用 FFmpeg 提取音轨和读取媒体信息。
                </p>
              </div>
              <button
                className="icon-button"
                onClick={dismissFfmpegSetup}
                disabled={ffmpegInstalling}
              >
                ×
              </button>
            </div>

            {environment?.ffmpeg && environment?.ffprobe ? (
              <div className="ffmpeg-ready">
                <span className="status-dot ok" />
                <div>
                  <strong>FFmpeg 已就绪</strong>
                  <p>{environment.version || "版本未知"}</p>
                  {environment.ffmpeg_path && (
                    <p className="ffmpeg-path">{environment.ffmpeg_path}</p>
                  )}
                </div>
              </div>
            ) : (
              <>
                <p className="hint">
                  {environment?.install_supported
                    ? `可以自动下载并安装到本应用目录，不需要管理员权限。下载约 ${formatBytes(
                        environment.install_size,
                      )}。`
                    : "当前平台暂不支持自动安装，请按下方说明手动安装。"}
                </p>
                {ffmpegInstalling && (
                  <div className="install-progress">
                    <div className="progress-bar">
                      <div
                        style={{ width: `${ffmpegProgress?.percent ?? 0}%` }}
                      />
                    </div>
                    <div className="progress-text">
                      {ffmpegProgress
                        ? `${ffmpegProgress.percent}% · ${ffmpegProgress.message}`
                        : "正在准备下载..."}
                    </div>
                  </div>
                )}
                {ffmpegError && <div className="warning">{ffmpegError}</div>}
                <div className="form-actions">
                  {environment?.install_supported && (
                    <button
                      className="btn"
                      onClick={installFfmpeg}
                      disabled={ffmpegInstalling}
                    >
                      {ffmpegInstalling ? "安装中..." : "一键安装 FFmpeg"}
                    </button>
                  )}
                  {ffmpegInstalling && (
                    <button className="btn secondary" onClick={cancelFfmpegInstall}>
                      取消
                    </button>
                  )}
                  <button className="btn secondary" onClick={chooseFfmpegPath}>
                    选择已安装的 ffmpeg
                  </button>
                  <button
                    className="btn secondary"
                    onClick={() =>
                      refreshEnvironment().catch((error) =>
                        setFfmpegError(String(error)),
                      )
                    }
                  >
                    重新检测
                  </button>
                </div>
              </>
            )}

            <div className="manual-install">
              <h3>手动安装</h3>
              <p>
                Windows：<code>winget install ffmpeg</code>
              </p>
              <p>
                macOS：<code>brew install ffmpeg</code>
              </p>
              <p>
                Linux：<code>sudo apt install ffmpeg</code>
              </p>
              <p className="hint">
                Windows 也可从{" "}
                <span className="link-text">https://www.gyan.dev/ffmpeg/builds/</span>{" "}
                下载，解压后将 ffmpeg 所在目录加入 PATH。
              </p>
              <p className="hint">
                自动安装使用第三方静态构建，FFmpeg 按其自身许可证分发。
              </p>
            </div>
          </div>
        </div>
      )}

      {aboutOpen && (
        <div className="modal-backdrop" onClick={() => setAboutOpen(false)}>
          <div
            className="modal about-modal"
            onClick={(event) => event.stopPropagation()}
          >
            <div className="modal-header">
              <div>
                <h2>关于 Audio2Sub</h2>
                <p className="hint">本地音频 / 视频转字幕工具</p>
              </div>
              <button className="icon-button" onClick={() => setAboutOpen(false)}>
                ×
              </button>
            </div>

            <div className="author-card">
              <div className="author-mark">龙</div>
              <div>
                <h3>程序员龙龙</h3>
                <p>独立开发者 · Gitee lonffrey1</p>
              </div>
            </div>

            <div className="contact-list">
              <ContactRow
                label="邮箱"
                value="lonffrey@163.com"
                onCopy={() => copyText("lonffrey@163.com", "邮箱")}
              />
              <ContactRow
                label="小红书"
                value="indieDevLong（程序员龙龙）"
                onCopy={() => copyText("indieDevLong", "小红书号")}
              />
              <ContactRow
                label="抖音"
                value="2098958109（程序员龙龙）"
                onCopy={() => copyText("2098958109", "抖音号")}
              />
              <ContactRow
                label="Gitee"
                value="gitee.com/lonffrey1"
                onCopy={() => copyText("https://gitee.com/lonffrey1", "Gitee 地址")}
              />
            </div>

            <p className="about-note">
              项目反馈、功能建议、合作交流欢迎通过以上方式联系。如果 Audio2Sub
              对你有帮助，欢迎关注、分享或点一个 Star。
            </p>
          </div>
        </div>
      )}

      {message && <div className={`toast ${message.type}`}>{message.text}</div>}
    </div>
  );
}
