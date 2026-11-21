#!/usr/bin/env python3
"""rollback.py — 回滚 consolidate / drift-upgrade / replicas 备份

备份目录结构: backups/<ts>/<agent 相对 skills 路径>/<skill-name>（实体目录）
回滚动作: 原位若是 symlink → 删除链接；把备份实体 mv 回原位。原位已是实体目录则报冲突跳过。

用法:
  rollback.py <backup-dir>            # dry-run，只打印计划
  rollback.py <backup-dir> --apply    # 执行
"""
import json
import os
import shutil
import sys
from pathlib import Path

from mesh_config import AGENT_DIRS, HOME, MESH_ROOT

APPLY = "--apply" in sys.argv


def main():
    backup = Path(sys.argv[1])
    if not backup.is_dir():
        print(f"backup dir not found: {backup}", file=sys.stderr)
        return 2

    # agent skills 根的相对形式（如 .claude/skills）
    rel_roots = {}
    for agent, p in AGENT_DIRS.items():
        try:
            rel_roots[agent] = p.relative_to(HOME)
        except ValueError:
            continue

    restored, conflicts, errors = [], [], []
    for agent, rel_root in rel_roots.items():
        b_root = backup / rel_root
        if not b_root.is_dir():
            continue
        for skill_dir in sorted(b_root.iterdir()):
            if not skill_dir.is_dir():
                continue
            target = HOME / rel_root / skill_dir.name
            entry = {"skill": skill_dir.name, "agent": agent, "to": str(target)}
            if target.is_symlink():
                if APPLY:
                    target.unlink()
                    shutil.move(str(skill_dir), str(target))
                restored.append({**entry, "action": "replaced_symlink"})
            elif target.exists():
                conflicts.append({**entry, "reason": "target is a real directory"})
            else:
                if APPLY:
                    target.parent.mkdir(parents=True, exist_ok=True)
                    shutil.move(str(skill_dir), str(target))
                restored.append({**entry, "action": "restored"})

    print(json.dumps({
        "mode": "apply" if APPLY else "dry-run",
        "backup": str(backup),
        "restored": len(restored),
        "conflicts": conflicts,
        "errors": errors,
        "items": restored,
    }, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
