#!/bin/zsh
# run_worker.sh — multi-harness worker adapter (agent-mesh MVP)
#
# 用法:
#   run_worker.sh <task.json>
#
# 任务单 schema v1:
#   id              必填  结果目录名（results/<id>/）
#   worker          必填  dsh | codex | opencode | hermes | arkcli
#   brief           必填  任务描述（传给 worker 的 prompt）
#   workspace       可选  执行前 cd 到该目录
#   timeout_seconds 可选  默认 300；超时状态记为 timeout（exit 124）
#   model           可选  arkcli --model / opencode -m
#   session_id      可选  dsh --session-id（会话续接）
#   permission_mode 可选  dsh --permission-mode
#   profile         可选  arkcli --profile（火山方舟 profile 切换）
#   max_output_tokens 可选 arkcli --max-output-tokens
#   done_criteria   可选  {"stdout_regex": "<ERE>"}；缺省判定 exit==0 且 stdout 非空
#
# 产物:
#   results/<id>/stdout.txt   worker 原始 stdout（答案本体）
#   results/<id>/stderr.txt   worker 诊断输出
#   results/<id>/result.json  结构化结果（status/duration/criteria 判定）
#
# 退出码: success=0，其余（failed/timeout/criteria_failed）=1

set -uo pipefail

MESH_ROOT="${MESH_ROOT:-$(cd "$(dirname "${0:A}")" && pwd)}"
DEFAULT_TIMEOUT=300

die() { print -r -- "run_worker: $*" >&2; exit 2; }

