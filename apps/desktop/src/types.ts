export type EndpointKind = "openai" | "groq" | "siliconflow";

export type EndpointView = {
  id: string;
  name: string;
  kind: EndpointKind;
  base_url: string;
  model: string;
  has_api_key: boolean;
};

export type SettingsView = {
  endpoints: EndpointView[];
};

export type EnvironmentStatus = {
  ffmpeg: boolean;
  ffprobe: boolean;
  source: "env" | "custom" | "managed" | "system" | "missing" | string;
  ffmpeg_path: string | null;
  ffprobe_path: string | null;
  version: string | null;
  install_supported: boolean;
  install_size: number;
  license_url: string;
  source_url: string;
};

export type InstallProgress = {
  percent: number;
  stage: string;
  message: string;
  downloaded: number;
  total: number;
};

export type EndpointDraft = {
  id?: string;
  name: string;
  kind: EndpointKind;
  base_url: string;
  model: string;
  api_key: string;
};

export type Segment = {
  start_ms: number;
  end_ms: number;
  text: string;
};

export type TranscriptionResult = {
  project_id: string;
  filename: string;
  engine: string;
  duration_ms: number;
  segments: Segment[];
  created_at: string;
};

export type ProjectSummary = {
  id: string;
  filename: string;
  engine: string;
  segment_count: number;
  duration_ms: number;
  created_at: string;
};

export type ProgressPayload = {
  task_id: string;
  stage: string;
  percent: number;
  message: string;
};

export type FinishedPayload = {
  task_id: string;
  result: TranscriptionResult | null;
  error: string | null;
};

export type SubtitleFormat = "srt" | "vtt" | "txt";

export type Message = {
  type: "success" | "error" | "info";
  text: string;
};
