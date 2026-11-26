#!/bin/zsh
# prune.sh — agent-mesh 数据治理（防膨胀）
#
# 用法:
#   prune.sh              dry-run：只报告将清理什么
#   prune.sh --apply      执行
#   prune.sh --days 14 --apply   自定义保留天数（默认 30）
#
# 动作:
#   1. results/ 下超过 N 天的 run 结果目录移入系统废纸篓（可恢复，不用 rm -rf）
#      保留: *-state.json / ledger.jsonl / trend.jsonl / board.html（发布定位与统计依赖）
#   2. ledger.jsonl 超过 5000 行时轮转保留最近 5000 行（被清理 run 的历史随之失效属正常）
# 退出码: 恒 0（清理失败才非 0）

set -uo pipefail
MESH="${MESH_ROOT:-$(cd "$(dirname "${0:A}")" && pwd)}"
DAYS=30
APPLY=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --apply) APPLY=1; shift ;;
    --days) DAYS="$2"; shift 2 ;;
    *) echo "usage: prune.sh [--days N] [--apply]" >&2; exit 2 ;;
  esac
done

CUTOFF=$(python3 -c "import datetime; print((datetime.datetime.now()-datetime.timedelta(days=$DAYS)).strftime('%Y%m%d'))")

print -r -- "prune: 保留 ${DAYS} 天（cutoff $CUTOFF）· $([ $APPLY -eq 1 ] && echo APPLY || echo dry-run)"

REMOVED=0; KEPT=0
for d in "$MESH"/results/*/; do
  name=$(basename "$d")
  # 从目录名时间戳判断（*-YYYYMMDD-HHMMSS*）；无时间戳的（run 目录如 smoke-pipeline）按 mtime
  ts=$(print -r -- "$name" | grep -oE '[0-9]{8}' | head -1)
  if [[ -z "$ts" ]]; then
    ts=$(date -r "$d" +%Y%m%d 2>/dev/null || echo 99999999)
  fi
  if [[ "$ts" -lt "$CUTOFF" ]]; then
    if [[ $APPLY -eq 1 ]]; then
      # 移入系统废纸篓（可恢复），不使用 rm -rf
      DEST="$HOME/.Trash/agent-mesh-prune-$(date +%Y%m%d%H%M%S)-$name"
      mv "$d" "$DEST" && print -r -- "  🗑→Trash  $name"
    else
      print -r -- "  [将移 Trash] $name"
    fi
    REMOVED=$((REMOVED+1))
  else
    KEPT=$((KEPT+1))
  fi
done

# ledger 轮转
LEDGER="$MESH/results/ledger.jsonl"
if [[ -f "$LEDGER" ]]; then
  LINES=$(wc -l < "$LEDGER" | tr -d ' ')
  if [[ "$LINES" -gt 5000 ]]; then
    if [[ $APPLY -eq 1 ]]; then
      tail -5000 "$LEDGER" > "$LEDGER.tmp" && mv "$LEDGER.tmp" "$LEDGER"
    fi
    print -r -- "  ledger: $LINES 行 → $([ $APPLY -eq 1 ] && echo '已轮转至' || echo '将轮转至') 5000 行"
  fi
fi

print -r -- "prune: $([ $APPLY -eq 1 ] && echo '已清理' || echo '将清理') $REMOVED 个过期目录，保留 $KEPT 个"
exit 0
