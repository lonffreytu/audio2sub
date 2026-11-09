#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

for tool in cargo npm ffmpeg ffprobe; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "缺少依赖: $tool" >&2
        exit 1
    fi
done

run() {
    echo
    echo "==> $1"
    shift
    "$@"
}

run "Rust 格式检查" cargo fmt --all -- --check
run "Rust Clippy" cargo clippy --workspace --all-targets -- -D warnings
run "Rust 单元/集成/E2E 测试" cargo test --workspace

if [[ "${SKIP_DESKTOP:-0}" != "1" ]]; then
    pushd apps/desktop >/dev/null

    if [[ ! -d node_modules ]]; then
        run "安装前端依赖" npm install
    fi

    run "前端类型检查与构建" npm run build
    popd >/dev/null

    pushd apps/desktop/src-tauri >/dev/null
    run "桌面端格式检查" cargo fmt -- --check
    run "桌面端 Clippy" cargo clippy --all-targets -- -D warnings
    run "桌面端编译检查" cargo check
    run "桌面端测试" cargo test
    popd >/dev/null
fi

echo
echo "全部检查通过。"
