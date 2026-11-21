#!/usr/bin/env python3
"""gen_llm_brief.py — 为 needs_llm 重复组生成 LLM 分类用 diff 摘要

每组产出紧凑证据：代表副本对（最新 vs 对比方，含中央库）、
文件集合差异（only-in-A / only-in-B 前 10）、SKILL.md frontmatter 摘要对比、
大小与 mtime。输出单个 Markdown 文件供 LLM 批量分类。

用法: gen_llm_brief.py <replicas-analyze-*.json>
"""
import json
import sys
from pathlib import Path

HOME = Path.home()
CENTRAL = HOME / ".agents/skills"


def file_set(d: Path):
    out = set()
    for p in d.rglob("*"):
        if p.name == ".DS_Store":
            continue
        if p.is_file() or p.is_symlink():
            out.add(str(p.relative_to(d)))
    return out


def fm_summary(d: Path):
    f = d / "SKILL.md"
    if not f.exists():
        return "(无 SKILL.md)"
    txt = f.read_text(errors="replace")
    # 抽 frontmatter 或开头
    head = txt[:800]
    return head.replace("\n", " ")[:350]


def fmt_bytes(n):
    return f"{n/1024:.0f}K" if n >= 1024 else f"{n}B"


def main():
    data = json.loads(Path(sys.argv[1]).read_text())
    groups = [g for g in data["groups"] if g.get("needs_llm") and g["hash_variants"] > 1]

    parts = [f"# 重复 skills 分类输入（{len(groups)} 组）\n"]
    parts.append("对每组给出决策：sync_central（功能等价，用最新版统一收编中央库）/ "
                 "keep_fork（agent 特定定制，保留分叉）/ merge（各有独有内容，需人工合并）。\n")

    for g in groups:
        info = g["info"]
        newest = g["newest_copy"]
        others = [a for a in g["agents"] if a != newest]
        peer = others[0]
        a_path = Path(info[newest]["path"])
        b_path = Path(info[peer]["path"])

        fa, fb = file_set(a_path), file_set(b_path)
        only_a = sorted(fa - fb)[:10]
        only_b = sorted(fb - fa)[:10]

        lines = [f"\n## {g['skill']}（{g['copies']} 份实体，分布于: {', '.join(g['agents'])}"
                 f"{'；中央库已有同名' if g['in_central'] else ''}）"]
        lines.append(f"- 最新副本 [{newest}] {info[newest]['mtime']} "
                     f"{info[newest]['files']} 文件 {fmt_bytes(info[newest]['bytes'])}")
        for o in others:
            lines.append(f"- 对比副本 [{o}] {info[o]['mtime']} "
                         f"{info[o]['files']} 文件 {fmt_bytes(info[o]['bytes'])}")
        if only_a:
            lines.append(f"- 仅最新副本有: {', '.join(only_a)}")
        if only_b:
            lines.append(f"- 仅 {peer} 副本有: {', '.join(only_b)}")
        lines.append(f"- [{newest}] SKILL.md: {fm_summary(a_path)}")
        lines.append(f"- [{peer}] SKILL.md: {fm_summary(b_path)}")
        parts.append("\n".join(lines))

    out = Path(sys.argv[1]).with_name(Path(sys.argv[1]).stem.replace("replicas-llm-input", "llm-brief") + ".md")
    out.write_text("\n".join(parts))
    print(json.dumps({"brief": str(out), "groups": len(groups),
                      "bytes": out.stat().st_size}, ensure_ascii=False))


if __name__ == "__main__":
    sys.exit(main())
