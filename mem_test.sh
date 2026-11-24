#!/bin/zsh
# mem_test.sh — 五 worker 会话续接记忆验证
# 每个 worker 两轮：r1 记暗号 → r2 凭 session_id 复述。暗号按 worker 区分防串台。
set -uo pipefail
MESH="${MESH_ROOT:-$(cd "$(dirname "${0:A}")" && pwd)}"
cd "$MESH"

PASS=0; FAIL=0
for w in codex opencode hermes dsh arkcli claude qwen; do
  TOKEN="KUMQUAT-${w}-42"
  if [[ "$w" == "claude" && -z "${ANTHROPIC_API_KEY:-}" && ! -f "$MESH/claude.env" ]]; then
    print -r -- "  ⏭️  claude 未配置 ANTHROPIC_API_KEY（预期内跳过）"; continue
  fi
  EXTRA1=""
  EXTRA2=""
  [[ "$w" == "arkcli" ]] && EXTRA1=',"profile":"agent-plan_cn-beijing_personal","thinking":"disabled","max_output_tokens":64' && EXTRA2="$EXTRA1"
  # dsh 首轮不传 session_id（headless 只 adopt 已存在 session，新建后由适配器提取 id）

  cat > /tmp/mem-$w-1.json <<EOF
{"id":"mem-$w-1","worker":"$w","timeout_seconds":150$EXTRA1,
 "brief":"请记住暗号 $TOKEN。只回复 OK 两个字，不要任何其他内容。"}
EOF
  ./run_worker.sh /tmp/mem-$w-1.json >/dev/null 2>&1
  SID=$(python3 -c "import json; print(json.load(open('results/mem-$w-1/result.json')).get('session_id') or '')" 2>/dev/null)

  if [[ -n "$SID" ]]; then
    SESS_FIELD=",\"session_id\":\"$SID\""
  else
    print -r -- "  ⚠️  $w: r1 未提取到 session_id，跳过续接测试"; FAIL=$((FAIL+1)); continue
  fi

  cat > /tmp/mem-$w-2.json <<EOF
{"id":"mem-$w-2","worker":"$w","timeout_seconds":150$EXTRA2$SESS_FIELD,
 "brief":"我上一轮让你记住的暗号是什么？只回答暗号本身，不要其他内容。",
 "done_criteria":{"stdout_regex":"$TOKEN"}}
EOF
  if ./run_worker.sh /tmp/mem-$w-2.json >/dev/null 2>&1; then
    ANS=$(python3 -c "import json; print(json.load(open('results/mem-$w-2/result.json'))['answer_preview'][:80])" 2>/dev/null)
    print -r -- "  ✅ $w 会话记忆 PASS（sid=${SID:0:24}… 答: $ANS）"; PASS=$((PASS+1))
  else
    ST=$(python3 -c "import json; print(json.load(open('results/mem-$w-2/result.json'))['status'])" 2>/dev/null)
    ANS=$(python3 -c "import json; print(json.load(open('results/mem-$w-2/result.json'))['answer_preview'][:80])" 2>/dev/null)
    print -r -- "  ❌ $w 会话记忆 FAIL（status=$ST 答: $ANS）"; FAIL=$((FAIL+1))
  fi
done
print -r -- "mem_test: PASS=$PASS FAIL=$FAIL"
[[ $FAIL -eq 0 ]] && exit 0 || exit 1
