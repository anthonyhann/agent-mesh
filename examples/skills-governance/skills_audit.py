#!/usr/bin/env python3
"""skills_audit.py — 本机多智能体 skills 安装巡检（只读，零副作用）

扫描 10 个 agent 的 skills 目录，分类 实体复制 vs 符号链接，找出：
  1. duplicates     同名 skill 被多个 agent 各存一份实体（最浪费）
  2. consolidatable 实体 skill 在中央库 ~/.agents/skills 已有同名 → 可换 symlink
  3. broken_links   断链 symlink
  4. source_stats   symlink 源分布（central / ai-labs / other）

输出: results/skills-audit-<ts>/audit.json + report.html
退出码: 恒 0（巡检本身失败才非 0）
"""
import datetime
import html
import json
import os
import sys
from pathlib import Path

HOME = Path.home()
from mesh_config import AGENT_DIRS, CENTRAL, FORK_REGISTRY, MESH_ROOT


def classify_source(resolved: str) -> str:
    if str(CENTRAL) in resolved:
        return "central"
    if "ai-labs" in resolved:
        return "ai-labs"
    return "other"


def scan():
    inventory = {}          # name -> [loc, ...]
    agent_summary = {}
    for agent, d in AGENT_DIRS.items():
        if not d.is_dir():
            continue
        stats = {"entity": 0, "symlink": 0}
        for entry in sorted(d.iterdir()):
            name = entry.name
            if entry.is_symlink():
                raw = os.readlink(entry)
                resolved = (entry.parent / raw).resolve() if not os.path.isabs(raw) else Path(raw)
                loc = {
                    "agent": agent,
                    "type": "symlink",
                    "target": str(resolved),
                    "exists": resolved.exists(),
                    "source": classify_source(str(resolved)),
                }
                stats["symlink"] += 1
            elif entry.is_dir():
                loc = {"agent": agent, "type": "entity"}
                stats["entity"] += 1
            else:
                continue  # 普通文件（skills-lock.json 等）不纳入 skill 计数
            inventory.setdefault(name, []).append(loc)
        agent_summary[agent] = stats
    return inventory, agent_summary


def analyze(inventory):
    central_names = {p.name for p in CENTRAL.iterdir() if p.is_dir()} if CENTRAL.is_dir() else set()
    forks = set()
    if FORK_REGISTRY.exists():
        try:
            forks = set(json.loads(FORK_REGISTRY.read_text()).get("forks", {}))
        except Exception:
            pass

    duplicates, consolidatable, broken, source_stats = {}, [], [], {}
    for name, locs in sorted(inventory.items()):
        entities = [l for l in locs if l["type"] == "entity"]
        symlinks = [l for l in locs if l["type"] == "symlink"]

        if len(entities) > 1:
            duplicates[name] = {
                "agents": [l["agent"] for l in entities],
                "fork_status": "approved" if name in forks else "pending",
            }

        if entities and name in central_names:
            consolidatable.append({
                "skill": name,
                "entity_agents": [l["agent"] for l in entities],
                "linked_agents": [l["agent"] for l in symlinks],
                "fork_status": "approved" if name in forks else "pending",
            })

        for l in symlinks:
            if not l["exists"]:
                broken.append({"skill": name, "agent": l["agent"], "target": l["target"]})
            source_stats[l["source"]] = source_stats.get(l["source"], 0) + 1

    return {
        "duplicates": duplicates,
        "forks_registered": len(forks),
        "consolidatable": consolidatable,
        "broken_links": broken,
        "source_stats": source_stats,
        "central_library": {"path": str(CENTRAL), "skill_count": len(central_names)},
    }


