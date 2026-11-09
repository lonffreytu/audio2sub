use audio2sub_core::{AppSettings, Ffmpeg};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

pub const DOWNLOAD_BASE: &str =
    "https://github.com/descriptinc/ffmpeg-ffprobe-static/releases/download/b6.1.2-rc.1";
pub const FALLBACK_BASE: &str =
    "https://gh-proxy.com/https://github.com/descriptinc/ffmpeg-ffprobe-static/releases/download/b6.1.2-rc.1";
pub const LICENSE_URL: &str =
    "https://github.com/descriptinc/ffmpeg-ffprobe-static/blob/b6.1.2-rc.1/LICENSE";
pub const SOURCE_URL: &str = "https://ffmpeg.org/download.html";
const DOWNLOAD_ATTEMPTS: u32 = 3;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ToolAsset {
    pub name: &'static str,
    pub sha256: &'static str,
    pub size: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct PlatformAssets {
    pub ffmpeg: ToolAsset,
    pub ffprobe: ToolAsset,
}

#[derive(Debug, Clone)]
pub struct ResolvedTools {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub source: &'static str,
}

impl ResolvedTools {
    pub fn to_ffmpeg(&self) -> Ffmpeg {
        Ffmpeg::new(self.ffmpeg.clone(), self.ffprobe.clone())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolStatus {
    pub ffmpeg: bool,
    pub ffprobe: bool,
    pub source: String,
    pub ffmpeg_path: Option<String>,
    pub ffprobe_path: Option<String>,
    pub version: Option<String>,
    pub install_supported: bool,
    pub install_size: u64,
    pub license_url: String,
    pub source_url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallProgress {
    pub percent: u8,
    pub stage: String,
    pub message: String,
    pub downloaded: u64,
    pub total: u64,
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
pub fn platform_assets() -> Option<PlatformAssets> {
    Some(PlatformAssets {
        ffmpeg: ToolAsset {
            name: "ffmpeg-win32-x64",
            sha256: "4b3a7a7c41a4064a8738536da6a304371c1fdc567c599ccb768bebd49ccdb9fb",
            size: 126_518_784,
        },
        ffprobe: ToolAsset {
            name: "ffprobe-win32-x64",
            sha256: "c75d3ea9d6ab334ee2e90f15232af80aeac24d8a3185cd6ba19680acb8f43732",
            size: 126_382_080,
        },
    })
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub fn platform_assets() -> Option<PlatformAssets> {
    Some(PlatformAssets {
        ffmpeg: ToolAsset {
            name: "ffmpeg-darwin-arm64",
            sha256: "9f865039102a1139c7057d7f21ddaacd106d602fa3af1f99b70f43d520439b8c",
            size: 47_078_120,
        },
        ffprobe: ToolAsset {
            name: "ffprobe-darwin-arm64",
            sha256: "05a26b32c32115785d48b01601e104712bbc6c2b1d363b9cf44c42232684e25e",
            size: 47_037_128,
        },
    })
}

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
pub fn platform_assets() -> Option<PlatformAssets> {
    Some(PlatformAssets {
        ffmpeg: ToolAsset {
            name: "ffmpeg-darwin-x64",
            sha256: "4a4a968b98859588e98500ae25973d80a5ca5eed0724222b9f76360dcb72a001",
            size: 75_991_688,
        },
        ffprobe: ToolAsset {
            name: "ffprobe-darwin-x64",
            sha256: "ce5414269f0efa1e88b5e23b57f801d5b9a40be554716544936e0332b4601a62",
            size: 75_917_176,
        },
    })
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub fn platform_assets() -> Option<PlatformAssets> {
    Some(PlatformAssets {
        ffmpeg: ToolAsset {
            name: "ffmpeg-linux-x64",
            sha256: "d7791526ff9447dbdf88b78c43a2b98975862c8b18980b56b0f11e5bf1eafc08",
            size: 128_513_072,
        },
        ffprobe: ToolAsset {
            name: "ffprobe-linux-x64",
            sha256: "33e4d1be0482dce5355a04c62ccbda3d3f9f91fe4aa40cdb3585493278d91a1d",
            size: 128_374_768,
        },
    })
}

#[cfg(not(any(
    all(target_os = "windows", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "x86_64"),
)))]
pub fn platform_assets() -> Option<PlatformAssets> {
    None
}

pub fn managed_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("tools").join("ffmpeg")
}

pub fn tool_file_name(base: &str) -> String {
    if cfg!(windows) {
        format!("{base}.exe")
    } else {
        base.to_string()
    }
}

pub async fn tool_available(path: &Path) -> bool {
    tokio::process::Command::new(path)
        .arg("-version")
        .output()
        .await
        .map(|output| output.status.success())
        .unwrap_or(false)
}

async fn tool_version(path: &Path) -> Option<String> {
    let output = tokio::process::Command::new(path)
        .arg("-version")
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
}

pub async fn resolve(app_data_dir: &Path, settings: &AppSettings) -> ResolvedTools {
    let env_ffmpeg = std::env::var_os("AUDIO2SUB_FFMPEG").map(PathBuf::from);
    let env_ffprobe = std::env::var_os("AUDIO2SUB_FFPROBE").map(PathBuf::from);
    if let (Some(ffmpeg), Some(ffprobe)) = (env_ffmpeg, env_ffprobe) {
        if tool_available(&ffmpeg).await && tool_available(&ffprobe).await {
            return ResolvedTools {
                ffmpeg,
                ffprobe,
                source: "env",
            };
        }
    }

    if let (Some(ffmpeg), Some(ffprobe)) = (&settings.ffmpeg_path, &settings.ffprobe_path) {
        let ffmpeg = PathBuf::from(ffmpeg);
        let ffprobe = PathBuf::from(ffprobe);
        if tool_available(&ffmpeg).await && tool_available(&ffprobe).await {
            return ResolvedTools {
                ffmpeg,
                ffprobe,
                source: "custom",
            };
        }
    }

    let managed = managed_dir(app_data_dir);
    let managed_ffmpeg = managed.join(tool_file_name("ffmpeg"));
    let managed_ffprobe = managed.join(tool_file_name("ffprobe"));
    if tool_available(&managed_ffmpeg).await && tool_available(&managed_ffprobe).await {
        return ResolvedTools {
            ffmpeg: managed_ffmpeg,
            ffprobe: managed_ffprobe,
            source: "managed",
        };
    }

    let system_ffmpeg = PathBuf::from("ffmpeg");
    let system_ffprobe = PathBuf::from("ffprobe");
    if tool_available(&system_ffmpeg).await && tool_available(&system_ffprobe).await {
        return ResolvedTools {
            ffmpeg: system_ffmpeg,
            ffprobe: system_ffprobe,
            source: "system",
        };
    }

    ResolvedTools {
        ffmpeg: system_ffmpeg,
        ffprobe: system_ffprobe,
        source: "missing",
    }
}

pub async fn status(app_data_dir: &Path, settings: &AppSettings) -> ToolStatus {
    let resolved = resolve(app_data_dir, settings).await;
    let ffmpeg = tool_available(&resolved.ffmpeg).await;
    let ffprobe = tool_available(&resolved.ffprobe).await;
    let version = if ffmpeg {
        tool_version(&resolved.ffmpeg).await
    } else {
        None
    };
    let assets = platform_assets();
    ToolStatus {
        ffmpeg,
        ffprobe,
        source: resolved.source.to_string(),
        ffmpeg_path: ffmpeg.then(|| resolved.ffmpeg.display().to_string()),
        ffprobe_path: ffprobe.then(|| resolved.ffprobe.display().to_string()),
        version,
        install_supported: assets.is_some(),
        install_size: assets
            .map(|assets| assets.ffmpeg.size + assets.ffprobe.size)
            .unwrap_or(0),
        license_url: LICENSE_URL.to_string(),
        source_url: SOURCE_URL.to_string(),
    }
}

pub async fn validate_custom(ffmpeg_path: &Path) -> Result<(PathBuf, PathBuf), String> {
    if !tool_available(ffmpeg_path).await {
        return Err("无法运行该 ffmpeg，请选择正确的可执行文件".to_string());
    }
    let parent = ffmpeg_path
        .parent()
        .ok_or_else(|| "无法确定 ffmpeg 所在目录".to_string())?;
    let ffprobe = parent.join(tool_file_name("ffprobe"));
    if !tool_available(&ffprobe).await {
        return Err(format!(
            "同目录下没有找到可用的 ffprobe: {}",
            ffprobe.display()
        ));
    }
    Ok((ffmpeg_path.to_path_buf(), ffprobe))
}

fn download_bases() -> Vec<String> {
    if let Ok(value) = std::env::var("AUDIO2SUB_FFMPEG_MIRROR") {
        let value = value.trim_end_matches('/');
        if !value.is_empty() {
            return vec![value.to_string()];
        }
    }
    vec![DOWNLOAD_BASE.to_string(), FALLBACK_BASE.to_string()]
}

fn download_percent(completed: u64, total: u64) -> u8 {
    ((completed as u128 * 90) / total.max(1) as u128).min(90) as u8
}

pub async fn install(
    app_data_dir: &Path,
    cancel: CancellationToken,
    progress: &(dyn Fn(InstallProgress) + Send + Sync),
) -> Result<ResolvedTools, String> {
    let assets = platform_assets()
        .ok_or_else(|| "当前平台暂不支持自动安装 FFmpeg，请使用手动安装方式".to_string())?;
    let dir = managed_dir(app_data_dir);
    let temp = dir.join("download");
    if temp.exists() {
        std::fs::remove_dir_all(&temp).map_err(|error| format!("清理临时目录失败: {error}"))?;
    }
    std::fs::create_dir_all(&temp).map_err(|error| format!("创建临时目录失败: {error}"))?;
    std::fs::create_dir_all(&dir).map_err(|error| format!("创建工具目录失败: {error}"))?;

    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(1800))
        .user_agent("Audio2Sub/0.2.0")
        .build()
        .map_err(|error| format!("初始化下载器失败: {error}"))?;

    let total = assets.ffmpeg.size + assets.ffprobe.size;
    let mut completed = 0u64;
    let ffmpeg_temp = temp.join(assets.ffmpeg.name);
    let ffprobe_temp = temp.join(assets.ffprobe.name);

    download_asset(
        &client,
        assets.ffmpeg,
        &ffmpeg_temp,
        &cancel,
        progress,
        &mut completed,
        total,
    )
    .await?;
    download_asset(
        &client,
        assets.ffprobe,
        &ffprobe_temp,
        &cancel,
        progress,
        &mut completed,
        total,
    )
    .await?;

    progress(InstallProgress {
        percent: 92,
        stage: "verify".to_string(),
        message: "校验安装文件...".to_string(),
        downloaded: total,
        total,
    });

    let ffmpeg_final = dir.join(tool_file_name("ffmpeg"));
    let ffprobe_final = dir.join(tool_file_name("ffprobe"));
    if ffmpeg_final.exists() {
        std::fs::remove_file(&ffmpeg_final)
            .map_err(|error| format!("替换 ffmpeg 失败: {error}"))?;
    }
    if ffprobe_final.exists() {
        std::fs::remove_file(&ffprobe_final)
            .map_err(|error| format!("替换 ffprobe 失败: {error}"))?;
    }
    std::fs::rename(&ffmpeg_temp, &ffmpeg_final)
        .map_err(|error| format!("安装 ffmpeg 失败: {error}"))?;
    std::fs::rename(&ffprobe_temp, &ffprobe_final)
        .map_err(|error| format!("安装 ffprobe 失败: {error}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(&ffmpeg_final, mode.clone())
            .map_err(|error| format!("设置 ffmpeg 权限失败: {error}"))?;
        std::fs::set_permissions(&ffprobe_final, mode)
            .map_err(|error| format!("设置 ffprobe 权限失败: {error}"))?;
    }

    let _ = std::fs::remove_dir_all(&temp);

    if !tool_available(&ffmpeg_final).await || !tool_available(&ffprobe_final).await {
        return Err("安装完成，但 FFmpeg 无法启动".to_string());
    }

    progress(InstallProgress {
        percent: 100,
        stage: "done".to_string(),
        message: "FFmpeg 安装完成".to_string(),
        downloaded: total,
        total,
    });

    Ok(ResolvedTools {
        ffmpeg: ffmpeg_final,
        ffprobe: ffprobe_final,
        source: "managed",
    })
}

struct DownloadRequest<'a> {
    client: &'a reqwest::Client,
    url: &'a str,
    asset: ToolAsset,
    destination: &'a Path,
    cancel: &'a CancellationToken,
    progress: &'a (dyn Fn(InstallProgress) + Send + Sync),
    completed: &'a mut u64,
    total: u64,
}

async fn download_asset(
    client: &reqwest::Client,
    asset: ToolAsset,
    destination: &Path,
    cancel: &CancellationToken,
    progress: &(dyn Fn(InstallProgress) + Send + Sync),
    completed: &mut u64,
    total: u64,
) -> Result<(), String> {
    let bases = download_bases();
    let mut last_error = String::new();
    for (base_index, base) in bases.iter().enumerate() {
        let url = format!("{base}/{}", asset.name);
        for attempt in 1..=DOWNLOAD_ATTEMPTS {
            if cancel.is_cancelled() {
                return Err("安装已取消".to_string());
            }
            let request = DownloadRequest {
                client,
                url: &url,
                asset,
                destination,
                cancel,
                progress,
                completed,
                total,
            };
            match download_asset_once(request).await {
                Ok(()) => return Ok(()),
                Err(error) => {
                    last_error = error;
                    if attempt < DOWNLOAD_ATTEMPTS {
                        let downloaded = *completed;
                        progress(InstallProgress {
                            percent: download_percent(downloaded, total),
                            stage: "retry".to_string(),
                            message: format!(
                                "{} 下载失败，正在重试（{attempt}/{DOWNLOAD_ATTEMPTS}）...",
                                asset.name
                            ),
                            downloaded,
                            total,
                        });
                        tokio::select! {
                            _ = tokio::time::sleep(Duration::from_secs(u64::from(attempt) * 2)) => {}
                            _ = cancel.cancelled() => return Err("安装已取消".to_string()),
                        }
                    }
                }
            }
        }
        if base_index + 1 < bases.len() {
            let downloaded = *completed;
            progress(InstallProgress {
                percent: download_percent(downloaded, total),
                stage: "mirror".to_string(),
                message: "官方源不可用，切换备用下载源...".to_string(),
                downloaded,
                total,
            });
        }
    }
    Err(last_error)
}

async fn download_asset_once(request: DownloadRequest<'_>) -> Result<(), String> {
    let DownloadRequest {
        client,
        url,
        asset,
        destination,
        cancel,
        progress,
        completed,
        total,
    } = request;

    let _ = tokio::fs::remove_file(destination).await;
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("下载 {} 失败: {error}", asset.name))?;
    if !response.status().is_success() {
        return Err(format!(
            "下载 {} 失败: HTTP {}",
            asset.name,
            response.status().as_u16()
        ));
    }
    if let Some(length) = response.content_length() {
        if length != asset.size {
            return Err(format!(
                "{} 大小不符: 期望 {}，实际 {}",
                asset.name, asset.size, length
            ));
        }
    }

    let mut file = tokio::fs::File::create(destination)
        .await
        .map_err(|error| format!("创建临时文件失败: {error}"))?;
    let mut hasher = Sha256::new();
    let mut bytes_done = 0u64;
    let mut last_percent = u8::MAX;

    loop {
        let chunk = tokio::select! {
            result = response.chunk() => result.map_err(|error| format!("下载中断: {error}"))?,
            _ = cancel.cancelled() => {
                let _ = tokio::fs::remove_file(destination).await;
                return Err("安装已取消".to_string());
            }
        };
        let Some(chunk) = chunk else {
            break;
        };
        file.write_all(&chunk)
            .await
            .map_err(|error| format!("写入文件失败: {error}"))?;
        hasher.update(&chunk);
        let length = chunk.len() as u64;
        bytes_done += length;
        let downloaded = completed.saturating_add(bytes_done);
        let percent = download_percent(downloaded, total);
        if percent != last_percent {
            progress(InstallProgress {
                percent,
                stage: "download".to_string(),
                message: format!("下载 {}...", asset.name),
                downloaded,
                total,
            });
            last_percent = percent;
        }
    }

    file.flush()
        .await
        .map_err(|error| format!("写入文件失败: {error}"))?;
    drop(file);

    if bytes_done != asset.size {
        let _ = tokio::fs::remove_file(destination).await;
        return Err(format!(
            "{} 下载不完整: 期望 {}，实际 {}",
            asset.name, asset.size, bytes_done
        ));
    }

    let hash = format!("{:x}", hasher.finalize());
    if hash != asset.sha256 {
        let _ = tokio::fs::remove_file(destination).await;
        return Err(format!("{} SHA256 校验失败", asset.name));
    }

    *completed = completed.saturating_add(bytes_done);
    let downloaded = *completed;
    progress(InstallProgress {
        percent: download_percent(downloaded, total),
        stage: "verify".to_string(),
        message: format!("{} 校验通过", asset.name),
        downloaded,
        total,
    });

    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    struct EmptyPathGuard {
        old: Option<std::ffi::OsString>,
        dir: PathBuf,
    }

    impl EmptyPathGuard {
        fn new() -> Self {
            let old = std::env::var_os("PATH");
            let dir =
                std::env::temp_dir().join(format!("audio2sub-empty-path-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("create empty PATH dir");
            std::env::set_var("PATH", &dir);
            Self { old, dir }
        }
    }

    impl Drop for EmptyPathGuard {
        fn drop(&mut self) {
            if let Some(old) = self.old.take() {
                std::env::set_var("PATH", old);
            } else {
                std::env::remove_var("PATH");
            }
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn platform_manifest_is_well_formed() {
        if let Some(assets) = platform_assets() {
            assert_eq!(assets.ffmpeg.sha256.len(), 64);
            assert_eq!(assets.ffprobe.sha256.len(), 64);
            assert!(assets.ffmpeg.size > 0);
            assert!(assets.ffprobe.size > 0);
        }
    }

    #[test]
    fn managed_directory_is_under_app_data() {
        let path = managed_dir(Path::new("app-data"));
        assert_eq!(path, Path::new("app-data").join("tools").join("ffmpeg"));
    }

    #[test]
    fn tool_file_name_matches_platform() {
        let name = tool_file_name("ffmpeg");
        if cfg!(windows) {
            assert_eq!(name, "ffmpeg.exe");
        } else {
            assert_eq!(name, "ffmpeg");
        }
    }

    #[tokio::test]
    #[ignore = "manual smoke test; requires AUDIO2SUB_TEST_FFMPEG_INSTALL=1"]
    async fn installs_ffmpeg_smoke() {
        if std::env::var("AUDIO2SUB_TEST_FFMPEG_INSTALL")
            .ok()
            .as_deref()
            != Some("1")
        {
            return;
        }
        let path_guard = EmptyPathGuard::new();
        std::env::remove_var("AUDIO2SUB_FFMPEG");
        std::env::remove_var("AUDIO2SUB_FFPROBE");

        let dir =
            std::env::temp_dir().join(format!("audio2sub-installer-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let settings = AppSettings::default();

        let before = status(&dir, &settings).await;
        assert!(!before.ffmpeg);
        assert!(!before.ffprobe);
        assert_eq!(before.source, "missing");
        assert!(before.ffmpeg_path.is_none());
        assert!(before.ffprobe_path.is_none());
        assert_eq!(before.install_supported, platform_assets().is_some());

        let resolved = install(&dir, CancellationToken::new(), &|_| {})
            .await
            .expect("install from mirror");
        assert_eq!(resolved.source, "managed");
        assert!(tool_available(&resolved.ffmpeg).await);
        assert!(tool_available(&resolved.ffprobe).await);

        let after = status(&dir, &settings).await;
        assert!(after.ffmpeg);
        assert!(after.ffprobe);
        assert_eq!(after.source, "managed");
        assert!(after.version.is_some());

        let _ = std::fs::remove_dir_all(&dir);
        drop(path_guard);
    }
}
