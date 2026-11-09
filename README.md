# Audio2Sub

基于 Rust + Tauri 2 的本地音频 / 视频转字幕桌面工具。

Audio2Sub 把本地媒体文件转换成 SRT、VTT 或 TXT 字幕。桌面端提供拖拽导入、识别进度、任务取消、字幕逐条编辑、时间码调整、历史记录和识别接口管理。命令行版本共用同一套 Rust 核心，适合批处理和自动化。

Windows 已完成本地运行验证；macOS / Linux 已纳入 CI 编译检查，运行时仍建议在对应系统实机验证。

快速导航：

- 想了解功能：看「功能」
- 想直接使用：看「使用教程」和「下载安装」
- 想用命令行：看「命令行用法」
- 想参与开发或自己打包：看「开发与构建」

## 功能

- 音频 / 视频拖拽导入
- 必剪（B 站）免费识别引擎，无需账号和 API Key
- OpenAI 兼容接口：OpenAI、Groq、硅基流动，以及自定义 Base URL / 模型
- FFmpeg 自动提取音轨、探测时长
- 实时进度、阶段日志、任务取消
- 字幕表格编辑：文本、开始时间、结束时间、新增、插入、删除
- 导出 SRT、VTT、TXT
- SQLite 历史记录，支持重新打开和继续编辑
- API Key 存入系统凭据管理器，不写入项目文件
- 深色 / 浅色主题
- 启动时检测 ffmpeg / ffprobe，缺失时给出提示

## 界面预览

![Audio2Sub 桌面端主界面](docs/screenshots/main.png)

桌面端主界面：文件导入、引擎选择、字幕编辑与历史记录。

## 使用教程

如果还没安装，先看后面的「下载安装」，安装完成后再按本节操作。

### 1. 导入文件

把音频或视频直接拖进左侧上传区，或者点击上传区选择文件。

支持常见格式：

- 音频：MP3、WAV、M4A、FLAC、OGG、OPUS、AAC
- 视频：MP4、MKV、MOV、WebM、AVI、FLV、WMV、TS、M4V

视频会自动提取音轨。

### 2. 选择识别引擎和语言

在顶部选择引擎：

- 必剪（B 站，免费）：不需要配置，适合中文、英文
- OpenAI / Groq / 硅基流动 / 自定义：需要先在「设置」中配置 API Key

语言默认自动检测，也可以手动指定 `zh`、`en`、`ja`、`ko` 等。

### 3. 开始识别

点击「开始识别」。界面会显示进度和当前阶段，可以随时取消。

### 4. 编辑字幕

识别完成后，右侧会显示字幕表格：

- 修改字幕文本
- 修改开始 / 结束时间
- 新增字幕
- 在当前行后插入
- 删除字幕
- 保存修改到历史记录

### 5. 导出字幕

点击「导出 SRT」「VTT」或「TXT」，选择保存位置即可。

### 6. 历史记录

左侧历史记录会保留最近的识别项目。点击即可重新打开，继续编辑或导出。

## 下载安装

### 下载地址

- GitHub Releases（国际）：<https://github.com/lonffreytu/audio2sub/releases>
- Gitee Releases（中国国内）：<https://gitee.com/lonffrey1/Audio2Sub/releases>

国内访问 GitHub 较慢时，优先使用 Gitee。如果页面暂时没有安装包，说明该版本还在构建中。

### 先安装 FFmpeg

Audio2Sub 本身不内置 FFmpeg，需要系统已经安装 `ffmpeg` 和 `ffprobe`，并且能在终端中直接调用。

Windows：

```powershell
winget install ffmpeg
```

安装后重新打开终端，确认：

```powershell
ffmpeg -version
ffprobe -version
```

macOS：

```bash
brew install ffmpeg
```

Linux：

```bash
sudo apt install ffmpeg
```

如果应用启动后提示未检测到 FFmpeg，安装完成后重启 Audio2Sub 即可。

### Windows 安装

下载以下任意一个文件：

- `Audio2Sub_0.1.0_x64-setup.exe`（NSIS 安装包，推荐）
- `Audio2Sub_0.1.0_x64_en-US.msi`（MSI 安装包）

双击安装即可。当前版本没有代码签名，Windows SmartScreen 可能提示风险，点击「更多信息」然后选择「仍要运行」。

系统要求：Windows 10 / 11，WebView2 Runtime。大部分 Windows 10/11 已经自带 WebView2；如果启动失败，安装微软官方的 WebView2 Runtime。

### macOS 安装

根据芯片选择：

