#!/bin/zsh
# setup.sh — agent-mesh 环境自检与初始化
set -uo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(dirname "$SCRIPT_DIR")"

echo "== agent-mesh setup =="
echo "root: $ROOT"
echo

ok() { echo "  ✅ $1"; }
warn() { echo "  ⚠️  $1"; }
bad() { echo "  ❌ $1"; }

echo "▸ 运行时依赖"
command -v python3 >/dev/null && ok "python3 $(python3 --version 2>&1 | awk '{print $2}')" || bad "python3 缺失"
command -v zstd >/dev/null && ok "zstd（dsh tokens 采集需要）" || warn "zstd 缺失（brew install zstd）——dsh tokens 将为空"
command -v timeout >/dev/null && ok "timeout（coreutils）" || warn "timeout 缺失（brew install coreutils）——任务超时保护不可用"

echo "▸ Worker CLI（装几个用几个，至少一个）"
for w in dsh codex opencode hermes arkcli claude; do
  if command -v "$w" >/dev/null; then
    ok "$w → $(command -v $w)"
  else
    warn "$w 未安装"
  fi
done

echo "▸ 可选能力"
[[ -d "$HOME/.workbuddy-ai" ]] && ok "workbuddy（在线页发布）" || warn "无 workbuddy：publish_result.sh 将报 PUBLISHER_MISSING"
[[ -n "${ANTHROPIC_API_KEY:-}" || -f "$ROOT/claude.env" ]] && ok "claude 已配置" || warn "claude 未启用（写 key 到 ./claude.env 即激活）"

echo
echo "▸ 初始化"
mkdir -p "$ROOT/results" && ok "results/ 就绪"

echo
echo "自检完成。快速验证：./smoke.sh --quick；全量：./smoke.sh"