def render_html(report: dict, generated_at: str, delta_text: str = "") -> str:
    a = report["analysis"]
    s = report["agent_summary"]

    def esc(x):
        return html.escape(str(x))

    agent_rows = "".join(
        f"<tr><td>{esc(k)}</td><td>{v['entity']}</td><td>{v['symlink']}</td></tr>"
        for k, v in s.items())

    dup_rows = "".join(
        f"<tr><td>{esc(n)}</td><td>{esc('、'.join(v['agents']))}</td><td>{len(v['agents'])} 份</td>"
        f"<td>{'✅ 已审定分叉' if v['fork_status'] == 'approved' else '⚠️ 待治理'}</td></tr>"
        for n, v in a["duplicates"].items()) or "<tr><td colspan=4>无 🎉</td></tr>"

    con_rows = "".join(
        f"<tr><td>{esc(c['skill'])}</td><td>{esc('、'.join(c['entity_agents']))}</td>"
        f"<td>{'✅ 已审定分叉保留' if c['fork_status'] == 'approved' else '中央库已有同名 → 删实体建 symlink'}</td></tr>"
        for c in a["consolidatable"]) or "<tr><td colspan=3>无 🎉</td></tr>"

    broken_rows = "".join(
        f"<tr><td>{esc(b['skill'])}</td><td>{esc(b['agent'])}</td><td><code>{esc(b['target'])}</code></td></tr>"
        for b in a["broken_links"]) or "<tr><td colspan=3>无 🎉</td></tr>"

    src = a["source_stats"]
    dup_count = len(a["duplicates"])
    dup_pending = sum(1 for v in a["duplicates"].values() if v["fork_status"] == "pending")
    dup_approved = dup_count - dup_pending
    con_count = len(a["consolidatable"])
    con_pending = sum(1 for c in a["consolidatable"] if c["fork_status"] == "pending")
    broken_count = len(a["broken_links"])
    total_entities = sum(v["entity"] for v in s.values())
    total_links = sum(v["symlink"] for v in s.values())

    return f"""<!DOCTYPE html>
<html lang="zh-CN"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Skills 巡检报告 · {generated_at}</title>
<style>
  body{{font-family:-apple-system,'SF Pro Text','PingFang SC',sans-serif;margin:0;background:#f5f5f7;color:#1d1d1f}}
  .wrap{{max-width:860px;margin:0 auto;padding:32px 20px 64px}}
  h1{{font-size:28px;margin:0 0 4px}}
  .sub{{color:#86868b;font-size:13px;margin-bottom:24px}}
  .cards{{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:12px;margin-bottom:28px}}
  .card{{background:#fff;border-radius:14px;padding:16px;box-shadow:0 1px 3px rgba(0,0,0,.06)}}
  .card .num{{font-size:30px;font-weight:700}} .card .lbl{{font-size:12px;color:#86868b;margin-top:2px}}
  .warn .num{{color:#ff9500}} .bad .num{{color:#ff3b30}} .ok .num{{color:#34c759}}
  section{{background:#fff;border-radius:14px;padding:20px;margin-bottom:16px;box-shadow:0 1px 3px rgba(0,0,0,.06)}}
  h2{{font-size:17px;margin:0 0 12px}}
  table{{width:100%;border-collapse:collapse;font-size:13px}}
  th{{text-align:left;color:#86868b;font-weight:500;padding:6px 8px;border-bottom:1px solid #eee}}
  td{{padding:6px 8px;border-bottom:1px solid #f5f5f7;vertical-align:top}}
  code{{font-family:'SF Mono',Menlo,monospace;font-size:12px;background:#f5f5f7;padding:1px 5px;border-radius:5px;word-break:break-all}}
  .foot{{text-align:center;color:#aeaeb2;font-size:12px;margin-top:24px}}
</style></head><body><div class="wrap">
<h1>🛰️ Skills 巡检报告</h1>
<div class="sub">生成时间 {generated_at} · 只读扫描，未改动任何文件 · 中央库 <code>{esc(a['central_library']['path'])}</code>（{a['central_library']['skill_count']} 个 skill）{delta_text}</div>
<div class="cards">
  <div class="card"><div class="num">{total_entities + total_links}</div><div class="lbl">安装点总数</div></div>
  <div class="card"><div class="num">{total_links}</div><div class="lbl">已共享 symlink</div></div>
  <div class="card warn"><div class="num">{dup_pending}</div><div class="lbl">待治理重复（另有 {dup_approved} 组已审定分叉）</div></div>
  <div class="card warn"><div class="num">{con_pending}</div><div class="lbl">可规范到中央库（另有 {con_count - con_pending} 组已审定）</div></div>
  <div class="card {'bad' if broken_count else 'ok'}"><div class="num">{broken_count}</div><div class="lbl">断链</div></div>
</div>
<section><h2>📊 各智能体安装方式</h2><table>
<tr><th>智能体</th><th>实体复制</th><th>符号链接</th></tr>{agent_rows}</table>
<p style="font-size:12px;color:#86868b">symlink 源分布：central {src.get('central',0)} · ai-labs {src.get('ai-labs',0)} · 其他 {src.get('other',0)}</p></section>
<section><h2>🔁 重复实体（已审定分叉保留，待治理项才需动作）</h2><table>
<tr><th>Skill</th><th>实体存在于</th><th>份数</th><th>状态</th></tr>{dup_rows}</table></section>
<section><h2>🎯 实体 vs 中央库（待规范项 + 已审定分叉）</h2><table>
<tr><th>Skill</th><th>实体存在于</th><th>状态 / 建议动作</th></tr>{con_rows}</table></section>
<section><h2>⛓️‍💥 断链 symlink</h2><table>
<tr><th>Skill</th><th>智能体</th><th>失效目标</th></tr>{broken_rows}</table></section>
<div class="foot">agent-mesh · skills_audit.py · catdesk automation 定时巡检</div>
</div></body></html>"""


