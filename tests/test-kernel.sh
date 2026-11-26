#!/bin/zsh
# test-kernel.sh — K0/K1/K2 内核改动回归测试（假 worker，无需真实 CLI/凭证）
# 用法: tests/test-kernel.sh   退出码 0=全绿
set -uo pipefail
zmodload zsh/datetime 2>/dev/null || true

MESH="$(cd "$(dirname "${0:A}")/.." && pwd)"
TMP="$(mktemp -d /tmp/mesh-kernel-test.XXXXXX)"
cleanup() { rm -rf "$TMP"; rm -rf "$MESH"/results/{k0-basic,k1-timeout,k1-override}; }
trap cleanup EXIT
PASS=0; FAIL=0
ok()  { print -r -- "  ✅ $1"; PASS=$((PASS+1)); }
bad() { print -r -- "  ❌ $1"; FAIL=$((FAIL+1)); }

# ---------- 假 worker：dsh stub（打印参数与环境探针） ----------
mkdir -p "$TMP/bin"
cat > "$TMP/bin/dsh" <<'STUB'
#!/bin/zsh
# 输出答案 + 把环境探针打到 stdout（供 K2 断言）
print -r -- "FAKE_DSH_ANSWER"
print -r -- "PROBE_MY_TEST_VAR=${MY_TEST_VAR:-<absent>}"
print -r -- "PROBE_CODEX_SESSION_ID=${CODEX_SESSION_ID:-<absent>}"
print -r -- "PROBE_AWS_SECRET=${AWS_SECRET_ACCESS_KEY:-<absent>}"
print -r -- "PROBE_CODEX_API_KEY=${CODEX_API_KEY:-<absent>}"
print -r -- "PROBE_GITHUB_TOKEN=${GITHUB_TOKEN:-<absent>}"
STUB
chmod +x "$TMP/bin/dsh"

# 慢速假 worker（K1 超时用例）
cat > "$TMP/bin/dsh-slow" <<'STUB'
#!/bin/zsh
sleep 30
print -r -- "SHOULD_NOT_REACH"
STUB
chmod +x "$TMP/bin/dsh-slow"

export PATH="$TMP/bin:$PATH"
export MY_TEST_VAR="injected-ok" \
       CODEX_SESSION_ID="host-leak-check" \
       AWS_SECRET_ACCESS_KEY="aws-leak-check" \
       CODEX_API_KEY="codex-key-ok" \
       GITHUB_TOKEN="gh-leak-check" \
       MESH_ENV_EXTRA_VARS="MY_TEST_VAR AWS_SECRET_ACCESS_KEY CODEX_SESSION_ID * BAD?NAME"

mktask() { # $1=id $2=timeout
  cat > "$TMP/$1.json" <<EOF
{"id":"$1","worker":"dsh","brief":"fake task","timeout_seconds":$2}
EOF
}

print -r -- "== K0: running 标记 + 原子写 =="
mktask k0-basic 60
( cd "$TMP" && "$MESH/run_worker.sh" "$TMP/k0-basic.json" >/dev/null 2>&1 )
R="$MESH/results/k0-basic"
[[ -f "$R/result.json" && ! -f "$R/running" ]] \
  && ok "result.json 落盘且 running 标记已清除" \
  || bad "running 标记残留或 result.json 缺失（$R）"
[[ ! -f "$R/result.json.tmp" && ! -f "$R/answer.txt.tmp" ]] \
  && ok "无 .tmp 临时文件残留（原子 rename 生效）" \
  || bad ".tmp 临时文件残留"
[[ -f "$R/answer.txt" ]] && ok "answer.txt 存在" || bad "answer.txt 缺失"

print -r -- "== K1: timeout 探测 =="
if command -v timeout >/dev/null 2>&1; then
  mktask k1-timeout 2
  # 把 dsh 换成慢速版
  cp "$TMP/bin/dsh" "$TMP/bin/dsh.orig"; cp "$TMP/bin/dsh-slow" "$TMP/bin/dsh"
  T0=$EPOCHREALTIME
  ( cd "$TMP" && "$MESH/run_worker.sh" "$TMP/k1-timeout.json" >/dev/null 2>&1 )
  ELAPSED=$(( (${EPOCHREALTIME} - T0) ))
  cp "$TMP/bin/dsh.orig" "$TMP/bin/dsh"
  STATUS=$(python3 -c "import json;print(json.load(open('$MESH/results/k1-timeout/result.json'))['status'])" 2>/dev/null)
  [[ "$STATUS" == "timeout" ]] && ok "GNU timeout 生效：2s 超时状态=timeout（耗时 ${ELAPSED}s）" \
    || bad "超时用例状态异常：$STATUS（期望 timeout）"
else
  ok "本机无 timeout 命令，跳过超时用例（K1 直接 exec 路径已由 K0 用例覆盖）"
fi
# MESH_TIMEOUT_BIN 覆盖：指定一个必定失败的假 timeout
mktask k1-override 60
MESH_TIMEOUT_BIN=/bin/false "$MESH/run_worker.sh" "$TMP/k1-override.json" >/dev/null 2>&1
STATUS=$(python3 -c "import json;print(json.load(open('$MESH/results/k1-override/result.json'))['status'])" 2>/dev/null)
[[ "$STATUS" == "failed" ]] && ok "MESH_TIMEOUT_BIN 覆盖生效（/bin/false → failed）" \
  || bad "MESH_TIMEOUT_BIN 覆盖异常：$STATUS（期望 failed）"

print -r -- "== K2: env 精确白名单 + denylist =="
S="$MESH/results/k0-basic/stdout.txt"
grep -q "PROBE_MY_TEST_VAR=injected-ok" "$S" \
  && ok "MESH_ENV_EXTRA_VARS 精确变量注入成功" || bad "MY_TEST_VAR 未注入"
grep -q "PROBE_CODEX_API_KEY=codex-key-ok" "$S" \
  && ok "per-worker 精确凭证（CODEX_API_KEY）保留" || bad "CODEX_API_KEY 丢失"
grep -q "PROBE_CODEX_SESSION_ID=<absent>" "$S" \
  && ok "宿主 CODEX_SESSION_ID 已剔除（宽前缀泄漏防护）" || bad "CODEX_SESSION_ID 泄漏！"
grep -q "PROBE_AWS_SECRET=<absent>" "$S" \
  && ok "AWS_SECRET_ACCESS_KEY 被 denylist 拦截（即使显式列入 EXTRA_VARS）" || bad "AWS_SECRET 泄漏！"
grep -q "PROBE_GITHUB_TOKEN=<absent>" "$S" \
  && ok "GITHUB_TOKEN 已剔除" || bad "GITHUB_TOKEN 泄漏！"
# 通配符条目被忽略（不崩溃即通过，上面断言已覆盖语义）

print -r -- ""
print -r -- "结果：$PASS 通过 / $FAIL 失败"
[[ $FAIL -eq 0 ]]
