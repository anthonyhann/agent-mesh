#!/usr/bin/env python3
"""run_dag.py — DAG 并行派单执行器（与 run_pipeline.sh 的严格串行互补）

schema:
{
  "dag_id": "demo-dag",                  必填，run_id 与结果目录名
  "title": "可选标题",
  "max_parallel": 3,                     可选，默认 3
  "continue_on_error": false,            可选，默认失败即中止后续层
  "nodes": [
    {"seq": 1, "worker": "arkcli", "label": "职责", "brief": "...",
     "depends_on": [2, 3],        可选，缺省依赖上一 seq（首节点无依赖）
     ...其余字段同 run_worker 任务单}
  ]
}

数据流:
  {{prev_output}}        依赖中最大 seq 节点的归一化输出（>4000 字传路径+摘要）
  {{nodes.<seq>.output}} 引用任意已完成节点的输出

产物:
  results/<dag_id>/run_manifest.json   含 layers 分层信息（render_flow 分层渲染）
  results/<dag_id>/flow.html
退出码: 全部 success=0；任一 failed/skipped=1
"""
import json
import os
import re
import subprocess
import sys
import threading
from pathlib import Path

MESH = Path(os.environ.get("MESH_ROOT", Path(__file__).resolve().parent))
RUN_WORKER = MESH / "run_worker.sh"
RENDER = MESH / "render_flow.py"


