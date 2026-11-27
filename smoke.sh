#!/bin/zsh
# smoke.sh — agent-mesh 自检套件
#
# 用法:
#   smoke.sh           全量（语法 + 单元 + 五工人真 ping + 迷你 pipeline，有模型调用成本）
#   smoke.sh --quick   快速（仅语法 + 单元测试，不调模型）
#
# 退出码: 全部 PASS=0，任一 FAIL=1

set -uo pipefail
MESH="${MESH_ROOT:-$(cd "$(dirname "${0:A}")" && pwd)}"
QUICK=0
[[ "${1:-}" == "--quick" ]] && QUICK=1

PASS=0; FAIL=0
ok()   { PASS=$((PASS+1)); print -r -- "  ✅ $1"; }
bad()  { FAIL=$((FAIL+1)); print -r -- "  ❌ $1"; }
section() { print -r -- "\n▸ $1"; }

# ---------- 1. 语法 ----------
section "语法检查"
SYNTAX_OK=1
for f in "$MESH"/*.sh; do
  zsh -n "$f" 2>/dev/null || { SYNTAX_OK=0; bad "zsh syntax: $(basename "$f")"; }
done
for f in "$MESH"/*.py; do
  python3 -c "import ast,sys; ast.parse(open(sys.argv[1]).read())" "$f" 2>/dev/null \
    || { SYNTAX_OK=1; SYNTAX_OK=0; bad "python syntax: $(basename "$f")"; }
done
[[ $SYNTAX_OK -eq 1 ]] && ok "全部脚本语法"

# ---------- 2. 单元测试 ----------
section "单元测试"
# 2.1 redact.py 脱敏
T_IN=$(mktemp /tmp/smoke-redact-in-XXXX.html)
T_OUT=$(mktemp /tmp/smoke-redact-out-XXXX.html)
print -r -- "<p>路径 $HOME/.agents/skills 与 /Users/hanqiang/x</p>" > "$T_IN"
python3 "$MESH/redact.py" "$T_IN" "$T_OUT"
if ! grep -q "/Users/hanqiang" "$T_OUT" && ! grep -q "$HOME" "$T_OUT"; then
  ok "redact.py 脱敏"
else
  bad "redact.py 残留路径: $(cat "$T_OUT")"
fi
rm -f "$T_IN" "$T_OUT"

# 2.2 mesh_config 共享配置（examples/skills-governance 应用示例的模块）
if [[ -f "$MESH/examples/skills-governance/mesh_config.py" ]]; then
  ( cd "$MESH/examples/skills-governance" && python3 -c "
from mesh_config import AGENT_DIRS, CENTRAL
assert len(AGENT_DIRS) >= 1
print('ok')
" ) >/dev/null 2>&1 && ok "mesh_config 共享配置" || bad "mesh_config 加载失败"
else
  print -r -- "  ⏭️  mesh_config（examples 未安装，跳过）"
fi

# 2.3 rollback.py dry-run（用存在的备份目录）
if [[ -d "$MESH/backups" ]]; then
  LATEST_BK=$(ls -dt "$MESH"/backups/*/ 2>/dev/null | head -1)
else
  LATEST_BK=""
fi
if [[ -n "$LATEST_BK" && -f "$MESH/examples/skills-governance/rollback.py" ]]; then
  python3 "$MESH/examples/skills-governance/rollback.py" "$LATEST_BK" >/dev/null 2>&1 && ok "rollback.py dry-run" || bad "rollback.py 运行失败"
else
  print -r -- "  ⏭️  rollback 无备份可测（跳过）"
fi

if [[ $QUICK -eq 1 ]]; then
  # 2.4 内核回归（K0/K1/K2：running 标记/原子写、timeout 探测、env 白名单；假 worker 无模型成本）
  if [[ -x "$MESH/tests/test-kernel.sh" ]]; then
    section "内核回归（K0/K1/K2）"
    "$MESH/tests/test-kernel.sh" && ok "tests/test-kernel.sh 全绿" || bad "tests/test-kernel.sh 有用例失败"
  fi
  print -r -- "\nsmoke(quick): PASS=$PASS FAIL=$FAIL"
  [[ $FAIL -eq 0 ]] && exit 0 || exit 1
fi

# ---------- 3. 工人真 ping（有模型调用；claude 未配 key 时预期跳过） ----------
section "工人 ping"
for w in dsh arkcli codex opencode hermes claude qwen pi; do
  if [[ "$w" == "claude" && -z "${ANTHROPIC_API_KEY:-}" && ! -f "$MESH/claude.env" ]]; then
    print -r -- "  ⏭️  claude 未配置 ANTHROPIC_API_KEY（预期内跳过）"; continue
  fi
  TF=$(ls "$MESH"/examples/tasks/*-ping-"$w".json 2>/dev/null | head -1)
  if [[ -z "$TF" ]]; then
    print -r -- "  ⏭️  $w 无 ping 任务单（跳过）"; continue
  fi
  "$MESH/run_worker.sh" "$TF" >/dev/null 2>&1
  ST=$(python3 -c "import json,glob,sys; print(json.load(open(sys.argv[1]))['status'])" \
       "$MESH/results/$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['id'])" "$TF")/result.json" 2>/dev/null)
  [[ "$ST" == "success" ]] && ok "$w ping" || bad "$w ping -> $ST"
done

# ---------- 4. 迷你 pipeline（验证 answer.txt 归一化与 prev_output 流转） ----------
section "迷你 pipeline（数据流转）"
SMOKE_PIPE="$MESH/tasks/pipeline-smoke.json"
cat > "$SMOKE_PIPE" <<'EOF'
{
  "pipeline_id": "smoke-pipeline",
  "title": "smoke 自检管线",
  "nodes": [
    {"seq": 1, "worker": "arkcli", "label": "生成", "profile": "agent-plan_cn-beijing_personal",
     "thinking": "disabled", "max_output_tokens": 64, "timeout_seconds": 60,
     "brief": "Reply with exactly one word: PING.",
     "done_criteria": {"stdout_regex": "PING"}},
    {"seq": 2, "worker": "dsh", "label": "复述", "timeout_seconds": 120,
     "brief": "上一节点输出是：{{prev_output}}\n请原样复述你收到的那个词，只回该词。",
     "done_criteria": {"stdout_regex": "PING"}}
  ]
}
EOF
"$MESH/run_pipeline.sh" "$SMOKE_PIPE" >/dev/null 2>&1
N1=$(python3 -c "import json; print(json.load(open('$MESH/results/smoke-pipeline-n1/result.json'))['status'])" 2>/dev/null)
N2=$(python3 -c "import json; print(json.load(open('$MESH/results/smoke-pipeline-n2/result.json'))['status'])" 2>/dev/null)
[[ "$N1" == "success" && "$N2" == "success" ]] && ok "pipeline 两节点流转" || bad "pipeline n1=$N1 n2=$N2"
[[ -f "$MESH/results/smoke-pipeline/flow.html" ]] && ok "流转图生成" || bad "flow.html 缺失"

print -r -- "\nsmoke: PASS=$PASS FAIL=$FAIL"
[[ $FAIL -eq 0 ]] && exit 0 || exit 1
