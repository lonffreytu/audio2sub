param(
    [switch]$SkipDesktop,
    [switch]$SkipInstall
)

$ErrorActionPreference = "Stop"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root

function Assert-LastExitCode {
    param([string]$Name)
    if ($LASTEXITCODE -ne 0) {
        throw "步骤失败: $Name (exit $LASTEXITCODE)"
    }
}

try {
    Write-Host ""
    Write-Host "==> Rust 格式检查" -ForegroundColor Cyan
    cargo fmt --all -- --check
    Assert-LastExitCode "Rust 格式检查"

    Write-Host ""
    Write-Host "==> Rust Clippy" -ForegroundColor Cyan
    cargo clippy --workspace --all-targets -- -D warnings
    Assert-LastExitCode "Rust Clippy"

    Write-Host ""
    Write-Host "==> Rust 单元/集成/E2E 测试" -ForegroundColor Cyan
    cargo test --workspace
    Assert-LastExitCode "Rust 测试"

    if (-not $SkipDesktop) {
        Push-Location "apps/desktop"
        try {
            if (-not $SkipInstall -and -not (Test-Path "node_modules")) {
                Write-Host ""
                Write-Host "==> 安装前端依赖" -ForegroundColor Cyan
                npm install
                Assert-LastExitCode "安装前端依赖"
            }

            Write-Host ""
            Write-Host "==> 前端类型检查与构建" -ForegroundColor Cyan
            npm run build
            Assert-LastExitCode "前端构建"
        }
        finally {
            Pop-Location
        }

        Push-Location "apps/desktop/src-tauri"
        try {
            Write-Host ""
            Write-Host "==> 桌面端格式检查" -ForegroundColor Cyan
            cargo fmt -- --check
            Assert-LastExitCode "桌面端格式检查"

            Write-Host ""
            Write-Host "==> 桌面端 Clippy" -ForegroundColor Cyan
            cargo clippy --all-targets -- -D warnings
            Assert-LastExitCode "桌面端 Clippy"

            Write-Host ""
            Write-Host "==> 桌面端编译检查" -ForegroundColor Cyan
            cargo check
            Assert-LastExitCode "桌面端编译检查"

            Write-Host ""
            Write-Host "==> 桌面端测试" -ForegroundColor Cyan
            cargo test
            Assert-LastExitCode "桌面端测试"
        }
        finally {
            Pop-Location
        }
    }

    Write-Host ""
    Write-Host "全部检查通过。" -ForegroundColor Green
}
finally {
    Pop-Location
}
