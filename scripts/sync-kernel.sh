#!/bin/zsh
# sync-kernel.sh — 把仓库根的内核脚本同步进 agentchat/kernel/（打包源）
# 内核改动后、tauri build 前执行。K0-K4 改动必须同步才会进安装包。
set -euo pipefail
ROOT="$(cd "$(dirname "${0:A}")/.." && pwd)"
FILES=(run_worker.sh run_pipeline.sh run_dag.py render_session.py render_flow.py mesh_board.py redact.py prune.sh)
for f in "${FILES[@]}"; do
  cp "$ROOT/$f" "$ROOT/agentchat/kernel/$f"
done
chmod +x "$ROOT"/agentchat/kernel/*.sh "$ROOT"/agentchat/kernel/*.py
print -r -- "kernel synced: ${#FILES[@]} files -> agentchat/kernel/"
