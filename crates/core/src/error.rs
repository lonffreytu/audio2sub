#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("网络错误: {0}")]
    Http(#[from] reqwest::Error),
    #[error("JSON 错误: {0}")]
    Json(#[from] serde_json::Error),
    #[error("数据库错误: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("FFmpeg 错误: {0}")]
    Ffmpeg(String),
    #[error("识别错误: {0}")]
    Asr(String),
    #[error("参数错误: {0}")]
    InvalidInput(String),
    #[error("任务已取消")]
    Cancelled,
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;
