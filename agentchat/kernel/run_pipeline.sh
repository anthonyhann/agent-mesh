#!/bin/zsh
# run_pipeline.sh — 多节点派单管线：顺序执行 nodes，数据随节点流转
#
# 用法: run_pipeline.sh <pipeline.json>
#
# pipeline schema:
# {
#   "pipeline_id": "demo-001",            必填，run_id 与结果目录名
#   "continue_on_error": false,           可选，默认失败即中止
#   "nodes": [
#     {
#       "seq": 1,                          必填，执行顺序
#       "worker": "arkcli",                必填
#       "label": "职责说明（流转图显示）",    建议
#       "brief": "任务 prompt；{{prev_output}} 会被替换为上一节点 stdout（截 4000 字）",
#       其余字段同 run_worker 任务单（model/profile/timeout_seconds/done_criteria...）
#     }
#   ]
# }
#
# 产物:
#   results/<pipeline_id>/run_manifest.json   节点状态汇总（流转图数据源）
#   results/<pipeline_id>/flow.html           流转图
#   results/ledger.jsonl                      追加每个节点记录
# 退出码: 全部 success=0，任一失败=1

set -uo pipefail

MESH_ROOT="${MESH_ROOT:-$(cd "$(dirname "${0:A}")" && pwd)}"
die() { print -r -- "run_pipeline: $*" >&2; exit 2; }

[[ $# -eq 1 && -f "$1" ]] || die "usage: run_pipeline.sh <pipeline.json>"
PIPE_FILE="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
command -v python3 >/dev/null || die "python3 not found"

PIPE_ID=$(python3 -c "import json,sys; print(json.load(open('$PIPE_FILE'))['pipeline_id'])")
RUN_DIR="$MESH_ROOT/results/$PIPE_ID"
mkdir -p "$RUN_DIR"

CONTINUE_ON_ERROR=$(python3 -c "import json; print('1' if json.load(open('$PIPE_FILE')).get('continue_on_error') else '')")
NODE_COUNT=$(python3 -c "import json; print(len(json.load(open('$PIPE_FILE'))['nodes']))")

PREV_OUTPUT=""
PREV_PATH=""
OVERALL_FAIL=0
SEQ=0

while [[ $SEQ -lt $NODE_COUNT ]]; do
  SEQ=$((SEQ + 1))
  # 生成该节点临时任务单（注入 prev_output 渲染；超长输出改传文件路径+摘要，防截断）
  NODE_TASK=$(PREV_OUTPUT="$PREV_OUTPUT" PREV_PATH="$PREV_PATH" PIPE_FILE="$PIPE_FILE" RUN_DIR="$RUN_DIR" SEQ="$SEQ" python3 - <<'PYEOF'
import json, os, sys
pipe = json.load(open(os.environ["PIPE_FILE"]))
node = pipe["nodes"][int(os.environ["SEQ"]) - 1]
prev = os.environ.get("PREV_OUTPUT", "")
prev_path = os.environ.get("PREV_PATH", "")
if prev and len(prev) > 4000:
    replacement = (f"[上游输出共 {len(prev)} 字符，完整内容见文件 {prev_path}"
                   f"（若你具备文件读取能力请直接读取全文）]\n\n前 2000 字符摘要:\n{prev[:2000]}")
elif prev:
    replacement = prev
else:
    replacement = "(无上游输出)"
brief = node["brief"].replace("{{prev_output}}", replacement)
task = {
    "id": f"{pipe['pipeline_id']}-n{node.get('seq', os.environ['SEQ'])}",
    "worker": node["worker"],
    "brief": brief,
}
for k in ("workspace", "timeout_seconds", "model", "session_id",
          "permission_mode", "profile", "max_output_tokens", "done_criteria"):
    if k in node:
        task[k] = node[k]
out = os.path.join(os.environ["RUN_DIR"], f".node-{os.environ['SEQ']}.json")
json.dump(task, open(out, "w"), ensure_ascii=False)
print(out)
PYEOF
) || die "failed to build node $SEQ task"

  NODE_LABEL=$(python3 -c "import json; print(json.load(open('$PIPE_FILE'))['nodes'][$SEQ-1].get('label',''))")
  TASK_ID=$(python3 -c "import json; print(json.load(open('$NODE_TASK'))['id'])")

  print -r -- "── node $SEQ/$NODE_COUNT [$NODE_LABEL] → $TASK_ID"
  RUN_ID="$PIPE_ID" SEQ="$SEQ" NODE_LABEL="$NODE_LABEL" \
    "$MESH_ROOT/run_worker.sh" "$NODE_TASK"
  NODE_RC=$?

  # 收集本节点归一化输出供下一节点消费（answer.txt 优先，防 JSON 信封噪音下传）
  NODE_OUT_DIR="$MESH_ROOT/results/$TASK_ID"
  if [[ -f "$NODE_OUT_DIR/answer.txt" ]]; then
    PREV_OUTPUT=$(cat "$NODE_OUT_DIR/answer.txt")
    PREV_PATH="$NODE_OUT_DIR/answer.txt"
  elif [[ -f "$NODE_OUT_DIR/stdout.txt" ]]; then
    PREV_OUTPUT=$(cat "$NODE_OUT_DIR/stdout.txt")
    PREV_PATH="$NODE_OUT_DIR/stdout.txt"
  else
    PREV_OUTPUT=""
    PREV_PATH=""
  fi

  if [[ $NODE_RC -ne 0 ]]; then
    OVERALL_FAIL=1
    [[ -z "$CONTINUE_ON_ERROR" ]] && break
  fi
done

# 汇总 run manifest（从 ledger 提取本 run 记录）
python3 - "$MESH_ROOT/results/ledger.jsonl" "$PIPE_ID" "$RUN_DIR/run_manifest.json" "$PIPE_FILE" <<'PYEOF'
import json, sys
ledger_path, run_id, out_path, pipe_path = sys.argv[1:5]
pipe = json.load(open(pipe_path))
recs = []
try:
    with open(ledger_path) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            r = json.loads(line)
            if r.get("run_id") == run_id:
                recs.append(r)
except FileNotFoundError:
    pass
recs.sort(key=lambda r: r.get("seq", 0))
ok = sum(1 for r in recs if r["status"] == "success")
manifest = {
    "run_id": run_id,
    "pipeline_file": pipe_path,
    "title": pipe.get("title") or run_id,
    "nodes": recs,
    "edges": [[recs[i]["seq"], recs[i+1]["seq"]] for i in range(len(recs)-1)],
    "summary": {"total": len(recs), "success": ok,
                "failed": len(recs) - ok,
                "duration_s": sum(r["duration_s"] for r in recs)},
}
json.dump(manifest, open(out_path, "w"), ensure_ascii=False, indent=2)
print(out_path)
PYEOF

# 渲染流转图
python3 "$MESH_ROOT/render_flow.py" "$RUN_DIR/run_manifest.json" --out "$RUN_DIR/flow.html" \
  && print -r -- "flow: $RUN_DIR/flow.html"

# 刷新本地看板（低成本纯渲染；在线看板由 automation 周更或手动 publish）
python3 "$MESH_ROOT/mesh_board.py" >/dev/null 2>&1 || true

rm -f "$RUN_DIR"/.node-*.json
[[ $OVERALL_FAIL -eq 0 ]] && exit 0 || exit 1
