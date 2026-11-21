"""mesh_config.py — agent-mesh 共享配置（DRY 单一事实源）

skills_audit.py / consolidate_skills.py / upgrade_drift.py / analyze_replicas.py
统一从这里读取 agent 目录清单与中央库路径。新增 agent 只改 mesh-agents.json。
"""
import json
import os
from pathlib import Path

HOME = Path.home()
MESH_ROOT = Path(os.environ.get(
    "MESH_ROOT", Path(__file__).resolve().parents[2]))

_DEFAULT_AGENTS = {
    "catpaw":    "~/.catpaw/skills",
    "catpaw-ws": "~/.catpaw/desk_default_workspace/.catpaw/skills",
    "claude":    "~/.claude/skills",
    "workbuddy": "~/.workbuddy-ai/skills",
    "junie":     "~/.junie/skills",
    "qwen":      "~/.qwen/skills",
    "cursor":    "~/.cursor/skills",
    "codebuddy": "~/.codebuddy/skills",
    "opencode":  "~/.config/opencode/skills",
    "hermes":    "~/.hermes/skills",
}
_DEFAULT_CENTRAL = "~/.agents/skills"


def _load():
    agents = {k: Path(os.path.expanduser(v)) for k, v in _DEFAULT_AGENTS.items()}
    central = Path(os.path.expanduser(_DEFAULT_CENTRAL))
    cfg = MESH_ROOT / "mesh-agents.json"
    if cfg.exists():
        try:
            d = json.loads(cfg.read_text())
            if d.get("agents"):
                agents = {k: Path(os.path.expanduser(v)) for k, v in d["agents"].items()}
            if d.get("central"):
                central = Path(os.path.expanduser(d["central"]))
        except Exception:
            pass  # 配置损坏时静默回退默认，不阻塞巡检
    return agents, central


AGENT_DIRS, CENTRAL = _load()
FORK_REGISTRY = CENTRAL / "FORK-REGISTRY.json"
