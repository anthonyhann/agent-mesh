#!/usr/bin/env python3
"""mesh_tui.py — agent-mesh 终端交互界面（TUI）

零依赖（python3 标准库 curses）。快捷完成：环境自检、Skill 安装、派单、状态查看、运维。
启动: ./mesh_tui.py 或 python3 mesh_tui.py（仓库根目录）
按键: ↑↓ 选择 · Enter 确认 · q/Esc 返回或退出
"""
import curses
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(os.environ.get("MESH_ROOT", Path(__file__).resolve().parent))
WORKERS = ["dsh", "codex", "opencode", "hermes", "arkcli", "claude", "qwen", "pi"]
AGENT_DIRS = [
    ("中央库", "~/.agents/skills"), ("Claude", "~/.claude/skills"),
    ("CatPaw", "~/.catpaw/skills"), ("OpenCode", "~/.config/opencode/skills"),
    ("Cursor", "~/.cursor/skills"), ("CodeBuddy", "~/.codebuddy/skills"),
    ("Qwen", "~/.qwen/skills"), ("Junie", "~/.junie/skills"),
    ("Hermes", "~/.hermes/skills"), ("WorkBuddy", "~/.workbuddy-ai/skills"),
]

MENU = [
    ("1", "环境自检", "检查 python3/zstd/timeout 与各 worker CLI"),
    ("2", "安装 Skill 到 Agent", "把 agent-mesh skill 装进各 Agent 技能目录"),
    ("3", "快捷派单", "选 worker 输入任务，立即执行并查看结果"),
    ("4", "运行状态", "最近运行 / 成功率 / tokens 消耗"),
    ("5", "运行示例", "体验 pipeline / DAG 样例任务"),
    ("6", "运维工具", "smoke 自检 / prune 清理"),
    ("q", "退出", ""),
]

LOGO = r"""
                     _                            _
   __ _  __ _  ___ _| |_ _____ _ __ ___   ___ __| |__
  / _` |/ _` |/ _ \ '_ \___/ '_ ` _ \ / _ / __| '_ \
 | (_| | (_| |  __| | | |  | | | | | |  __\__ | | | |
  \__,_|\__, |\___|_| |_|  |_| |_| |_|\___|___|_| |_|
        |___/
"""


def run(cmd, cwd=None, timeout=300):
    try:
        r = subprocess.run(cmd, cwd=cwd or ROOT, capture_output=True, text=True,
                           timeout=timeout)
        return (r.stdout + r.stderr).strip(), r.returncode
    except subprocess.TimeoutExpired:
        return "⏱ 执行超时", 124
    except Exception as e:
        return f"执行异常: {e}", 1


