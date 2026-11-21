#!/bin/zsh
# publish_result.sh — agent-mesh 发布末端：任意 HTML → workbuddy 在线页
#
# 用法:
#   publish_result.sh <file.html> [--name <显示名>] [--node-block-id <已有页 id，原地更新>] [--no-redact]
#
# 脱敏（默认开启，--no-redact 关闭）：上传前将 HTML 中的 $HOME 绝对路径替换为 ~，
# 防止公开页面泄露本机用户名与目录结构。本地原文件不动，脱敏作用于上传副本。
#
# 输出（stdout 单行 JSON）:
#   成功: {"ok":true,"node_block_id":"...","url":"https://workbuddy.cc/p/...","updated":true|false}
#   失败: {"ok":false,"stage":"import|publish","error":"..."}
#
# 约定:
#   - 自动选择 skill-library 最新版本目录（按目录修改时间）
#   - WORKBUDDY_CONFIG_DIR 默认 ~/.workbuddy-ai（wbipc 宿主 IPC 鉴权）
#   - exit code: 成功 0，失败 1

set -uo pipefail

export WORKBUDDY_CONFIG_DIR="${WORKBUDDY_CONFIG_DIR:-$HOME/.workbuddy-ai}"

fail() { print -r -- "{\"ok\":false,\"stage\":\"$1\",\"error\":$(python3 -c 'import json,sys;print(json.dumps(sys.argv[1]))' "$2")}"; exit 1; }

[[ $# -ge 1 && -f "$1" ]] || fail "args" "usage: publish_result.sh <file.html> [--name n] [--node-block-id id] [--no-redact]"
HTML_PATH="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"; shift

NAME=""
NODE_BLOCK_ID=""
REDACT=1
while [[ $# -gt 0 ]]; do
  case "$1" in
    --name) NAME="$2"; shift 2 ;;
    --node-block-id) NODE_BLOCK_ID="$2"; shift 2 ;;
    --no-redact) REDACT=0; shift ;;
    *) fail "args" "unknown flag: $1" ;;
  esac
done

# 最新版 skill-library（按 mtime，规避 sort -V 平台差异）
LIB_ROOT=$(ls -dt "$HOME"/.workbuddy-ai/plugins/cache/workbuddy-builtin/skill-library/*/page 2>/dev/null | head -1)
[[ -n "$LIB_ROOT" && -f "$LIB_ROOT/import_html.py" ]] || fail "env" "PUBLISHER_MISSING: 未检测到 workbuddy 发布后端（~/.workbuddy-ai）。发布为可选能力：HTML 产物已在本地 results/ 可直接使用；如需在线页请安装 workbuddy 或改用你自己的发布脚本替换本文件。"

# ---- 脱敏（默认）：生成上传用脱敏副本（逻辑在 redact.py，可单测） ----
UPLOAD_PATH="$HTML_PATH"
if [[ $REDACT -eq 1 ]]; then
  REDACT_PY="$(cd "$(dirname "$0")" && pwd)/redact.py"
  UPLOAD_PATH="$(mktemp /tmp/publish-redact-XXXXXX.html)"
  python3 "$REDACT_PY" "$HTML_PATH" "$UPLOAD_PATH" || fail "redact" "redact.py failed"
  trap 'rm -f "$UPLOAD_PATH"' EXIT
fi

# ---- import ----
typeset -a IMPORT_ARGS=("$UPLOAD_PATH")
[[ -n "$NAME" ]] && IMPORT_ARGS+=(--file-name "$NAME")
[[ -n "$NODE_BLOCK_ID" ]] && IMPORT_ARGS+=(--node-block-id "$NODE_BLOCK_ID")

IMPORT_OUT=$(python3 "$LIB_ROOT/import_html.py" "${IMPORT_ARGS[@]}" 2>/dev/null)
OK_LINE=$(print -r -- "$IMPORT_OUT" | grep '^KS_IMPORT_OK ' | tail -1)
[[ -n "$OK_LINE" ]] || fail "import" "$(print -r -- "$IMPORT_OUT" | head -c 300)"

IMPORT_JSON="${OK_LINE#KS_IMPORT_OK }"
NODE_ID=$(python3 -c "import json,sys; print(json.loads(sys.argv[1]).get('node_block_id',''))" "$IMPORT_JSON")
[[ -n "$NODE_ID" ]] || fail "import" "node_block_id missing in: $IMPORT_JSON"

# ---- publish ----
PUB_OUT=$(python3 "$LIB_ROOT/publish_page.py" --node-id "$NODE_ID" 2>/dev/null)
# 输出为多行混合：JSON 信封 + KS_PAGE_PUBLISH\t<url> + KS_USER_REPLY；优先抓 KS_PAGE_PUBLISH 行
URL=$(print -r -- "$PUB_OUT" | awk -F'\t' '/^KS_PAGE_PUBLISH/{print $2}' | tail -1)
if [[ -z "$URL" ]]; then
  URL=$(print -r -- "$PUB_OUT" | grep '^{' | head -1 | python3 -c "
import json, sys
try:
    d = json.loads(sys.stdin.read())
except Exception:
    print(''); sys.exit()
def find_url(o):
    if isinstance(o, dict):
        for k, v in o.items():
            if k in ('url','publish_url','publishUrl') and isinstance(v, str) and v.startswith('http'):
                return v
            r = find_url(v)
            if r: return r
    elif isinstance(o, list):
        for i in o:
            r = find_url(i)
            if r: return r
    return ''
print(find_url(d) or '')")
fi
[[ -n "$URL" ]] || fail "publish" "$(print -r -- "$PUB_OUT" | head -c 300)"

UPDATED=false
[[ -n "$NODE_BLOCK_ID" ]] && UPDATED=true
python3 -c "
import json, sys
print(json.dumps({'ok': True, 'node_block_id': sys.argv[1], 'url': sys.argv[2], 'updated': sys.argv[3] == 'true'}, ensure_ascii=False))
" "$NODE_ID" "$URL" "$UPDATED"
exit 0