- Apple Silicon（M1 / M2 / M3 / M4）：下载 `Audio2Sub_0.1.0_aarch64.dmg`
- Intel Mac：下载 `Audio2Sub_0.1.0_x64.dmg`

打开 DMG，把 Audio2Sub 拖进「应用程序」。

当前版本没有签名和公证，首次打开可能被 Gatekeeper 拦截。可以右键点击 Audio2Sub，选择「打开」，或者到「系统设置 -> 隐私与安全性」中点击「仍要打开」。

如果不确定芯片类型，打开「苹果菜单 -> 关于本机」，查看「芯片」或「处理器」。

### Linux 安装

Debian / Ubuntu 下载 `.deb`：

```bash
sudo apt install ./Audio2Sub_0.1.0_amd64.deb
```

其他发行版下载 `.AppImage`：

```bash
chmod +x Audio2Sub_0.1.0_amd64.AppImage
./Audio2Sub_0.1.0_amd64.AppImage
```

如果 AppImage 提示 FUSE 相关错误，可以安装 `libfuse2`，或者使用：

```bash
./Audio2Sub_0.1.0_amd64.AppImage --appimage-extract-and-run
```

### 首次启动

1. 打开 Audio2Sub。
2. 如果提示未检测到 `ffmpeg` / `ffprobe`，先安装 FFmpeg 并重启应用。
3. 默认引擎是「必剪（B 站，免费）」，不需要账号和 API Key。
4. 如果必剪不可用，可以在「设置」中添加 OpenAI、Groq、硅基流动或其他 OpenAI 兼容接口。

## 配置 OpenAI 兼容接口

如果必剪不可用，或者需要更好的时间戳和断句，可以配置 Whisper 类接口。

1. 点击顶部「设置」。
2. 点击「添加接口」。
3. 选择类型：

| 类型 | Base URL | 默认模型 |
|---|---|---|
| OpenAI | `https://api.openai.com/v1` | `whisper-1` |
| Groq | `https://api.groq.com/openai/v1` | `whisper-large-v3` |
| 硅基流动 | `https://api.siliconflow.cn/v1` | `FunAudioLLM/SenseVoiceSmall` |
| 自定义 | 兼容 `/audio/transcriptions` 的地址 | 自行填写 |

4. 填入 API Key。
5. 点击「测试连接」。
6. 保存后，在顶部识别引擎下拉框中选择该接口。

API Key 会保存到系统凭据管理器：

- Windows：Credential Manager
- macOS：Keychain
- Linux：Secret Service（GNOME Keyring / KWallet）

## 识别引擎

| 引擎 | 认证 | 说明 |
|---|---|---|
| 必剪（B 站） | 无需账号、无需 API Key | 调用必剪网页接口，首次会匿名访问 B 站获取 WAF Cookie；非公开接口，稳定性无保证 |
| OpenAI | Bearer API Key | 官方 Whisper 接口 |
| Groq | Bearer API Key | OpenAI 兼容接口，免费额度较大，速度快 |
| 硅基流动 | Bearer API Key | OpenAI 兼容接口，支持 SenseVoice 等模型 |
| 自定义 | Bearer API Key | 任何兼容 `/audio/transcriptions` 的服务 |

必剪接口不需要登录 Token，但会使用匿名 Cookie 和单次任务的临时上传 / 任务 ID。该接口属于非官方接口，可能随时改版、限流或失效，请自行评估使用风险，并遵守 B 站相关服务条款。

## 常见问题

### 必剪识别失败，或者一直返回 412

必剪是 B 站非公开接口，可能临时不可用、限流或改版。可以换用 Groq、硅基流动、OpenAI 或自定义接口。

### 必剪没有逐句时间戳，断句不准

必剪返回的是纯文本，时间轴由程序按字数估算。需要更精确的逐句时间戳时，请使用 Whisper 类接口（Groq / OpenAI / 硅基流动）。

### 提示未检测到 ffmpeg / ffprobe

说明系统 PATH 中没有 FFmpeg。安装后重新打开终端和应用：

```powershell
winget install ffmpeg
```

```bash
brew install ffmpeg
```

```bash
sudo apt install ffmpeg
```

### Windows 提示 SmartScreen 风险

当前安装包未签名。点击「更多信息」->「仍要运行」。

### macOS 提示无法打开，或者已损坏

当前应用未签名和公证。右键点击应用选择「打开」，或到「系统设置 -> 隐私与安全性」点击「仍要打开」。

### API Key 保存在哪里

保存在系统凭据管理器，不会写进项目文件。删除接口时也会删除对应凭据。

### 能完全离线使用吗

