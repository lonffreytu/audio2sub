#!/usr/bin/env bash
set -euo pipefail

for tool in curl jq; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "缺少依赖: $tool" >&2
        exit 1
    fi
done

: "${GITEE_TOKEN:?请设置 GITEE_TOKEN}"
: "${GITEE_OWNER:?请设置 GITEE_OWNER}"
: "${GITEE_REPO:?请设置 GITEE_REPO}"

TAG="${1:?用法: publish-gitee.sh <tag> <file...>}"
shift

if [[ $# -eq 0 ]]; then
    echo "没有需要上传的文件" >&2
    exit 1
fi

API="https://gitee.com/api/v5/repos/${GITEE_OWNER}/${GITEE_REPO}"

existing="$(curl -fsSL "${API}/releases/tags/${TAG}?access_token=${GITEE_TOKEN}" 2>/dev/null || true)"
release_id="$(printf '%s' "$existing" | jq -r '.id // empty' 2>/dev/null || true)"

if [[ -z "$release_id" ]]; then
    echo "创建 Gitee Release: ${TAG}"
    payload="$(jq -n \
        --arg token "$GITEE_TOKEN" \
        --arg tag "$TAG" \
        --arg name "Audio2Sub ${TAG}" \
        --arg body "Audio2Sub ${TAG} 自动构建发布。" \
        '{access_token: $token, tag_name: $tag, name: $name, body: $body}')"
    created="$(curl -fsSL -X POST "${API}/releases" \
        -H "Content-Type: application/json" \
        -d "$payload")"
    release_id="$(printf '%s' "$created" | jq -r '.id')"
fi

if [[ -z "$release_id" || "$release_id" == "null" ]]; then
    echo "无法创建或获取 Gitee Release" >&2
    exit 1
fi

echo "Gitee Release ID: ${release_id}"

for file in "$@"; do
    if [[ ! -f "$file" ]]; then
        echo "文件不存在: $file" >&2
        exit 1
    fi
    echo "上传: $(basename "$file")"
    curl -fsSL -X POST "${API}/releases/${release_id}/attach_files" \
        -F "access_token=${GITEE_TOKEN}" \
        -F "file=@${file}"
    echo
done

echo "Gitee Release 上传完成。"
