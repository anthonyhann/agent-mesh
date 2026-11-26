#!/usr/bin/env python3
"""mesh_board.py — agent-mesh 运行看板

汇总 ledger（全部运行记录）+ trend（skills 趋势）+ 在线页 state，
渲染单页看板 HTML，可选 --publish 发布到 workbuddy（原地更新固定页）。

用法:
  mesh_board.py              # 只生成本地 HTML
  mesh_board.py --publish    # 生成并发布/原地更新在线看板
"""
import collections
import datetime
import html
import json
import os
import subprocess
import sys
from pathlib import Path

MESH = Path(os.environ.get("MESH_ROOT", Path(__file__).resolve().parent))
RESULTS = MESH / "results"
LEDGER = RESULTS / "ledger.jsonl"
TREND = RESULTS / "trend.jsonl"
BOARD_STATE = RESULTS / "board-publish-state.json"
RECENT_RUNS = 20


def esc(x):
    return html.escape(str(x))


def load_ledger():
    recs = []
    if LEDGER.exists():
        for line in LEDGER.read_text().strip().split("\n"):
            if line.strip():
                try:
                    recs.append(json.loads(line))
                except Exception:
                    continue
    return recs


def load_trend():
    rows = []
    if TREND.exists():
        for line in TREND.read_text().strip().split("\n"):
            if line.strip():
                try:
                    rows.append(json.loads(line))
                except Exception:
                    continue
    return rows


