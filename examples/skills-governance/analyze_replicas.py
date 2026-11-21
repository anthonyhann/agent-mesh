#!/usr/bin/env python3
"""analyze_replicas.py — 顽固重复 skills 精细分析与安全合并

对"多 agent 实体重复"的 skill 组：
  1. 两两 hash 找一致子集（含与中央库版本对比）
  2. diff 规模摘要：文件数/总大小/SKILL.md 是否一致/mtime 新旧
  3. 自动合并（--apply 时，仅对完全无争议情形）：
     - 全部实体 hash 一致（或扣除噪声后一致）→ 最新副本收编中央库，其余换链
     - 部分一致 → 一致子集合并到子集内最新副本（不进中央库），并记录剩余分歧
  4. 输出 analyze JSON：供 LLM 分类管线的 diff 摘要

用法: analyze_replicas.py [--apply]
"""
import datetime
import hashlib
import json
import os
import shutil
import sys
from pathlib import Path

HOME = Path.home()
AGENT_DIRS = {
    "catpaw":    HOME / ".catpaw/skills",
    "catpaw-ws": HOME / ".catpaw/desk_default_workspace/.catpaw/skills",
    "claude":    HOME / ".claude/skills",
    "workbuddy": HOME / ".workbuddy-ai/skills",
    "junie":     HOME / ".junie/skills",
    "qwen":      HOME / ".qwen/skills",
    "cursor":    HOME / ".cursor/skills",
    "codebuddy": HOME / ".codebuddy/skills",
    "opencode":  HOME / ".config/opencode/skills",
    "hermes":    HOME / ".hermes/skills",
}
CENTRAL = HOME / ".agents/skills"
MESH = Path(os.environ.get("MESH_ROOT", Path(__file__).resolve().parents[2]))
IGNORE = {".DS_Store"}
APPLY = "--apply" in sys.argv
TS = datetime.datetime.now().strftime("%Y%m%d-%H%M%S")
BACKUP_DIR = MESH / "backups" / f"replicas-{TS}"


def dir_hash(d: Path):
    h = hashlib.sha256()
    n_files = 0
    total = 0
    for p in sorted(d.rglob("*")):
        if p.name in IGNORE:
            continue
        rel = str(p.relative_to(d))
        if p.is_symlink():
            h.update(f"{rel}\0L\0{os.readlink(p)}\0".encode())
        elif p.is_file():
            n_files += 1
            total += p.stat().st_size
            h.update(f"{rel}\0F\0{hashlib.sha256(p.read_bytes()).hexdigest()}\0".encode())
    return h.hexdigest(), n_files, total


def skill_md_digest(d: Path):
    f = d / "SKILL.md"
    if not f.exists():
        return None
    txt = f.read_text(errors="replace")
    return hashlib.sha256(txt.encode()).hexdigest()[:12], len(txt)