[[ $# -eq 1 && -f "$1" ]] || die "usage: run_worker.sh <task.json>"
TASK_FILE="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
command -v python3 >/dev/null || die "python3 not found"

# ---------- 解析任务单（python3 一次性导出 shell 变量） ----------
eval "$(python3 - "$TASK_FILE" <<'PYEOF'
import json, shlex, sys
d = json.load(open(sys.argv[1]))
def emit(name, val):
    if val is None or val == "":
        print(f'{name}=')
    else:
        print(f'{name}={shlex.quote(str(val))}')
emit("T_ID",       d.get("id"))
emit("T_WORKER",   d.get("worker"))
emit("T_BRIEF",    d.get("brief"))
emit("T_WORKSPACE", d.get("workspace"))
emit("T_TIMEOUT",  d.get("timeout_seconds"))
emit("T_MODEL",    d.get("model"))
emit("T_SESSION",  d.get("session_id"))
emit("T_PERM",     d.get("permission_mode"))
emit("T_MAXTOK",   d.get("max_output_tokens"))
emit("T_PROFILE",  d.get("profile"))
emit("T_SCHEMA",   d.get("text_schema"))
emit("T_THINKING", d.get("thinking"))
retry = d.get("retry") or {}
emit("T_RETRY_MAX", retry.get("max_attempts"))
emit("T_RETRY_BACKOFF", retry.get("backoff_s"))
dc = d.get("done_criteria") or {}
emit("T_REGEX",    dc.get("stdout_regex"))
PYEOF
)"

[[ -n "$T_ID" && -n "$T_WORKER" && -n "$T_BRIEF" ]] || die "task missing required fields (id/worker/brief)"
TIMEOUT_SECS="${T_TIMEOUT:-$DEFAULT_TIMEOUT}"

OUT_DIR="$MESH_ROOT/results/$T_ID"
mkdir -p "$OUT_DIR"
STDOUT_F="$OUT_DIR/stdout.txt"
STDERR_F="$OUT_DIR/stderr.txt"

# ---------- worker 适配（构造外部命令数组，供 timeout 直接调用） ----------
# 注意：GNU timeout 只能执行外部命令，无法调用 shell 函数，故统一构造 CMD 数组。
typeset -a CMD
case "$T_WORKER" in
  dsh)
    CMD=(dsh --profile headless --approval-policy never)
    [[ -n "$T_SESSION" ]] && CMD+=(--session-id "$T_SESSION")
    [[ -n "$T_PERM" ]]    && CMD+=(--permission-mode "$T_PERM")
    CMD+=("$T_BRIEF")
    ;;
  codex)
    # codex exec 默认非交互；-s workspace-write 允许写工作区不走交互审批（无人值守）
    # --skip-git-repo-check：派单目录不一定是 git 仓库，跳过可信目录检查
    if [[ -n "$T_SESSION" ]]; then
      # 会话续接：codex exec resume [OPTIONS] <session_id> [prompt]
      # 注意：resume 不支持 -s/--sandbox（沿用原 session 配置），可用 --skip-git-repo-check
      CMD=(codex exec resume --skip-git-repo-check "$T_SESSION" "$T_BRIEF")
    else
      CMD=(codex exec -s workspace-write --skip-git-repo-check "$T_BRIEF")
    fi
    ;;
  opencode)
    CMD=(opencode run)
    [[ -n "$T_MODEL" ]] && CMD+=(-m "$T_MODEL")
    [[ -n "$T_SESSION" ]] && CMD+=(-s "$T_SESSION")
    CMD+=("$T_BRIEF")
    ;;
  hermes)
    CMD=(hermes -z "$T_BRIEF")
    [[ -n "$T_SESSION" ]] && CMD+=(--resume "$T_SESSION")
    ;;
  arkcli)
    CMD=(arkcli)
    [[ -n "$T_PROFILE" ]] && CMD+=(--profile "$T_PROFILE")
    CMD+=(+chat "$T_BRIEF")
    [[ -n "$T_MODEL" ]]  && CMD+=(--model "$T_MODEL")
    [[ -n "$T_MAXTOK" ]] && CMD+=(--max-output-tokens "$T_MAXTOK")
    [[ -n "$T_SCHEMA" ]] && CMD+=(--text-format json_schema --text-schema "$T_SCHEMA")
    [[ -n "$T_THINKING" ]] && CMD+=(--thinking "$T_THINKING")
    # 会话续接：上一轮响应 id（见 result.json 的 session_id）+ --store 使响应可检索
    # --store true 无条件加：响应需先持久化，后续轮次才能用 previous_response_id 续接
    CMD+=(--store true)
    [[ -n "$T_SESSION" ]] && CMD+=(--previous-response-id "$T_SESSION")
    ;;
  claude)
    # Claude Code headless：-p 非交互 + JSON 信封（result/session_id/usage/total_cost_usd）
    # 启用门控：需官方 ANTHROPIC_API_KEY（env 或 $MESH_ROOT/claude.env）。
    # 未配置 → 秒级明确失败（不挂起）；mcli 路由拒绝 sdk 裸调，不要走 mc --code。
    CLAUDE_ENV="$MESH_ROOT/claude.env"
    if [[ -z "${ANTHROPIC_API_KEY:-}" && -f "$CLAUDE_ENV" ]]; then
      set -a; source "$CLAUDE_ENV"; set +a
    fi
    if [[ -z "${ANTHROPIC_API_KEY:-}" ]]; then
      SKIP_EXEC="claude 未启用：未找到 ANTHROPIC_API_KEY。配置：printf 'ANTHROPIC_API_KEY=sk-ant-...\\n' > $CLAUDE_ENV（该文件已 gitignore）"
    else
      CMD=(claude -p "$T_BRIEF" --output-format json --permission-mode bypassPermissions)
      # 官方 API 不认 settings.json 里的 mc-code 路由模型名，必须显式给官方模型
      CMD+=(--model "${T_MODEL:-claude-sonnet-4-5}")
      [[ -n "$T_SESSION" ]] && CMD+=(--resume "$T_SESSION")
    fi
    ;;
  *) die "unknown worker: $T_WORKER (supported: dsh codex opencode hermes arkcli claude)" ;;
esac

# ---------- 执行 ----------
[[ -n "$T_WORKSPACE" ]] && cd "$T_WORKSPACE"

START_TS=$(date +%s)
START_ISO=$(date -u +%Y-%m-%dT%H:%M:%SZ)

# 执行（含重试：仅对执行失败重试——exit≠0/超时；criteria 不过属于"答案不对"不重试）
MAX_ATTEMPTS=${T_RETRY_MAX:-1}
BACKOFF_S=${T_RETRY_BACKOFF:-5}
ATTEMPT=1
if [[ -n "${SKIP_EXEC:-}" ]]; then
  # 凭据门控快速失败路径（如 claude 未配置 key）：不执行、不重试
  : > "$STDOUT_F"
  print -r -- "$SKIP_EXEC" > "$STDERR_F"
  EXIT_CODE=1