def main():
    publish = "--publish" in sys.argv
    recs = load_ledger()
    trend = load_trend()

    # 按 run_id 分组（保序，最新在后）
    runs = collections.OrderedDict()
    for r in recs:
        runs.setdefault(r["run_id"], []).append(r)
    run_list = list(runs.items())[-RECENT_RUNS:][::-1]

    total_runs = len(runs)
    total_nodes = len(recs)
    ok_nodes = sum(1 for r in recs if r["status"] == "success")
    total_tokens = sum(r.get("tokens") or 0 for r in recs)
    last_ts = recs[-1]["ts"] if recs else "-"

    # tokens 维度：按 worker 分布 / 按日趋势（最近 7 天）
    worker_tokens, worker_runs = collections.Counter(), collections.Counter()
    for r in recs:
        worker_runs[r["worker"]] += 1
        worker_tokens[r["worker"]] += r.get("tokens") or 0
    max_wt = max(worker_tokens.values()) if worker_tokens else 1
    daily = collections.OrderedDict()
    for r in recs:
        day = (r.get("ts") or "")[:10]
        if day:
            daily[day] = daily.get(day, 0) + (r.get("tokens") or 0)
    days = sorted(daily.keys())[-7:][::-1]
    max_day = max(daily.values()) if daily else 1

    worker_token_rows = "".join(
        f"<tr><td>{esc(w)}</td><td>{worker_runs[w]}</td>"
        f"<td>{worker_tokens[w] or '—'}</td>"
        f"<td style='width:40%'><div style='background:#0a84ff;height:8px;border-radius:4px;"
        f"width:{100 * worker_tokens[w] // max_wt if worker_tokens[w] else 0}%'></div></td></tr>"
        for w in sorted(worker_tokens, key=worker_tokens.get, reverse=True))
    daily_rows = "".join(
        f"<tr><td>{esc(d)}</td><td>{daily[d] or '—'}</td>"
        f"<td style='width:40%'><div style='background:#34c759;height:8px;border-radius:4px;"
        f"width:{100 * daily[d] // max_day if daily[d] else 0}%'></div></td></tr>"
        for d in days)

    run_rows = "".join(
        f"<tr><td>{esc(rid)}</td><td>{len(nodes)}</td>"
        f"<td><span style='color:{'#34c759' if all(n['status']=='success' for n in nodes) else '#ff3b30'}'>"
        f"{sum(1 for n in nodes if n['status']=='success')}/{len(nodes)}</span></td>"
        f"<td>{sum(n['duration_s'] for n in nodes)}s</td>"
        f"<td>{sum(n.get('tokens') or 0 for n in nodes) or '—'}</td>"
        f"<td>{esc(nodes[0].get('ts',''))}</td></tr>"
        for rid, nodes in run_list)

    pages = []
    for sf in sorted(RESULTS.glob("*-publish-state.json")):
        try:
            d = json.loads(sf.read_text())
            pages.append((sf.name.replace("-publish-state.json", ""),
                          d.get("url", ""), d.get("last_run", "")))
        except Exception:
            continue
    page_rows = "".join(
        f"<tr><td>{esc(n)}</td><td><a href='{esc(u)}'>{esc(u)}</a></td><td>{esc(t)}</td></tr>"
        for n, u, t in pages)

    trend_rows = "".join(
        f"<tr><td>{esc(t['ts'])}</td><td>{t['total']}</td><td>{t['dup_pending']}</td>"
        f"<td>{t['con_pending']}</td><td>{t['broken']}</td></tr>"
        for t in trend[-8:][::-1])

    gen_at = datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    doc = f"""<!DOCTYPE html>
<html lang="zh-CN"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>agent-mesh 看板</title>
<style>
 body{{font-family:-apple-system,'SF Pro Text','PingFang SC',sans-serif;background:#f5f5f7;color:#1d1d1f;margin:0}}
 .wrap{{max-width:960px;margin:0 auto;padding:32px 20px 64px}}
 h1{{font-size:26px;margin:0 0 4px}} .sub{{color:#86868b;font-size:13px;margin-bottom:24px}}
 .cards{{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:12px;margin-bottom:24px}}
 .card{{background:#fff;border-radius:14px;padding:16px;box-shadow:0 1px 3px rgba(0,0,0,.06)}}
 .num{{font-size:28px;font-weight:700}} .lbl{{font-size:12px;color:#86868b;margin-top:2px}}
 section{{background:#fff;border-radius:14px;padding:20px;margin-bottom:16px;box-shadow:0 1px 3px rgba(0,0,0,.06)}}
 h2{{font-size:16px;margin:0 0 12px}}
 table{{width:100%;border-collapse:collapse;font-size:13px}}
 th{{text-align:left;color:#86868b;font-weight:500;padding:6px 8px;border-bottom:1px solid #eee}}
 td{{padding:6px 8px;border-bottom:1px solid #f5f5f7}}
 a{{color:#0a84ff;text-decoration:none}} a:hover{{text-decoration:underline}}
 .foot{{text-align:center;color:#aeaeb2;font-size:12px;margin-top:24px}}
</style></head><body><div class="wrap">
<h1>📡 agent-mesh 运行看板</h1>
<div class="sub">生成于 {gen_at} · 最近运行 {last_ts}</div>
<div class="cards">
 <div class="card"><div class="num">{total_runs}</div><div class="lbl">管线/任务运行数</div></div>
 <div class="card"><div class="num">{ok_nodes}/{total_nodes}</div><div class="lbl">节点成功</div></div>
 <div class="card"><div class="num">{total_tokens or '—'}</div><div class="lbl">累计 tokens（五家原生统计·历史部分缺失）</div></div>
 <div class="card"><div class="num">{len(pages)}</div><div class="lbl">在线页</div></div>
</div>
<section><h2>🧵 最近 {len(run_list)} 次运行</h2><table>
<tr><th>run_id</th><th>节点</th><th>成功</th><th>耗时</th><th>tokens</th><th>开始时间</th></tr>
{run_rows or '<tr><td colspan=6>暂无</td></tr>'}</table></section>
<section><h2>🔗 在线页（workbuddy）</h2><table>
<tr><th>页面</th><th>URL</th><th>最近更新</th></tr>
{page_rows or '<tr><td colspan=3>暂无</td></tr>'}</table></section>
<section><h2>💰 Tokens 消耗分布（按 worker）</h2><table>
<tr><th>Worker</th><th>Runs</th><th>Tokens</th><th>占比</th></tr>
{worker_token_rows or '<tr><td colspan=4>暂无</td></tr>'}</table></section>
<section><h2>📅 Tokens 每日消耗（最近 7 天）</h2><table>
<tr><th>日期</th><th>Tokens</th><th>趋势</th></tr>
{daily_rows or '<tr><td colspan=3>暂无</td></tr>'}</table></section>
<section><h2>📈 Skills 趋势（最近 8 次巡检）</h2><table>
<tr><th>时间戳</th><th>总数</th><th>待治理重复</th><th>待规范</th><th>断链</th></tr>
{trend_rows or '<tr><td colspan=5>暂无</td></tr>'}</table></section>
<div class="foot">agent-mesh · mesh_board.py · ledger/trend/state 三源汇总</div>
</div></body></html>"""

    out = RESULTS / "board.html"
    out.write_text(doc)
    print(json.dumps({"ok": True, "board": str(out), "runs": total_runs}, ensure_ascii=False))

    if publish:
        pub = [str(MESH / "publish_result.sh"), str(out), "--name", "agent-mesh 运行看板.html"]
        node_id = None
        if BOARD_STATE.exists():
            try:
                node_id = json.loads(BOARD_STATE.read_text()).get("node_block_id")
            except Exception:
                pass
        if node_id:
            pub += ["--node-block-id", node_id]
        r = subprocess.run(["/bin/zsh"] + pub, capture_output=True, text=True)
        print(r.stdout.strip())
        try:
            res = json.loads(r.stdout.strip().split("\n")[-1])
            if res.get("ok"):
                BOARD_STATE.write_text(json.dumps({
                    "node_block_id": res["node_block_id"], "url": res["url"],
                    "last_run": gen_at}, ensure_ascii=False, indent=2))
        except Exception:
            pass
    return 0


if __name__ == "__main__":
    sys.exit(main())
