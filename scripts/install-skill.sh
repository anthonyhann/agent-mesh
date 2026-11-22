#!/bin/zsh
# install-skill.sh — 把 agent-mesh skill 安装（symlink）到本机各 Agent 的技能目录
#
# 用法:
#   ./scripts/install-skill.sh            # 安装到所有检测到的 agent
#   ./scripts/install-skill.sh --list     # 只列出检测到的目录
#
# 原理：symlink 指向仓库内 skills/agent-mesh/，仓库更新即所有 agent 同步。
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SKILL_SRC="$ROOT/skills/agent-mesh"
[[ -f "$SKILL_SRC/SKILL.md" ]] || { echo "skill not found: $SKILL_SRC" >&2; exit 1; }

# 各 agent 的 skills 目录约定（存在的才装）
CANDIDATES=(
  "$HOME/.agents/skills"
  "$HOME/.claude/skills"
  "$HOME/.catpaw/skills"
  "$HOME/.config/opencode/skills"
  "$HOME/.cursor/skills"
  "$HOME/.codebuddy/skills"
  "$HOME/.qwen/skills"
  "$HOME/.junie/skills"
  "$HOME/.hermes/skills"
  "$HOME/.workbuddy-ai/skills"
)

LIST_ONLY=0
[[ "${1:-}" == "--list" ]] && LIST_ONLY=1

installed=0
for dir in "${CANDIDATES[@]}"; do
  [[ -d "$dir" ]] || continue
  dest="$dir/agent-mesh"
  if [[ $LIST_ONLY -eq 1 ]]; then
    echo "  检测到: $dir"
    continue
  fi
  if [[ -L "$dest" ]]; then
    echo "  已是链接: $dest（跳过）"
  elif [[ -e "$dest" ]]; then
    echo "  ⚠️  $dest 已存在且非链接（跳过，不覆盖）"
  else
    ln -s "$SKILL_SRC" "$dest" && { echo "  ✅ $dest"; installed=$((installed+1)); }
  fi
done
[[ $LIST_ONLY -eq 0 ]] && echo "完成：新装 $installed 处。各 Agent 重开会话即可发现 agent-mesh skill。"
exit 0
