#!/usr/bin/env python3
"""render_flow.py — run manifest → HTML Agent 工作流转图

用法:
  render_flow.py <run_manifest.json> [--out flow.html]

节点卡片含：worker 徽标（按 harness 配色）、职责 label、brief 摘要、
状态（success/failed/timeout/criteria_failed 配色）、耗时、产物折叠区。
"""
import html
import json
import sys
from pathlib import Path

WORKER_STYLE = {
    "dsh":      ("#7c3aed", "DSH"),
    "codex":    ("#10a37f", "Codex"),
    "opencode": ("#0ea5e9", "OpenCode"),
    "hermes":   ("#f59e0b", "Hermes"),
    "arkcli":   ("#ff6a00", "Ark·豆包"),
    "claude":   ("#d97706", "Claude"),
    "catdesk":  ("#6e6e73", "CatDesk"),
}
STATUS_STYLE = {
    "success":         ("#34c759", "✓ 成功"),
    "failed":          ("#ff3b30", "✗ 失败"),
    "timeout":         ("#ff9500", "⏱ 超时"),
    "criteria_failed": ("#ffcc00", "⚠ 验收未过"),
    "skipped":         ("#aeaeb2", "⤼ 跳过"),
}


def esc(x):
    return html.escape(str(x))


def render(manifest: dict) -> str:
    nodes = manifest["nodes"]
    summ = manifest["summary"]
    title = manifest.get("title") or manifest["run_id"]
    layers = manifest.get("layers")

    def node_card(n):
        wcolor, wname = WORKER_STYLE.get(n["worker"], ("#8e8e93", n["worker"]))
        scolor, sname = STATUS_STYLE.get(n["status"], ("#8e8e93", n["status"]))
        label = n.get("node_label") or n["task_id"]
        preview_html = ""
        rj = Path(n["result_dir"]) / "result.json"
        if rj.exists():
            try:
                preview = json.loads(rj.read_text()).get("answer_preview", "")
                if preview:
                    preview_html = (
                        f"<details><summary>产出预览</summary>"
                        f"<pre>{esc(preview[:600])}</pre></details>")
            except Exception:
                pass
        dim = "opacity:.55" if n["status"] == "skipped" else ""
        return f"""
      <div class="node" style="border-top-color:{wcolor};{dim}">
        <div class="top"><span class="badge" style="background:{wcolor}">{esc(wname)}</span>
          <span class="seq">#{n.get('seq','?')}</span></div>
        <div class="label">{esc(label)}</div>
        <div class="brief">{esc(n.get('brief_preview',''))}</div>
        <div class="meta"><span class="status" style="color:{scolor}">●</span>
          {esc(sname)} · {n['duration_s']}s</div>
        <div class="ts">{esc(n.get('ts',''))}</div>
        {preview_html}
      </div>"""

    if layers:
        # DAG 分层：每层一行（层内并行节点并排），层间向下箭头
        seq_to_node = {n["seq"]: n for n in nodes}
        flow_parts = []
        for li, layer in enumerate(layers):
            cards = "".join(node_card(seq_to_node[s]) for s in layer if s in seq_to_node)
            flow_parts.append(f'<div class="layer">{cards}</div>')
            if li < len(layers) - 1:
                flow_parts.append('<div class="varrow">↓</div>')
        flow = "\n".join(flow_parts)
    else:
        # 串行横向流（兼容原 pipeline manifest）
        cards = []
        for i, n in enumerate(nodes):
            cards.append(node_card(n))
            if i < len(nodes) - 1:
                cards.append('<div class="arrow">➜</div>')
        flow = f'<div class="flow">{chr(10).join(cards)}</div>'
    return f"""<!DOCTYPE html>
<html lang="zh-CN"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Agent 流转图 · {esc(title)}</title>
<style>
  body{{font-family:-apple-system,'SF Pro Text','PingFang SC',sans-serif;background:#f5f5f7;color:#1d1d1f;margin:0}}
  .wrap{{max-width:1080px;margin:0 auto;padding:32px 20px 64px}}
  h1{{font-size:24px;margin:0 0 4px}}
  .sub{{color:#86868b;font-size:13px;margin-bottom:24px}}
  .summary{{display:flex;gap:12px;margin-bottom:24px;flex-wrap:wrap}}
  .chip{{background:#fff;border-radius:10px;padding:8px 14px;font-size:13px;box-shadow:0 1px 3px rgba(0,0,0,.06)}}
  .flow{{display:flex;align-items:stretch;gap:0;flex-wrap:wrap;row-gap:28px}}
  .layer{{display:flex;gap:14px;flex-wrap:wrap;justify-content:center}}
  .varrow{{text-align:center;font-size:24px;color:#c7c7cc;line-height:1;padding:6px 0}}
  .node{{background:#fff;border-radius:14px;padding:16px;width:240px;box-shadow:0 1px 4px rgba(0,0,0,.08);border-top:4px solid #ccc}}
  .top{{display:flex;justify-content:space-between;align-items:center;margin-bottom:8px}}
  .badge{{color:#fff;font-size:11px;font-weight:600;padding:3px 10px;border-radius:99px}}
  .seq{{color:#aeaeb2;font-size:12px}}
  .label{{font-weight:600;font-size:15px;margin-bottom:6px}}
  .brief{{font-size:12px;color:#6e6e73;line-height:1.5;margin-bottom:10px;word-break:break-all}}
  .meta{{font-size:13px;margin-bottom:4px}}
  .ts{{font-size:11px;color:#aeaeb2}}
  .arrow{{display:flex;align-items:center;justify-content:center;font-size:22px;color:#c7c7cc;padding:0 10px}}
  details{{margin-top:10px;font-size:12px}}
  summary{{cursor:pointer;color:#0a84ff}}
  pre{{background:#f5f5f7;border-radius:8px;padding:10px;font-size:11px;overflow:auto;max-height:220px;white-space:pre-wrap;word-break:break-all}}
  .foot{{text-align:center;color:#aeaeb2;font-size:12px;margin-top:32px}}
</style></head><body><div class="wrap">
<h1>🗺️ Agent 工作流转图</h1>
<div class="sub">run: {esc(manifest['run_id'])}</div>
<div class="summary">
  <div class="chip">节点 <b>{summ['total']}</b></div>
  <div class="chip">成功 <b style="color:#34c759">{summ['success']}</b></div>
  <div class="chip">失败 <b style="color:#ff3b30">{summ['failed']}</b></div>
  <div class="chip">总耗时 <b>{summ['duration_s']}s</b></div>
</div>
<div class="flow-wrap">{flow}
</div>
<div class="foot">agent-mesh · render_flow.py · 每个节点为独立 fresh context</div>
</div></body></html>"""


def main():
    if len(sys.argv) < 2:
        print("usage: render_flow.py <run_manifest.json> [--out flow.html]", file=sys.stderr)
        return 2
    manifest = json.loads(Path(sys.argv[1]).read_text())
    out = Path("flow.html")
    if "--out" in sys.argv:
        out = Path(sys.argv[sys.argv.index("--out") + 1])
    out.write_text(render(manifest))
    print(str(out))
    return 0


if __name__ == "__main__":
    sys.exit(main())