暂时不能。必剪和 OpenAI 兼容接口都需要联网，音频会被发送到所选服务。后续计划加入本地 ASR 引擎。

### 长音频有什么限制

OpenAI 兼容接口通常有约 25MB 的文件限制。当前版本暂未自动切片；长音频可以先用必剪，或者手动切分后识别。

### 数据保存在哪里

桌面端默认数据目录：

```text
Windows: %APPDATA%\cc.audio2sub.desktop
macOS:   ~/Library/Application Support/cc.audio2sub.desktop
Linux:   ~/.local/share/cc.audio2sub.desktop
```

其中包含 `audio2sub.db` 和 `settings.json`。

## 已知限制

- 必剪接口为非公开接口，可能失效；建议配置其他引擎或后续使用本地模型兜底。
- OpenAI 兼容接口暂未对超过约 25MB 的长音频做自动切片。
- 暂未提供波形图、视频预览和时间轴拖拽。
- FFmpeg 未打包进安装包，需要用户自行安装并配置 PATH。
- macOS / Linux 已通过 CI 编译检查，但尚未完成实机运行验证。
- 暂未提供安装包签名和自动更新。

## 数据与隐私

- 桌面端没有登录、2FA 或云同步，默认单人本地使用。
- 识别历史保存在本地 SQLite 数据库。
- API Key 保存在系统凭据管理器，不会写入仓库文件。
- 程序不包含遥测。
- 识别本身可能使用云端引擎：音频 / 视频会被发送到所选的必剪或 OpenAI 兼容服务。若需要完全离线，请等待后续本地推理引擎支持。

## 命令行用法

CLI 需要从源码构建，安装包里只包含桌面程序。

必剪引擎：

```powershell
audio2sub transcribe "D:\media\meeting.mp4" --engine bcut --format srt
```

OpenAI 兼容接口：

```powershell
audio2sub transcribe "D:\media\audio.mp3" `
  --engine openai `
  --base-url "https://api.groq.com/openai/v1" `
  --api-key "你的 API Key" `
  --model "whisper-large-v3" `
  --language zh `
  --format srt
```

硅基流动示例：

```powershell
audio2sub transcribe "D:\media\audio.mp3" `
  --engine openai `
  --base-url "https://api.siliconflow.cn/v1" `
  --api-key "你的 API Key" `
  --model "FunAudioLLM/SenseVoiceSmall"
```

其他命令：

```powershell
audio2sub probe "D:\media\video.mp4"
audio2sub history --data-dir ".\data"
```

默认输出文件与输入文件同目录，扩展名由 `--format` 决定。CLI 默认数据目录为用户数据目录下的 `Audio2Sub`，可通过 `--data-dir` 或环境变量 `AUDIO2SUB_DATA_DIR` 指定。

### 支持的环境变量

| 变量 | 作用 |
|---|---|
| `AUDIO2SUB_API_KEY` | OpenAI 兼容接口 API Key |
| `OPENAI_API_KEY` | `AUDIO2SUB_API_KEY` 未设置时的回退 |
| `AUDIO2SUB_BASE_URL` | OpenAI 兼容接口 Base URL |
| `AUDIO2SUB_MODEL` | 模型名称 |
| `AUDIO2SUB_DATA_DIR` | CLI 数据目录 |
| `AUDIO2SUB_FFMPEG` | ffmpeg 可执行文件路径 |
| `AUDIO2SUB_FFPROBE` | ffprobe 可执行文件路径 |

## 开发与构建

### 技术栈

- 桌面框架：Tauri 2
- 前端：React 19 + TypeScript + Vite
- 核心逻辑：Rust
- 异步运行时：Tokio
- HTTP：reqwest
- 数据库：SQLite（rusqlite）
- 音视频处理：系统 FFmpeg / ffprobe
- API Key 存储：系统凭据管理器（keyring）

### 项目结构

```text
Audio2Sub/
├─ crates/
│  ├─ core/                 # 核心库：引擎、媒体、字幕、存储、任务流水线
│  └─ cli/                  # 命令行程序 audio2sub
├─ apps/
│  └─ desktop/
│     ├─ src/               # React 前端
│     └─ src-tauri/         # Tauri Rust 后端、窗口、命令、keyring
├─ docs/
│  └─ screenshots/
│     └─ main.png           # README 界面截图
├─ scripts/
│  ├─ test.ps1              # Windows 一键检查与测试
│  ├─ test.sh               # macOS / Linux 一键检查与测试
│  └─ publish-gitee.sh      # 同步 Release 到 Gitee
├─ .github/workflows/
│  ├─ ci.yml                # CI：Rust 测试、三平台桌面检查、前端构建
│  └─ release.yml           # 推送 v* 标签时构建安装包并发布 Release
├─ Cargo.toml               # Rust workspace（core + cli）
├─ Cargo.lock
├─ LICENSE
└─ README.md
```