else
  while true; do
    # stdin 统一封 /dev/null：任务全部由 brief 提供，防止 worker（如 codex）检测非 TTY 时误读 stdin
    timeout "$TIMEOUT_SECS" "${CMD[@]}" < /dev/null >"$STDOUT_F" 2>"$STDERR_F"
    EXIT_CODE=$?
    if [[ $EXIT_CODE -eq 0 || $ATTEMPT -ge $MAX_ATTEMPTS ]]; then
      break
    fi
    print -r -- "[retry] attempt $ATTEMPT/$MAX_ATTEMPTS exit=$EXIT_CODE, backoff ${BACKOFF_S}s" >&2
    sleep "$BACKOFF_S"
    ATTEMPT=$((ATTEMPT + 1))
  done
fi
ATTEMPTS=$ATTEMPT

END_TS=$(date +%s)
END_ISO=$(date -u +%Y-%m-%dT%H:%M:%SZ)
DURATION=$((END_TS - START_TS))

# ---------- 验收 + 写 result.json ----------
# content-aware：stdout 若为含 content 字段的 JSON 信封（如 arkcli），
# criteria 只对 content 匹配，防止 thinking 模型的 reasoning_content 造成假阳性。
STATUS=$(TASK_FILE="$TASK_FILE" OUT_DIR="$OUT_DIR" T_ID="$T_ID" T_WORKER="$T_WORKER" \
EXIT_CODE="$EXIT_CODE" DURATION="$DURATION" ATTEMPTS="$ATTEMPTS" \
START_ISO="$START_ISO" END_ISO="$END_ISO" T_REGEX="$T_REGEX" \
python3 - <<'PYEOF'
import json, os, re
out = os.environ["OUT_DIR"]
stdout_txt = open(os.path.join(out, "stdout.txt"), errors="replace").read()
regex = os.environ.get("T_REGEX") or None
exit_code = int(os.environ["EXIT_CODE"])

def content_aware_target(txt):
    try:
        d = json.loads(txt)
        if isinstance(d, dict):
            if isinstance(d.get("content"), str):
                return d["content"], "content"
            if isinstance(d.get("result"), str):   # claude --output-format json 信封
                return d["result"], "content"
    except Exception:
        pass
    return txt, "stdout"

target, target_kind = content_aware_target(stdout_txt)

# 从 JSON 信封提取会话 id（如 arkcli 的 resp_xxx），供下轮任务单 session_id 续接
session_id = None
tokens = None
cost_usd = None
try:
    d = json.loads(stdout_txt)
    if isinstance(d, dict):
        if isinstance(d.get("id"), str):
            session_id = d["id"]
        if isinstance(d.get("session_id"), str):   # claude 信封
            session_id = d["session_id"]
        u = d.get("usage")
        if isinstance(u, dict):
            if isinstance(u.get("total_tokens"), int):
                tokens = u["total_tokens"]
            elif u.get("input_tokens") is not None:   # claude usage 口径：in+out+cache_read
                tokens = (u.get("input_tokens") or 0) + (u.get("output_tokens") or 0) \
                         + (u.get("cache_read_input_tokens") or 0)
        if isinstance(d.get("total_cost_usd"), (int, float)):
            cost_usd = d["total_cost_usd"]
except Exception:
    pass

# 非信封类 worker 的会话 id 提取（codex=rollout 文件名 / opencode+hermes=sessions list 最新行）
if not session_id:
    w = os.environ.get("T_WORKER", "")
    try:
        import glob as _glob
        import subprocess as _sp
        if w == "codex":
            import datetime as _dt
            files = _glob.glob(os.path.expanduser("~/.codex/sessions/**/rollout-*.jsonl"), recursive=True)
            # 文件名内嵌创建时间戳（本地时区）：rollout-YYYY-MM-DDTHH-MM-SS-<uuid>[_<fork-uuid>].jsonl
            # 按"文件名时间戳落在本轮窗口"过滤，避免误选其他并发活跃 session（mtime 不可靠）
            start_local = _dt.datetime.fromisoformat(os.environ["START_ISO"].replace("Z", "+00:00")).astimezone().timestamp()
            best = None
            for f in files:
                m = re.search(r"rollout-(\d{4})-(\d{2})-(\d{2})T(\d{2})-(\d{2})-(\d{2})-.+\.jsonl$", f)
                if not m:
                    continue
                ts = _dt.datetime(*[int(m.group(i)) for i in range(1, 7)]).timestamp()
                if ts >= start_local - 10 and (best is None or ts > best[0]):
                    best = (ts, f)
            if best:
                m2 = re.search(r"[-_]([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\.jsonl$", best[1])
                session_id = m2.group(1) if m2 else None
        elif w == "opencode":
            _out = _sp.run(["opencode", "session", "list"], capture_output=True, text=True, timeout=15).stdout
            rows = [l for l in _out.strip().split("\n") if l.strip()]
            if len(rows) >= 3:
                session_id = rows[2].split()[0]
        elif w == "hermes":
            _out = _sp.run(["hermes", "sessions", "list"], capture_output=True, text=True, timeout=15).stdout
            rows = [l for l in _out.strip().split("\n") if l.strip()]
            if len(rows) >= 3:
                session_id = rows[2].lstrip("- ").split()[-1]
        elif w == "dsh":
            # dsh headless 首轮新建 session：~/.dsh/sessions/<ws-slug>/<uuid>，取本轮开始后新建的最新目录
            import datetime as _dt
            start_ts = _dt.datetime.fromisoformat(os.environ["START_ISO"].replace("Z", "+00:00")).timestamp()
            base = os.path.expanduser("~/.dsh/sessions")
            cands = []
            if os.path.isdir(base):
                for slug in os.listdir(base):
                    sd = os.path.join(base, slug)
                    if not os.path.isdir(sd):
                        continue
                    for sid in os.listdir(sd):
                        p = os.path.join(sd, sid)
                        if os.path.isdir(p):
                            mt = os.path.getmtime(p)
                            if mt >= start_ts - 10:
                                cands.append((mt, sid))
            if cands:
                session_id = max(cands)[1]
    except Exception:
        pass

# tokens 采集（带外通道；口径=本次 run 总消耗；各家原生统计，无则 None）
if tokens is None:
    try:
        _stderr_txt = open(os.path.join(out, "stderr.txt"), errors="replace").read()
        if w == "codex":
            # codex exec 结尾在 stderr 打印 "tokens used\n  N,NNN"
            m = re.search(r"tokens used\s*\n\s*([\d,]+)", _stderr_txt)
            if m:
                tokens = int(m.group(1).replace(",", ""))
        elif w == "dsh" and session_id:
            # session 事件流（zstd）最后一条 assistant/message 的 usage.totalTokens（turn 末累计）
            # dsh 退出后文件可能仍在异步 flush：校验 returncode，未读到 usage 则间隔重试
            import time as _time
            for _attempt in range(3):
                for slug in os.listdir(os.path.expanduser("~/.dsh/sessions")):
                    sd = os.path.expanduser(f"~/.dsh/sessions/{slug}/{session_id}/session.v4.jsonl.zstd")
                    if not os.path.exists(sd):
                        continue
                    _rz = _sp.run(["zstd", "-dc", sd], capture_output=True, text=True, timeout=30)
                    if _rz.returncode != 0:
                        continue
                    for line in _rz.stdout.strip().split("\n"):
                        try:
                            e = json.loads(line)
                            u = (e.get("data") or {}).get("usage")
                            if e.get("type") == "assistant/message" and isinstance(u, dict) and u.get("totalTokens"):
                                tokens = u["totalTokens"]
                        except Exception:
                            continue
                    break
                if tokens:
                    break
                _time.sleep(1)
        elif w == "hermes" and session_id:
            # hermes 会话库逐消息 input/output 求和
            _out = _sp.run(["hermes", "sessions", "export", "--format", "jsonl", "--session-id", session_id, "-"],
                           capture_output=True, text=True, timeout=30).stdout
            t_in = t_out = 0
            for line in _out.strip().split("\n"):
                try:
                    e = json.loads(line)
                    t_in += e.get("input_tokens") or 0
                    t_out += e.get("output_tokens") or 0
                except Exception:
                    continue
            if t_in or t_out:
                tokens = t_in + t_out
        elif w == "opencode" and session_id:
            # export JSON 最后一条 assistant 的 info.tokens.total（turn 末累计）
            _out = _sp.run(["opencode", "export", session_id], capture_output=True, text=True, timeout=60).stdout
            if "{" in _out:
                _d = json.loads(_out[_out.index("{"):])
                for _m in _d.get("messages", []):
                    _info = _m.get("info") or {}
                    if _info.get("role") == "assistant" and isinstance(_info.get("tokens"), dict):
                        tokens = _info["tokens"].get("total") or tokens
    except Exception:
        pass

if exit_code == 124:
    status = "timeout"
elif exit_code != 0:
    status = "failed"
elif not stdout_txt.strip():
    status = "criteria_failed"
else:
    status = "success"

criteria = None
if regex:
    passed = bool(re.search(regex, target, re.M))
    criteria = {"stdout_regex": regex, "passed": passed, "target": target_kind}
    if status == "success" and not passed:
        status = "criteria_failed"

result = {
    "task_id": os.environ["T_ID"],
    "worker": os.environ["T_WORKER"],
    "status": status,
    "exit_code": exit_code,
    "attempts": int(os.environ.get("ATTEMPTS") or 1),
    "duration_s": int(os.environ["DURATION"]),
    "started_at": os.environ["START_ISO"],
    "finished_at": os.environ["END_ISO"],
    "stdout_path": os.path.join(out, "stdout.txt"),
    "stderr_path": os.path.join(out, "stderr.txt"),
    "answer_preview": target.strip()[:500],
    "criteria": criteria,
    "session_id": session_id,
    "tokens": tokens,
    "cost_usd": cost_usd,
}
with open(os.path.join(out, "result.json"), "w") as f:
    json.dump(result, f, ensure_ascii=False, indent=2)
# 归一化正文：JSON 信封类输出（如 arkcli）提取 content 落 answer.txt，
# 供 pipeline 节点间数据流转使用（避免信封/reasoning 噪音下传）
with open(os.path.join(out, "answer.txt"), "w") as f:
    f.write(target)
print(status)
PYEOF
)

