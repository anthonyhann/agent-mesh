#!/usr/bin/env python3
"""consolidate_skills.py — skills 中央库规范化迁移

动作（全部可回滚，实体目录先移入 backups 而非删除）：
  A. consolidatable：agent 实体在中央库 ~/.agents/skills 有同名
     - 内容 hash 一致 → 实体移入 backups，建 symlink 指向中央库
     - hash 不一致   → 记入 conflicts，不动（人工决策）
  B. duplicates：多 agent 各存实体，中央库无同名
     - 全部副本 hash 一致 → 最新副本 mv 进中央库，其余换 symlink
     - hash 不一致        → 记入 conflicts，不动
  C. broken symlink：目标不存在
     - 中央库有同名          → 重链到中央库
     - 其他 agent 有同名实体 → 重链到该实体
     - 完全无源             → 删除死链（记入 manifest；symlink 无内容，零风险）

用法：
  consolidate_skills.py            # dry-run：只输出计划
  consolidate_skills.py --apply    # 执行
退出码：0 正常；1 执行期错误。
"""
import datetime
import hashlib
import json
import os
import shutil
import sys
from pathlib import Path

HOME = Path.home()
from mesh_config import AGENT_DIRS, CENTRAL, FORK_REGISTRY
MESH = Path(os.environ.get("MESH_ROOT", Path(__file__).resolve().parents[2]))
IGNORE = {".DS_Store"}

APPLY = "--apply" in sys.argv
TS = datetime.datetime.now().strftime("%Y%m%d-%H%M%S")
BACKUP_DIR = MESH / "backups" / f"consolidate-{TS}"
MANIFEST = {"ts": TS, "apply": APPLY, "actions": [], "conflicts": [], "errors": []}


def dir_hash(d: Path) -> str:
    """递归内容 hash：相对路径 + 文件内容 sha256，排序后总 hash。symlink 以内跟随。"""
    h = hashlib.sha256()
    items = []
    for p in sorted(d.rglob("*")):
        if p.name in IGNORE:
            continue
        rel = str(p.relative_to(d))
        if p.is_symlink():
            items.append((rel, "L", os.readlink(p)))
        elif p.is_file():
            items.append((rel, "F", hashlib.sha256(p.read_bytes()).hexdigest()))
    for rel, kind, digest in items:
        h.update(f"{rel}\0{kind}\0{digest}\0".encode())
    return h.hexdigest()


def scan():
    """name -> {agent: (path, kind, target_resolved|None, exists)}"""
    inv = {}
    for agent, d in AGENT_DIRS.items():
        if not d.is_dir():
            continue
        for e in sorted(d.iterdir()):
            if e.is_symlink():
                raw = os.readlink(e)
                resolved = (e.parent / raw).resolve() if not os.path.isabs(raw) else Path(raw)
                inv.setdefault(e.name, {})[agent] = (e, "symlink", resolved, resolved.exists())
            elif e.is_dir():
                inv.setdefault(e.name, {})[agent] = (e, "entity", None, True)
    return inv


def plan_and_run():
    inv = scan()
    central_names = {p.name for p in CENTRAL.iterdir() if p.is_dir()} if CENTRAL.is_dir() else set()
    # 已审定定制分叉注册表：这些 skill 的实体是 agent 专属适配，跳过一切合并/换链
    forks = set()
    if FORK_REGISTRY.exists():
        try:
            forks = set(json.loads(FORK_REGISTRY.read_text()).get("forks", {}))
        except Exception:
            pass

    def record(action, **kw):
        MANIFEST["actions"].append({"action": action, **kw})

    def do_backup(path: Path) -> Path:
        rel = path.relative_to(HOME) if str(path).startswith(str(HOME)) else Path(path.name)
        dest = BACKUP_DIR / rel
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.move(str(path), str(dest))
        return dest

    def link_to(src: Path, link: Path):
        link.symlink_to(src)

    for name, agents in sorted(inv.items()):
        if name in forks:
            MANIFEST["conflicts"].append({
                "skill": name, "type": "fork_registered",
                "note": "已审定定制分叉（FORK-REGISTRY），跳过",
            })
            continue
        entities = {a: v for a, v in agents.items() if v[1] == "entity"}
        links = {a: v for a, v in agents.items() if v[1] == "symlink"}
        central_path = CENTRAL / name
        in_central = name in central_names

        # ---------- C. 断链 ----------
        broken = {a: v for a, v in links.items() if not v[3]}
        for a, (lpath, _, tgt, _) in broken.items():
            if in_central:
                record("relink", skill=name, agent=a, to=str(central_path), reason="broken->central")
                if APPLY:
                    lpath.unlink()
                    link_to(central_path, lpath)
            elif entities:
                donor = next(iter(entities.values()))[0]
                record("relink", skill=name, agent=a, to=str(donor), reason="broken->entity")
                if APPLY:
                    lpath.unlink()
                    link_to(donor, lpath)
            else:
                record("remove_broken", skill=name, agent=a, dead_target=str(tgt))
                if APPLY:
                    lpath.unlink()

        # ---------- A/B. 实体处理 ----------
        if not entities:
            continue
        live_links = {a: v for a, v in links.items() if v[3]}

        if in_central:
            # A. 中央库有同名：hash 一致才换链
            c_hash = dir_hash(central_path)
            for a, (epath, _, _, _) in entities.items():
                e_hash = dir_hash(epath)
                if e_hash == c_hash:
                    record("replace_with_link", skill=name, agent=a,
                           central=str(central_path), backup=str(BACKUP_DIR))
                    if APPLY:
                        do_backup(epath)
                        link_to(central_path, epath)
                else:
                    MANIFEST["conflicts"].append({
                        "skill": name, "agent": a, "type": "differs_from_central",
                        "entity": str(epath), "central": str(central_path),
                    })
        else:
            # B. 中央库无同名：多副本一致则收一份进中央库
            hashes = {a: dir_hash(v[0]) for a, v in entities.items()}
            if len(set(hashes.values())) == 1 and len(entities) > 0:
                # 选 mtime 最新副本为 donor
                donor_agent = max(entities, key=lambda a: entities[a][0].stat().st_mtime)
                donor_path = entities[donor_agent][0]
                record("adopt_to_central", skill=name, from_agent=donor_agent,
                       to=str(central_path), copies=len(entities))
                if APPLY:
                    CENTRAL.mkdir(parents=True, exist_ok=True)
                    shutil.move(str(donor_path), str(central_path))
                    link_to(central_path, donor_path)
                for a, (epath, _, _, _) in entities.items():
                    if a == donor_agent:
                        continue
                    record("replace_with_link", skill=name, agent=a,
                           central=str(central_path))
                    if APPLY:
                        do_backup(epath)
                        link_to(central_path, epath)
                # 该 skill 现在进中央库了，更新 in_central 语义给断链用（本轮 C 已跑过，无碍）
            elif len(entities) > 1:
                MANIFEST["conflicts"].append({
                    "skill": name, "type": "replicas_differ",
                    "agents": sorted(entities),
                    "note": "多副本内容不一致，未合并",
                })

    return MANIFEST


def main():
    m = plan_and_run()
    out = MESH / "results" / f"consolidate-{TS}.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(m, ensure_ascii=False, indent=2))

    stats = {}
    for act in m["actions"]:
        stats[act["action"]] = stats.get(act["action"], 0) + 1
    print(json.dumps({
        "mode": "apply" if APPLY else "dry-run",
        "planned": stats,
        "conflicts": len(m["conflicts"]),
        "backup_dir": str(BACKUP_DIR) if APPLY else None,
        "manifest": str(out),
    }, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