桌面端是独立的 Tauri workspace，通过路径依赖使用 `crates/core`。根 workspace 不包含 `apps/desktop/src-tauri`，因此 `cargo test --workspace` 只测试核心库和 CLI，不会编译整个 Tauri / WebView 依赖树。

### 环境要求

通用要求：

- Rust stable
- Node.js 22+ 和 npm
- FFmpeg 与 ffprobe，且位于 `PATH` 中
- Tauri 2 构建依赖，参见 [Tauri prerequisites](https://tauri.app/start/prerequisites/)

Windows：

- Windows 10 / 11
- WebView2 Runtime
- MSVC Build Tools

macOS：

- Xcode Command Line Tools
- FFmpeg：`brew install ffmpeg`
- `.app` / `.dmg` 需要在 macOS 上构建；签名和公证需要 Apple 开发者账号

Linux：

- WebKitGTK 4.1、`libayatana-appindicator3-dev`、`librsvg2-dev`、`patchelf`
- keyring 编译依赖：`libdbus-1-dev`、`pkg-config`
- FFmpeg：`sudo apt install ffmpeg`
- API Key 存储依赖 Secret Service（GNOME Keyring / KWallet）；无桌面环境的服务器可能无法使用系统凭据管理器

如果使用自定义 FFmpeg 路径，可以设置：

```powershell
$env:AUDIO2SUB_FFMPEG = "D:\tools\ffmpeg\bin\ffmpeg.exe"
$env:AUDIO2SUB_FFPROBE = "D:\tools\ffmpeg\bin\ffprobe.exe"
```

```bash
export AUDIO2SUB_FFMPEG=/opt/ffmpeg/bin/ffmpeg
export AUDIO2SUB_FFPROBE=/opt/ffmpeg/bin/ffprobe
```

### 平台支持

| 平台 | 当前状态 | 典型产物 |
|---|---|---|
| Windows | 已本地验证 | MSI / NSIS |
| macOS | CI 编译检查，待实机验证 | `.app` / `.dmg` |
| Linux | CI 编译检查，待实机验证 | `.deb` / AppImage |

Tauri 不支持从 Windows 直接交叉编译 macOS / Linux 桌面安装包。请在各目标系统上构建，或使用 CI matrix。当前 CI 已配置 Windows、macOS、Ubuntu 三平台桌面编译检查。核心库和 CLI 是纯 Rust，可跨平台编译。

### 桌面端开发

```powershell
cd apps\desktop
npm install
npx tauri dev
```

### 构建桌面程序

```powershell
cd apps\desktop
npx tauri build
```

Debug 版本可直接生成可执行文件：

```powershell
npx tauri build --debug --no-bundle
```

产物位于：

```text
apps\desktop\src-tauri\target\debug\audio2sub-desktop.exe
```

### 构建 CLI

```powershell
cargo build --release -p audio2sub-cli
```

产物：

```text
target\release\audio2sub.exe
```

### 测试

Windows 一键检查：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\test.ps1
```

macOS / Linux 一键检查：

```bash
bash scripts/test.sh
```

两个脚本都会依次执行：

- Rust 格式检查
- Rust Clippy（`-D warnings`）
- Rust 单元测试、集成测试、CLI 端到端测试
- 前端 TypeScript 类型检查与 Vite 构建
- 桌面端格式检查、Clippy、编译检查

也可以单独运行：

```powershell
cargo test --workspace
```

当前测试覆盖：

- 字幕时间格式、SRT / VTT / TXT 输出
- 中文断句、英文断词、时间轴分配
- FFmpeg 探测与音频提取
- 必剪完整上传 / 轮询流程（WireMock，不请求真实服务）
- OpenAI 兼容接口解析、错误处理、纯文本回退
- SQLite 历史记录
- 任务取消与结果持久化
- CLI 端到端转写与历史查询

CI 配置见 `.github/workflows/ci.yml`。

### 发布安装包

#### GitHub Actions 自动构建

仓库包含 `.github/workflows/release.yml`。推送 `v*` 标签时，会自动在 Windows、macOS、Linux 上构建安装包：

```bash
git tag v0.1.0
git push origin v0.1.0
```

产物包括：

- Windows：`.msi` / `.exe`（NSIS）
- macOS：`.dmg` / `.app`
- Linux：`.deb` / `.AppImage`

构建产物会先作为 Actions artifacts 汇总，再由单独的 publish job 统一创建 Release，避免多平台并发时产生多个 draft。工作流默认创建 draft release，检查安装包无误后，在 GitHub 页面手动发布即可。

默认构建不包含代码签名。Windows 可能弹出 SmartScreen 提示，macOS 可能触发 Gatekeeper 拦截；正式发布建议配置 Windows 代码签名证书和 Apple Developer 签名 / 公证。

#### Gitee

Gitee 不支持 GitHub Actions。可选方案：

1. 使用 Gitee Go 构建 Linux 安装包，但它不适合同时产出 Windows / macOS 安装包。
2. 推荐：由 GitHub Actions 构建，再把产物同步到 Gitee Release。仓库提供 `scripts/publish-gitee.sh`：

```bash
export GITEE_TOKEN="你的 Gitee 私人令牌"
export GITEE_OWNER="lonffrey1"
export GITEE_REPO="Audio2Sub"
bash scripts/publish-gitee.sh v0.1.0 artifacts/*
```

脚本会创建（或复用）对应 tag 的 Gitee Release，并上传安装包。Gitee 私人令牌需要 `projects` 权限；发布前请确认 Gitee Release 的单文件大小限制。

同时需要把代码和标签推送到 Gitee：

```bash
git remote add gitee https://gitee.com/lonffrey1/Audio2Sub.git
git push gitee master --tags
```

如果不想使用脚本，也可以在 Gitee 网页端手动创建 Release 并上传安装包。Gitee 上的源码仓库可以镜像 GitHub，但构建产物和 Release 附件需要单独同步。

### 架构

```text
React UI (apps/desktop/src)
  -> Tauri commands (apps/desktop/src-tauri/src/lib.rs)
    -> TranscriptionService (crates/core/src/service.rs)
      -> Ffmpeg        crates/core/src/media.rs
      -> BcutEngine    crates/core/src/engine/bcut.rs
      -> OpenAiEngine  crates/core/src/engine/openai.rs
      -> HistoryStore  crates/core/src/storage.rs

CLI (crates/cli/src/main.rs)
  -> 同一个 TranscriptionService
```

更换引擎只需要实现 `AsrEngine` trait，不需要改桌面端和 CLI。

## 与上游项目的关系

本项目在功能和接口流程上参考了：

- [yyzq-cf/ywsj-audio-to-subtitle](https://github.com/yyzq-cf/ywsj-audio-to-subtitle)

Audio2Sub 是独立的 Rust 实现，没有包含上游项目的 Python 代码、HTML / CSS / JavaScript、图片或其他资源。界面、图标、前端组件和 Rust 代码均为重新实现。上游仓库 README 声称 MIT，但仓库中没有 LICENSE 文件；如需复用上游代码或资源，请先与原作者确认授权。

## Roadmap

- 本地 ASR 引擎（whisper-rs / sherpa-rs），支持离线识别
- 长音频自动切片与时间戳偏移
- 波形图、视频预览、时间轴拖拽
- 批量队列与并发控制
- 安装包签名与自动更新
- macOS / Linux 验证与打包

## 贡献

欢迎提交 Issue 和 Pull Request。提交前请确保测试通过：

Windows：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\test.ps1
```

macOS / Linux：

```bash
bash scripts/test.sh
```

请保持提交范围单一，并在 PR 中说明改动目的和验证方式。

## 作者

程序员龙龙（lonffrey1）

- 邮箱：lonffrey@163.com
- 小红书：indieDevLong（程序员龙龙）
- 抖音：2098958109（程序员龙龙）
- Gitee：https://gitee.com/lonffrey1

项目反馈、功能建议、合作交流欢迎通过以上方式联系。如果 Audio2Sub 对你有帮助，欢迎关注、分享或给项目点一个 Star。

## 开源协议

本项目使用 [MIT License](LICENSE)。

## 致谢

- 功能与接口流程参考：[yyzq-cf/ywsj-audio-to-subtitle](https://github.com/yyzq-cf/ywsj-audio-to-subtitle)
- 必剪 ASR 接口来自 B 站必剪
- Tauri、Rust、React、FFmpeg 及所有开源依赖的维护者

## 免责声明

本项目与 B 站 / 必剪官方无关联。必剪接口属于非公开接口，使用本项目访问该接口的风险由使用者自行承担。请遵守相关服务条款、版权规定和当地法律。通过 OpenAI、Groq、硅基流动等第三方服务识别时，还需遵守对应服务商的条款和计费规则。