def main():
    generated_at = datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    ts = datetime.datetime.now().strftime("%Y%m%d-%H%M%S")
    out_dir = MESH_ROOT / "results" / f"skills-audit-{ts}"
    out_dir.mkdir(parents=True, exist_ok=True)

    inventory, agent_summary = scan()
    report = {
        "generated_at": generated_at,
        "agent_summary": agent_summary,
        "analysis": analyze(inventory),
        "inventory_size": len(inventory),
    }

    (out_dir / "audit.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2))

    # ---- trend：追加本次指标，与上次对比生成 delta 文本 ----
    a = report["analysis"]
    trend_file = MESH_ROOT / "results" / "trend.jsonl"
    cur = {
        "ts": ts,
        "total": len(inventory),
        "dup_pending": sum(1 for v in a["duplicates"].values() if v["fork_status"] == "pending"),
        "con_pending": sum(1 for c in a["consolidatable"] if c["fork_status"] == "pending"),
        "broken": len(a["broken_links"]),
    }
    prev = None
    if trend_file.exists():
        for line in trend_file.read_text().strip().split("\n")[::-1]:
            if line.strip():
                try:
                    prev = json.loads(line)
                    break
                except Exception:
                    continue
    with open(trend_file, "a") as f:
        f.write(json.dumps(cur, ensure_ascii=False) + "\n")
    delta_text = ""
    if prev:
        def d(key, label):
            diff = cur[key] - prev.get(key, 0)
            return f"{label}{'+' if diff > 0 else ''}{diff}" if diff else None
        parts = [x for x in (d("dup_pending", "待治理重复"), d("con_pending", "待规范"), d("broken", "断链")) if x]
        delta_text = " · 较上次: " + ("、".join(parts) if parts else "无变化")

    (out_dir / "report.html").write_text(
        render_html(report, generated_at, delta_text))

    a = report["analysis"]
    print(json.dumps({
        "out_dir": str(out_dir),
        "total_skills": len(inventory),
        "duplicates": len(a["duplicates"]),
        "consolidatable": len(a["consolidatable"]),
        "broken_links": len(a["broken_links"]),
        "report_html": str(out_dir / "report.html"),
    }, ensure_ascii=False))


if __name__ == "__main__":
    sys.exit(main())
