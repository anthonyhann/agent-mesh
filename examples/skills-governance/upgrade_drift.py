#!/usr/bin/env python3
"""upgrade_drift.py — 实体 vs 中央库版本漂移：升级换链（以中央库为准）

安全规则（硬约束，不可覆盖）：
  1. 跳过 ~/.agents/skills/FORK-REGISTRY.json 中已审定的定制分叉（codex 复核结论：
     这些是 agent 专属适配，换链会破坏对应 agent 运行时）
  2. 实体目录先 mv 到 backups（可回滚），再建 symlink
  3. dry-run 默认，--apply 才执行

用法: upgrade_drift.py [--apply]
"""
import datetime
import hashlib
import json
import os
import shutil
import sys
from pathlib import Path

HOME = Path.home()
from mesh_config import AGENT_DIRS, CENTRAL
REGISTRY = CENTRAL / "FORK-REGISTRY.json"
MESH = Path(os.environ.get("MESH_ROOT", Path(__file__).resolve().parents[2]))
IGNORE = {".DS_Store"}
APPLY = "--apply" in sys.argv
TS = datetime.datetime.now().strftime("%Y%m%d-%H%M%S")
BACKUP_DIR = MESH / "backups" / f"drift-upgrade-{TS}"


def dir_hash(d: Path) -> str:
    h = hashlib.sha256()
    for p in sorted(d.rglob("*")):
        if p.name in IGNORE:
            continue
        rel = str(p.relative_to(d))
        if p.is_symlink():
            h.update(f"{rel}\0L\0{os.readlink(p)}\0".encode())
        elif p.is_file():
            h.update(f"{rel}\0F\0{hashlib.sha256(p.read_bytes()).hexdigest()}\0".encode())
    return h.hexdigest()


def main():
    forks = set()
    if REGISTRY.exists():
        forks = set(json.loads(REGISTRY.read_text()).get("forks", {}))

    actions, skipped_fork, already_same = [], [], []
    for agent, d in AGENT_DIRS.items():
        if not d.is_dir():
            continue
        for e in sorted(d.iterdir()):
            if not (e.is_dir() and not e.is_symlink()):
                continue  # 只处理实体目录
            name = e.name
            cpath = CENTRAL / name
            if not cpath.is_dir():
                continue  # 中央库无同名，不是"漂移"场景
            if name in forks:
                skipped_fork.append({"skill": name, "agent": agent})
                continue
            e_hash = dir_hash(e)
            c_hash = dir_hash(cpath)
            if e_hash == c_hash:
                already_same.append({"skill": name, "agent": agent})
                continue
            # 漂移确认：备份 + 换链
            actions.append({"skill": name, "agent": agent,
                            "entity_hash": e_hash[:12], "central_hash": c_hash[:12]})
            if APPLY:
                rel = e.relative_to(HOME) if str(e).startswith(str(HOME)) else Path(e.name)
                dest = BACKUP_DIR / rel
                dest.parent.mkdir(parents=True, exist_ok=True)
                shutil.move(str(e), str(dest))
                e.symlink_to(cpath)

    out = MESH / "results" / f"drift-upgrade-{TS}.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps({
        "ts": TS, "apply": APPLY, "backup_dir": str(BACKUP_DIR) if APPLY else None,
        "upgraded": actions, "skipped_fork": skipped_fork, "already_same": already_same,
    }, ensure_ascii=False, indent=2))
    print(json.dumps({
        "mode": "apply" if APPLY else "dry-run",
        "to_upgrade": len(actions),
        "skipped_fork_protected": len(skipped_fork),
        "already_same": len(already_same),
        "backup_dir": str(BACKUP_DIR) if APPLY else None,
        "manifest": str(out),
    }, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