class TUI:
    def __init__(self, stdscr):
        self.s = stdscr
        curses.curs_set(0)
        curses.start_color()
        curses.use_default_colors()
        curses.init_pair(1, curses.COLOR_CYAN, -1)
        curses.init_pair(2, curses.COLOR_GREEN, -1)
        curses.init_pair(3, curses.COLOR_RED, -1)
        curses.init_pair(4, curses.COLOR_YELLOW, -1)
        self.sel = 0
        self.main_loop()

    def add(self, y, x, text, attr=0):
        try:
            self.s.addstr(y, x, text, attr)
        except curses.error:
            pass

    def header(self, title):
        self.s.clear()
        for i, line in enumerate(LOGO.split("\n")):
            self.add(i, 2, line, curses.color_pair(1) | curses.A_BOLD)
        self.add(2, 34, "agent-mesh TUI", curses.A_BOLD)
        self.add(3, 34, title, curses.color_pair(4))
        self.add(4, 34, "↑↓ 选择 · Enter 确认 · q 返回/退出", curses.A_DIM)

    def main_loop(self):
        while True:
            self.header("主菜单")
            y = 7
            for i, (key, name, desc) in enumerate(MENU):
                attr = curses.A_REVERSE if i == self.sel else 0
                self.add(y, 4, f" [{key}] {name:<22}", attr)
                self.add(y, 32, desc, curses.A_DIM)
                y += 1
            self.add(y + 1, 4, self.workers_status_line(), curses.A_DIM)
            self.s.refresh()
            k = self.s.getch()
            if k in (curses.KEY_UP,):
                self.sel = (self.sel - 1) % len(MENU)
            elif k in (curses.KEY_DOWN,):
                self.sel = (self.sel + 1) % len(MENU)
            elif k in (ord("q"), 27):
                if MENU[self.sel][0] == "q":
                    break
                self.sel = len(MENU) - 1
            elif k in (curses.KEY_ENTER, 10, 13):
                action = MENU[self.sel][0]
                if action == "q":
                    break
                {"1": self.page_setup, "2": self.page_skill,
                 "3": self.page_dispatch, "4": self.page_status,
                 "5": self.page_examples, "6": self.page_ops}[action]()

    def workers_status_line(self):
        ok = [w for w in WORKERS if shutil.which(w)]
        return f"已检测 workers ({len(ok)}/{len(WORKERS)}): {' '.join(ok) if ok else '（无）'}"

    def page_scroll(self, title, lines, footer="按任意键返回"):
        self.header(title)
        h, w = self.s.getmaxyx()
        top = 7
        maxw = w - 8
        for i, line in enumerate(lines[-(h - top - 3):]):
            self.add(top + i, 4, line[:maxw])
        self.add(h - 2, 4, footer, curses.color_pair(1))
        self.s.refresh()
        self.s.getch()

    # ---------- 1 环境自检 ----------
    def page_setup(self):
        lines = ["正在检查……"]
        self.page_scroll_nowait("环境自检", lines)
        lines = []
        for name, cmd in [("python3", "python3"), ("zstd", "zstd"), ("timeout(coreutils)", "timeout")]:
            lines.append(self.check(name, shutil.which(cmd), f"未安装：brew install {'zstd' if cmd=='zstd' else 'coreutils'}"))
        lines.append("")
        for w in WORKERS:
            lines.append(self.check(f"worker: {w}", shutil.which(w), "未安装"))
        lines.append("")
        wb = Path.home().joinpath(".workbuddy-ai").exists()
        lines.append(self.check("workbuddy（在线发布，可选）", wb, "未安装（可选）"))
        key = os.environ.get("ANTHROPIC_API_KEY") or (ROOT / "claude.env").exists()
        lines.append(self.check("claude 凭据", key, "未配置 → ./claude.env 写入 ANTHROPIC_API_KEY 即激活"))
        self.page_scroll("环境自检 · 完成", lines)

    def check(self, name, okv, hint):
        if okv:
            return f"  ✅ {name}"
        return f"  ⚠️  {name} — {hint}"

    def page_scroll_nowait(self, title, lines):
        self.header(title)
        for i, line in enumerate(lines):
            self.add(7 + i, 4, line)
        self.s.refresh()

    # ---------- 2 Skill 安装 ----------
    def page_skill(self):
        skill_src = ROOT / "skills" / "agent-mesh"
        lines = [f"skill 源: {skill_src}", ""]
        targets = []
        for label, d in AGENT_DIRS:
            p = Path(d).expanduser()
            if not p.is_dir():
                lines.append(f"  ⏭  {label:<10} {d}（目录不存在）")
                continue
            dest = p / "agent-mesh"
            if dest.is_symlink():
                lines.append(f"  ✅ {label:<10} 已安装（symlink）")
            elif dest.exists():
                lines.append(f"  ⚠️  {label:<10} 已存在非链接目录（跳过）")
            else:
                lines.append(f"  ◻  {label:<10} 可安装")
                targets.append((label, p, dest))
        lines.append("")
        lines.append(f"按 [a] 安装到全部 {len(targets)} 处 · 其他键返回")
        self.page_scroll_nowait("安装 Skill 到 Agent", lines)
        k = self.s.getch()
        if k in (ord("a"), ord("A")):
            done = []
            for label, p, dest in targets:
                try:
                    dest.symlink_to(skill_src)
                    done.append(f"  ✅ {label} → {dest}")
                except Exception as e:
                    done.append(f"  ❌ {label}: {e}")
            done.append("")
            done.append("提示：重开各 Agent 会话后 skill 生效")
            self.page_scroll("安装完成", done)

    # ---------- 3 快捷派单 ----------
    def page_dispatch(self):
        avail = [w for w in WORKERS if shutil.which(w)] or ["dsh"]
        self.header("快捷派单 · 选择 worker")
        for i, w in enumerate(avail):
            self.add(7 + i, 4, f" [{i+1}] {w}")
        self.add(7 + len(avail) + 1, 4, "数字选择 · q 返回", curses.A_DIM)
        self.s.refresh()
        k = self.s.getch()
        idx = k - ord("1")
        if not (0 <= idx < len(avail)):
            return
        worker = avail[idx]
        brief = self.prompt("快捷派单", f"worker={worker} · 输入任务 brief 后回车：")
        if not brief.strip():
            return
        task = {"id": f"tui-{worker}-{int(time.time())}", "worker": worker, "brief": brief}
        tf = ROOT / "results" / ".tui-task.json"
        tf.parent.mkdir(exist_ok=True)
        tf.write_text(json.dumps(task, ensure_ascii=False))
        self.page_scroll_nowait("执行中", [f"worker={worker}", "运行中，请稍候（最长 5 分钟）……"])
        out, rc = run(["/bin/zsh", str(ROOT / "run_worker.sh"), str(tf)], timeout=320)
        tf.unlink(missing_ok=True)
        rj = ROOT / "results" / task["id"] / "result.json"
        lines = [out[:400], ""]
        if rj.exists():
            d = json.loads(rj.read_text())
            lines += [f"status: {d['status']}  耗时: {d['duration_s']}s  tokens: {d.get('tokens')}",
                      "", "回答预览:", d.get("answer_preview", "")[:600],
                      "", f"子会话: file://{rj.parent}/session.html",
                      f"续接 session_id: {d.get('session_id')}"]
        self.page_scroll("派单结果", lines)

    def prompt(self, title, hint):
        self.header(title)
        self.add(7, 4, hint)
        curses.echo()
        curses.curs_set(1)
        self.s.move(9, 4)
        self.s.refresh()
        try:
            buf = self.s.getstr(9, 4, 800)
            return buf.decode("utf-8", errors="replace")
        finally:
            curses.noecho()
            curses.curs_set(0)

    # ---------- 4 运行状态 ----------
    def page_status(self):
        ledger = ROOT / "results" / "ledger.jsonl"
        lines = []
        if not ledger.exists():
            lines = ["尚无运行记录。先派一单：主菜单 → 快捷派单"]
        else:
            recs = []
            for line in ledger.read_text().strip().split("\n"):
                if line.strip():
                    try:
                        recs.append(json.loads(line))
                    except Exception:
                        continue
            ok = sum(1 for r in recs if r["status"] == "success")
            toks = sum(r.get("tokens") or 0 for r in recs)
            lines = [f"累计运行 {len(recs)} 节点 · 成功 {ok} · tokens（有记录）{toks:,}", "",
                     f"{'时间':<20}{'worker':<10}{'状态':<10}{'耗时':<7}{'tokens':<10}任务"]
            for r in recs[-12:][::-1]:
                lines.append(f"{r['ts'][:19]:<20}{r['worker']:<10}{r['status']:<10}"
                             f"{r['duration_s']:<7}{(r.get('tokens') or '—'):<10}{r['task_id'][:36]}")
        self.page_scroll("运行状态", lines)

    # ---------- 5 示例任务 ----------
    def page_examples(self):
        ex = ROOT / "examples" / "tasks"
        pipes = sorted(ex.glob("*pipeline*.json")) + sorted(ex.glob("*dag*.json"))
        self.header("运行示例 · 选择一个样例")
        for i, p in enumerate(pipes):
            self.add(7 + i, 4, f" [{i+1}] {p.name}")
        self.add(7 + len(pipes) + 1, 4, "数字选择 · q 返回", curses.A_DIM)
        self.s.refresh()
        k = self.s.getch()
        idx = k - ord("1")
        if not (0 <= idx < len(pipes)):
            return
        p = pipes[idx]
        runner = ["/bin/zsh", str(ROOT / "run_pipeline.sh"), str(p)] if "pipeline" in p.name \
            else ["python3", str(ROOT / "run_dag.py"), str(p)]
        self.page_scroll_nowait("执行中", [f"{p.name} 运行中（多节点，最长 10 分钟）……"])
        out, rc = run(runner, timeout=620)
        self.page_scroll(f"示例结果 · {p.name}", out.split("\n")[-30:])

    # ---------- 6 运维 ----------
    def page_ops(self):
        self.header("运维工具")
        items = [("1", "smoke --quick（语法+单元，零成本）"),
                 ("2", "smoke 全量（含 worker ping，有调用成本）"),
                 ("3", "prune 数据清理（dry-run 预览）")]
        for i, (k, name) in enumerate(items):
            self.add(7 + i, 4, f" [{k}] {name}")
        self.add(7 + len(items) + 1, 4, "数字选择 · q 返回", curses.A_DIM)
        self.s.refresh()
        k = self.s.getch()
        if k == ord("1"):
            out, _ = run(["/bin/zsh", str(ROOT / "smoke.sh"), "--quick"], timeout=300)
        elif k == ord("2"):
            out, _ = run(["/bin/zsh", str(ROOT / "smoke.sh")], timeout=900)
        elif k == ord("3"):
            out, _ = run(["/bin/zsh", str(ROOT / "prune.sh")], timeout=60)
        else:
            return
        self.page_scroll("运维输出", out.split("\n"))


def main():
    curses.wrapper(lambda s: TUI(s))


if __name__ == "__main__":
    main()