def main():
    dag_file = Path(sys.argv[1]).resolve()
    dag = json.loads(dag_file.read_text())
    dag_id = dag["dag_id"]
    run_dir = MESH / "results" / dag_id
    run_dir.mkdir(parents=True, exist_ok=True)
    max_par = int(dag.get("max_parallel", 3))
    coe = bool(dag.get("continue_on_error", False))

    specs = dag["nodes"]
    seqs = sorted(n["seq"] for n in specs)
    nodes = {}
    for n in specs:
        seq = n["seq"]
        deps = n.get("depends_on")
        if deps is None:
            deps = [seq - 1] if (seq - 1) in seqs else []
        nodes[seq] = {"spec": n, "deps": deps, "status": "pending",
                      "output": "", "answer_path": "", "rc": None}

    # 拓扑分层（Kahn）
    layers, assigned = [], set()
    remaining = set(nodes)
    while remaining:
        layer = sorted(s for s in remaining
                       if all(d in assigned or d not in nodes for d in nodes[s]["deps"]))
        if not layer:
            print(json.dumps({"ok": False, "error": "dependency cycle detected"}))
            return 2
        layers.append(layer)
        assigned.update(layer)
        remaining -= set(layer)

    def render_brief(seq):
        spec = nodes[seq]["spec"]
        brief = spec["brief"]
        for m in re.finditer(r"\{\{nodes\.(\d+)\.output\}\}", brief):
            ref = int(m.group(1))
            brief = brief.replace(m.group(0), nodes.get(ref, {}).get("output") or "(无)")
        deps = [d for d in nodes[seq]["deps"] if d in nodes]
        prev, prev_path = "", ""
        if deps:
            latest = max(deps)
            prev = nodes[latest]["output"]
            prev_path = nodes[latest]["answer_path"]
        if prev and len(prev) > 4000:
            prev = (f"[上游输出共 {len(prev)} 字符，完整内容见文件 {prev_path}"
                    f"（若你具备文件读取能力请直接读取全文）]\n\n前 2000 字符摘要:\n{prev[:2000]}")
        return brief.replace("{{prev_output}}", prev or "(无上游输出)")

    def run_node(seq):
        spec = nodes[seq]["spec"]
        task = {k: v for k, v in spec.items() if k not in ("seq", "label", "depends_on")}
        task.update({"id": f"{dag_id}-n{seq}", "worker": spec["worker"],
                     "brief": render_brief(seq)})
        task_file = run_dir / f".node-{seq}.json"
        task_file.write_text(json.dumps(task, ensure_ascii=False))
        env = dict(os.environ, RUN_ID=dag_id, SEQ=str(seq),
                   NODE_LABEL=spec.get("label", ""))
        r = subprocess.run(["/bin/zsh", str(RUN_WORKER), str(task_file)],
                           env=env, capture_output=True, text=True)
        nodes[seq]["rc"] = r.returncode
        out_dir = MESH / "results" / task["id"]
        ans, out = out_dir / "answer.txt", out_dir / "stdout.txt"
        if ans.exists():
            nodes[seq]["output"] = ans.read_text(errors="replace")
            nodes[seq]["answer_path"] = str(ans)
        elif out.exists():
            nodes[seq]["output"] = out.read_text(errors="replace")
            nodes[seq]["answer_path"] = str(out)
        nodes[seq]["status"] = "success" if r.returncode == 0 else "failed"
        task_file.unlink(missing_ok=True)
        print(f"  [{'✓' if r.returncode == 0 else '✗'}] n{seq} {spec.get('label','')} ({spec['worker']})", flush=True)

    failed_any = False
    aborted = False
    for li, layer in enumerate(layers):
        if aborted:
            for s in layer:
                nodes[s]["status"] = "skipped"
            continue
        runnable = []
        for s in layer:
            dep_failed = any(nodes[d]["status"] in ("failed", "skipped")
                             for d in nodes[s]["deps"] if d in nodes)
            if dep_failed:
                nodes[s]["status"] = "skipped"
                failed_any = True
            else:
                runnable.append(s)
        if not runnable:
            continue
        sem = threading.Semaphore(max_par)

        def worker(s):
            with sem:
                run_node(s)

        threads = [threading.Thread(target=worker, args=(s,)) for s in runnable]
        for t in threads:
            t.start()
        for t in threads:
            t.join()
        if any(nodes[s]["status"] == "failed" for s in runnable):
            failed_any = True
            if not coe:
                aborted = True

    # ---- manifest ----
    recs = []
    for s in sorted(nodes):
        n = nodes[s]
        result_dir = MESH / "results" / f"{dag_id}-n{s}"
        status = n["status"]
        if status == "skipped":
            duration, ts = 0, ""
        else:
            rj = result_dir / "result.json"
            if rj.exists():
                d = json.loads(rj.read_text())
                status = d["status"]
                duration = d["duration_s"]
                ts = d["started_at"]
            else:
                duration, ts = 0, ""
        recs.append({
            "seq": s, "node_label": n["spec"].get("label", ""),
            "task_id": f"{dag_id}-n{s}", "worker": n["spec"]["worker"],
            "brief_preview": n["spec"]["brief"][:160],
            "status": status, "duration_s": duration, "ts": ts,
            "result_dir": str(result_dir), "depends_on": nodes[s]["deps"],
        })
    ok = sum(1 for r in recs if r["status"] == "success")
    manifest = {
        "run_id": dag_id,
        "title": dag.get("title") or dag_id,
        "pipeline_file": str(dag_file),
        "nodes": recs,
        "layers": layers,
        "edges": [[d, r["seq"]] for r in recs for d in r["depends_on"]],
        "summary": {"total": len(recs), "success": ok,
                    "failed": len(recs) - ok,
                    "duration_s": sum(r["duration_s"] for r in recs)},
    }
    manifest_path = run_dir / "run_manifest.json"
    manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2))

    flow_path = run_dir / "flow.html"
    subprocess.run([sys.executable, str(RENDER), str(manifest_path), "--out", str(flow_path)],
                   capture_output=True)
    # 刷新本地看板（不发布；失败不阻塞）
    try:
        subprocess.run([sys.executable, str(MESH / "mesh_board.py")],
                       capture_output=True, timeout=60)
    except Exception:
        pass
    print(json.dumps({"ok": not failed_any, "manifest": str(manifest_path),
                      "flow": str(flow_path), "summary": manifest["summary"]},
                     ensure_ascii=False))
    return 1 if failed_any else 0


if __name__ == "__main__":
    sys.exit(main())