def main():
    # 扫描实体（symlink 不在本轮处理范围）
    entities = {}  # name -> {agent: path}
    for agent, d in AGENT_DIRS.items():
        if not d.is_dir():
            continue
        for e in sorted(d.iterdir()):
            if e.is_dir() and not e.is_symlink():
                entities.setdefault(e.name, {})[agent] = e

    groups = []
    actions, conflicts = [], []
    for name, locs in sorted(entities.items()):
        if len(locs) < 2:
            continue  # 单实体不重复
        info = {}
        for a, p in locs.items():
            h, nf, sz = dir_hash(p)
            info[a] = {
                "path": str(p), "hash": h[:12], "files": nf, "bytes": sz,
                "mtime": datetime.datetime.fromtimestamp(p.stat().st_mtime).isoformat(timespec="seconds"),
                "skill_md": skill_md_digest(p),
            }
        # 一致子集
        by_hash = {}
        for a, v in info.items():
            by_hash.setdefault(v["hash"], []).append(a)
        identical_groups = sorted(by_hash.values(), key=len, reverse=True)
        newest = max(info, key=lambda a: info[a]["mtime"])
        md_digests = {v["skill_md"][0] for v in info.values() if v["skill_md"]}
        group = {
            "skill": name,
            "agents": sorted(locs),
            "copies": len(locs),
            "hash_variants": len(by_hash),
            "identical_groups": identical_groups,
            "skill_md_unified": len(md_digests) <= 1,
            "newest_copy": newest,
            "in_central": (CENTRAL / name).is_dir(),
            "info": info,
        }

        # ---------- 决策 ----------
        if len(by_hash) == 1:
            # 完全一致：收编中央库（无同名时），其余换链
            group["auto_decision"] = "adopt_and_link"
            if APPLY:
                donor_path = locs[newest]
                cpath = CENTRAL / name
                CENTRAL.mkdir(parents=True, exist_ok=True)
                if not cpath.exists():
                    shutil.move(str(donor_path), str(cpath))
                    donor_path.symlink_to(cpath)
                    actions.append({"action": "adopt", "skill": name, "from": newest})
                else:
                    rel = str(donor_path.relative_to(HOME))
                    dest = BACKUP_DIR / rel
                    dest.parent.mkdir(parents=True, exist_ok=True)
                    shutil.move(str(donor_path), str(dest))
                    donor_path.symlink_to(cpath)
                    actions.append({"action": "link", "skill": name, "agent": newest})
                for a, p in locs.items():
                    if a == newest:
                        continue
                    rel = str(p.relative_to(HOME))
                    dest = BACKUP_DIR / rel
                    dest.parent.mkdir(parents=True, exist_ok=True)
                    shutil.move(str(p), str(dest))
                    p.symlink_to(cpath)
                    actions.append({"action": "link", "skill": name, "agent": a})
        elif len(identical_groups[0]) >= 2:
            # 部分一致：一致子集合并到子集内最新者（子集不内的不动）
            group["auto_decision"] = "merge_identical_subset"
            subset = identical_groups[0]
            keep = max(subset, key=lambda a: info[a]["mtime"])
            keep_path = locs[keep]
            for a in subset:
                if a == keep:
                    continue
                p = locs[a]
                if APPLY:
                    rel = str(p.relative_to(HOME))
                    dest = BACKUP_DIR / rel
                    dest.parent.mkdir(parents=True, exist_ok=True)
                    shutil.move(str(p), str(dest))
                    p.symlink_to(keep_path)
                actions.append({"action": "link", "skill": name, "agent": a, "to": keep})
            group["merged_subset"] = subset
            group["kept"] = keep
            remaining = [a for a in locs if a not in subset]
            group["needs_llm"] = len(remaining) > 0 or not (CENTRAL / name).exists()
        else:
            group["auto_decision"] = "needs_llm"
            group["needs_llm"] = True

        groups.append(group)

    # 汇总需要 LLM 分类的（含 86 个 differs_from_central 之外的 duplicates 项）
    need_llm = [g for g in groups if g.get("needs_llm") and g["hash_variants"] > 1]
    summary = {
        "ts": TS,
        "mode": "apply" if APPLY else "analyze",
        "duplicate_groups": len(groups),
        "fully_identical": sum(1 for g in groups if g["hash_variants"] == 1),
        "partial_identical": sum(1 for g in groups if g.get("auto_decision") == "merge_identical_subset"),
        "needs_llm": len(need_llm),
        "actions_taken": actions,
        "backup_dir": str(BACKUP_DIR) if APPLY else None,
    }
    out = MESH / "results" / f"replicas-analyze-{TS}.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps({"summary": summary, "groups": groups}, ensure_ascii=False, indent=2))
    # 供 LLM 的精简版
    llm_out = MESH / "results" / f"replicas-llm-input-{TS}.json"
    llm_out.write_text(json.dumps(need_llm, ensure_ascii=False, indent=2))
    summary["report"] = str(out)
    summary["llm_input"] = str(llm_out)
    print(json.dumps(summary, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