print -r -- "[$STATUS] $T_ID ($T_WORKER) ${DURATION}s exit=$EXIT_CODE -> $OUT_DIR/result.json"

# ---------- ledger（流转图数据源；RUN_ID/SEQ/NODE_LABEL 由 run_pipeline 注入） ----------
LEDGER="$MESH_ROOT/results/ledger.jsonl"
RUN_ID="${RUN_ID:-$T_ID}" SEQ="${SEQ:-1}" NODE_LABEL="${NODE_LABEL:-}" \
LEDGER="$LEDGER" T_ID="$T_ID" T_WORKER="$T_WORKER" T_BRIEF="$T_BRIEF" \
STATUS="$STATUS" DURATION="$DURATION" START_ISO="$START_ISO" OUT_DIR="$OUT_DIR" \
python3 - <<'PYEOF'
import json, os
rec = {
    "ts": os.environ["START_ISO"],
    "run_id": os.environ["RUN_ID"],
    "seq": int(os.environ.get("SEQ") or 1),
    "node_label": os.environ.get("NODE_LABEL") or "",
    "task_id": os.environ["T_ID"],
    "worker": os.environ["T_WORKER"],
    "brief_preview": os.environ["T_BRIEF"][:160],
    "status": os.environ["STATUS"],
    "duration_s": int(os.environ["DURATION"]),
    "result_dir": os.environ["OUT_DIR"],
    "tokens": None,
    "cost_usd": None,
}
try:
    _rj = json.load(open(os.path.join(os.environ["OUT_DIR"], "result.json")))
    rec["tokens"] = _rj.get("tokens")
    rec["cost_usd"] = _rj.get("cost_usd")
except Exception:
    pass
with open(os.environ["LEDGER"], "a") as f:
    f.write(json.dumps(rec, ensure_ascii=False) + "\n")
PYEOF

[[ "$STATUS" == "success" ]] && exit 0 || exit 1
