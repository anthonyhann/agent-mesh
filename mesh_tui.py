#!/usr/bin/env python3
"""mesh_tui.py — agent-mesh 终端交互界面（TUI）

零依赖（python3 标准库 curses）。快捷完成：环境自检、Skill 安装、派单、状态查看、运维。
启动: ./mesh_tui.py 或 python3 mesh_tui.py（仓库根目录）
按键: ↑↓ 选择 · Enter 确认 · q 返回/退出

布局：标题栏(顶) / 菜单区(中) / 状态栏(底)。中文按双宽计算列对齐，小窗口自动降级提示。
"""
import curses
import json
import os
import shutil
import subprocess
import sys
import time
import unicodedata
from pathlib import Path

ROOT = Path(os.environ.get("MESH_ROOT", Path(__file__).resolve().parent))
VERSION = "1.1.3"
WORKERS = ["dsh", "codex", "opencode", "hermes", "arkcli", "claude", "qwen", "pi"]
AGENT_DIRS = [
    ("中央库", "~/.agents/skills"), ("Claude", "~/.claude/skills"),
    ("CatPaw", "~/.catpaw/skills"), ("OpenCode", "~/.config/opencode/skills"),
    ("Cursor", "~/.cursor/skills"), ("CodeBuddy", "~/.codebuddy/skills"),
    ("Qwen", "~/.qwen/skills"), ("Junie", "~/.junie/skills"),
    ("Hermes", "~/.hermes/skills"), ("WorkBuddy", "~/.workbuddy-ai/skills"),
]
LOGO = [
    "   _                            _           ",
    "  __ _  __ _  ___ _| |_ _____ _ __ ___   ___ __| |__",
    " / _` |/ _` |/ _ \\ '_ \\___/ '_ ` _ \\ / _ / __| '_ \\",
    "| (_| | (_| |  __| | | |  | | | | | |  __\\__ | | | |",
    " \\__,_|\\__, |\\___|_| |_|  |_| |_| |_|\\___|___|_| |_|",
    "      |___/                                           ",
]

MENU = [
    ("1", "环境自检", "python3 / zstd / timeout / worker CLI 检测"),
    ("2", "安装 Skill", "把 agent-mesh skill 装进各 Agent 技能目录"),
    ("3", "快捷派单", "选 worker 输入任务，立即执行并查看结果"),
    ("4", "运行状态", "最近运行 / 成功率 / tokens 消耗"),
    ("5", "运行示例", "体验 pipeline / DAG 样例任务"),
    ("6", "运维工具", "smoke 自检 / prune 清理"),
    ("q", "退出", ""),
]
MIN_W, MIN_H = 60, 16


# ---------- 显示宽度工具（中文双宽） ----------
def dwidth(s):
    w = 0
    for ch in s:
        w += 2 if unicodedata.east_asian_width(ch) in ("W", "F") else 1
    return w


def pad(s, width):
    """按显示宽度右补齐；超宽截断（中文安全截断）。"""
    out, cur = "", 0
    for ch in s:
        cw = 2 if unicodedata.east_asian_width(ch) in ("W", "F") else 1
        if cur + cw > width:
            break
        out += ch
        cur += cw
    return out + " " * (width - cur)


def run(cmd, cwd=None, timeout=300):
    try:
        r = subprocess.run(cmd, cwd=cwd or ROOT, capture_output=True, text=True, timeout=timeout)
        return (r.stdout + r.stderr).strip(), r.returncode
    except subprocess.TimeoutExpired:
        return "⏱ 执行超时", 124
    except Exception as e:
        return f"执行异常: {e}", 1


class TUI:
    def __init__(self, stdscr):
        self.s = stdscr
        curses.curs_set(0)
        if curses.has_colors():
            curses.start_color()
            curses.use_default_colors()
            curses.init_pair(1, curses.COLOR_CYAN, -1)
            curses.init_pair(2, curses.COLOR_GREEN, -1)
            curses.init_pair(3, curses.COLOR_RED, -1)
            curses.init_pair(4, curses.COLOR_YELLOW, -1)
        self.sel = 0
        self.main_loop()

    def cp(self, n):
        return curses.color_pair(n) if curses.has_colors() else 0

    def put(self, y, x, text, attr=0):
        h, w = self.s.getmaxyx()
        if not (0 <= y < h) or x >= w:
            return
        try:
            self.s.addstr(y, x, pad(text, max(0, w - x)) if w - x > 0 else "", attr)
        except curses.error:
            pass

    def too_small(self):
        h, w = self.s.getmaxyx()
        if w >= MIN_W and h >= MIN_H:
            return False
        self.s.clear()
        msg = f"终端窗口过小（需要至少 {MIN_W}x{MIN_H}，当前 {w}x{h}）"
        self.put(max(0, h // 2), 2, msg, curses.A_BOLD)
        self.s.refresh()
        return True

    def chrome(self, title):
        """框架：LOGO 区（高度足够时）+ 顶栏 + 分隔线 + 底栏。返回内容区起始行。"""
        self.s.clear()          # 全屏重绘：防止上一页内容残留叠加
        h, w = self.s.getmaxyx()
        top = 0
        # LOGO：窗口高度 ≥22 且宽度 ≥52 时展示（独立区域，不与任何内容重叠）
        if h >= 22 and w >= 52:
            for i, line in enumerate(LOGO):
                self.put(i, 2, line, self.cp(1) | curses.A_BOLD)
            top = len(LOGO)
        bar_attr = curses.A_REVERSE | curses.A_BOLD
        left = f" agent-mesh TUI  v{VERSION}"
        right = f" {title} "
        self.put(top, 0, pad(left, max(0, w - dwidth(right) - 1)), bar_attr)
        self.put(top, max(0, w - dwidth(right)), right, bar_attr)
        self.put(top + 1, 0, "─" * (w - 1), self.cp(1))
        top += 2
        ok = [x for x in WORKERS if shutil.which(x)]
        hint = "↑↓ 选择 · Enter 确认 · q 返回/退出"
        self.put(h - 2, 0, "─" * (w - 1), self.cp(1))
        self.put(h - 1, 0, pad(f" workers {len(ok)}/{len(WORKERS)}: " + (" ".join(ok) or "无"), max(0, w - dwidth(hint) - 1)), curses.A_DIM)
        self.put(h - 1, max(0, w - dwidth(hint)), hint, curses.A_DIM)
        return top

    def snapshot(self):
        """MESH_TUI_DUMP 调试/CI：每次渲染后覆盖写当前真实画面（最后一帧即退出前画面）。"""
        path = os.environ.get("MESH_TUI_DUMP")
        if not path:
            return
        hh, ww = self.s.getmaxyx()
        dump = [f"[size {hh}x{ww}]"]
        for r in range(hh):
            try:
                row = self.s.instr(r, 0)
                dump.append(row.decode("utf-8", errors="replace").rstrip())
            except curses.error:
                dump.append("")
        Path(path).write_text("\n".join(dump))

    def main_loop(self):
        while True:
            if self.too_small():
                if self.s.getch() in (ord("q"), 27):
                    break
                continue
            top = self.chrome("主菜单")
            h, w = self.s.getmaxyx()
            y = top + 1
            name_w, key_w = 16, 5
            for i, (key, name, desc) in enumerate(MENU):
                attr = curses.A_REVERSE | curses.A_BOLD if i == self.sel else 0
                line = f" {key}  {pad(name, name_w)} {desc}"
                self.put(y, 2, pad(line, w - 4), attr)
                y += 1
            self.put(y + 1, 2, "选择后回车执行。首次使用建议先跑 ① 环境自检。", curses.A_DIM)
            self.s.refresh()
            self.snapshot()
            k = self.s.getch()
            if k == curses.KEY_UP:
                self.sel = (self.sel - 1) % len(MENU)
            elif k == curses.KEY_DOWN:
                self.sel = (self.sel + 1) % len(MENU)
            elif k in (ord("q"), 27):
                if MENU[self.sel][0] == "q":
                    break
                self.sel = len(MENU) - 1
            elif k in (curses.KEY_ENTER, 10, 13):
                action = MENU[self.sel][0]
                if action == "q":
                    break
                {"1": self.page_setup, "2": self.page_skill, "3": self.page_dispatch,
                 "4": self.page_status, "5": self.page_examples, "6": self.page_ops}[action]()

    def page(self, title, lines, footer="按任意键返回"):
        """通用内容页：可滚动（↑↓/PgUp/PgDn），底部提示。"""
        off = 0
        while True:
            if self.too_small():
                self.s.getch()
                return
            top = self.chrome(title)
            h, w = self.s.getmaxyx()
            body_h = h - top - 3
            for i in range(body_h):
                idx = off + i
                if idx < len(lines):
                    self.put(top + i, 2, lines[idx][: w - 4])
            self.put(h - 1, 1, pad(f" {footer}", w - 2), curses.A_REVERSE)
            self.s.refresh()
            self.snapshot()
            k = self.s.getch()
            if k == curses.KEY_UP:
                off = max(0, off - 1)
            elif k == curses.KEY_DOWN:
                off = min(max(0, len(lines) - body_h), off + 1)
            elif k in (curses.KEY_PPAGE,):
                off = max(0, off - body_h)
            elif k in (curses.KEY_NPAGE,):
                off = min(max(0, len(lines) - body_h), off + body_h)
            elif k in (10, 13, 27, ord("q")):
                return

    def splash(self, title, lines):
        """无等待页（执行中提示）。"""
        if self.too_small():
            return
        top = self.chrome(title)
        for i, line in enumerate(lines):
            self.put(top + i, 2, line)
        self.s.refresh()
        self.snapshot()

    # ---------- 1 环境自检 ----------
    def page_setup(self):
        self.splash("环境自检", ["正在检查……"])
        lines = []
        for name, cmd, hint in [("python3", "python3", "必需的运行环境（brew install python3）"),
                                ("zstd", "zstd", "dsh tokens 采集需要（brew install zstd）"),
                                ("timeout", "timeout", "任务超时保护（brew install coreutils）")]:
            p = shutil.which(cmd)
            lines.append(f"{'✅' if p else '⚠️ '} {name:<10}{p or hint}")
        lines.append("")
        avail = 0
        for w in WORKERS:
            p = shutil.which(w)
            avail += 1 if p else 0
            lines.append(f"{'✅' if p else '· '} worker {w:<10}{p or '未安装（可用其他 worker）'}")
        lines.append("")
        wb = Path.home().joinpath(".workbuddy-ai").exists()
        lines.append(f"{'✅' if wb else '· '} workbuddy           {'已安装（在线发布可用）' if wb else '未安装（可选）'}")
        key = bool(os.environ.get("ANTHROPIC_API_KEY")) or (ROOT / "claude.env").exists()
        lines.append(f"{'✅' if key else '· '} claude 凭据         {'已配置' if key else '未配置（claude.env 写入 ANTHROPIC_API_KEY）'}")
        lines.append("")
        lines.append(f"结论：{avail}/{len(WORKERS)} 个 worker 可用" + ("，即可开始派单。" if avail else "，请先安装至少一个 worker CLI。"))
        self.page("环境自检", lines)

    # ---------- 2 Skill 安装 ----------
    def page_skill(self):
        skill_src = ROOT / "skills" / "agent-mesh"
        lines = [f"skill 源: {skill_src}", ""]
        targets = []
        for label, d in AGENT_DIRS:
            p = Path(d).expanduser()
            if not p.is_dir():
                lines.append(f"·  {label:<10} {d}（目录不存在，跳过）")
                continue
            dest = p / "agent-mesh"
            if dest.is_symlink():
                lines.append(f"✅ {label:<10} 已安装")
            elif dest.exists():
                lines.append(f"⚠️  {label:<10} 已存在非链接目录（跳过）")
            else:
                lines.append(f"◻  {label:<10} 可安装")
                targets.append((label, p, dest))
        lines.append("")
        lines.append(f"回车安装到全部 {len(targets)} 处（a 全装 / 其他键返回）")
        self.splash("安装 Skill", lines)
        k = self.s.getch()
        if k not in (ord("a"), ord("A"), 10, 13):
            return
        done = []
        for label, p, dest in targets:
            try:
                dest.symlink_to(skill_src)
                done.append(f"✅ {label} → {dest}")
            except Exception as e:
                done.append(f"❌ {label}: {e}")
        done += ["", "提示：重开各 Agent 会话后 skill 生效（仓库更新自动同步）。"]
        self.page("安装完成", done)

    # ---------- 3 快捷派单 ----------
    def page_dispatch(self):
        avail = [w for w in WORKERS if shutil.which(w)] or ["dsh"]
        lines = ["选择 worker（数字键）：", ""] + [f" [{i+1}] {w}" for i, w in enumerate(avail)]
        self.splash("快捷派单 · 选择 worker", lines)
        k = self.s.getch()
        idx = k - ord("1")
        if not (0 <= idx < len(avail)):
            return
        worker = avail[idx]
        brief = self.prompt("快捷派单", f"worker = {worker}   输入任务后回车：")
        if not brief.strip():
            return
        task = {"id": f"tui-{worker}-{int(time.time())}", "worker": worker, "brief": brief}
        tf = ROOT / "results" / ".tui-task.json"
        tf.parent.mkdir(exist_ok=True)
        tf.write_text(json.dumps(task, ensure_ascii=False))
        self.splash("执行中", [f"worker = {worker}", "运行中，请稍候（最长 5 分钟）……"])
        out, rc = run(["/bin/zsh", str(ROOT / "run_worker.sh"), str(tf)], timeout=320)
        tf.unlink(missing_ok=True)
        rj = ROOT / "results" / task["id"] / "result.json"
        lines = []
        if rj.exists():
            d = json.loads(rj.read_text())
            lines += [f"状态 {d['status']}   耗时 {d['duration_s']}s   tokens {d.get('tokens') or '—'}",
                      f"子会话 file://{rj.parent}/session.html",
                      f"续接用 session_id: {d.get('session_id') or '—'}", "",
                      "回答预览："]
            lines += (d.get("answer_preview") or "").split("\n")[:20]
        else:
            lines = ["执行未产出 result.json，输出如下："] + out.split("\n")[:25]
        self.page("派单结果", lines)

    def prompt(self, title, hint):
        if self.too_small():
            return ""
        top = self.chrome(title)
        self.put(top, 2, hint)
        curses.echo()
        curses.curs_set(1)
        self.s.refresh()
        try:
            return self.s.getstr(top + 2, 2, 800).decode("utf-8", errors="replace")
        finally:
            curses.noecho()
            curses.curs_set(0)

    # ---------- 4 运行状态 ----------
    def page_status(self):
        ledger = ROOT / "results" / "ledger.jsonl"
        if not ledger.exists():
            return self.page("运行状态", ["尚无运行记录。", "", "先派一单：主菜单 → ③ 快捷派单"])
        recs = []
        for line in ledger.read_text().strip().split("\n"):
            if line.strip():
                try:
                    recs.append(json.loads(line))
                except Exception:
                    continue
        ok = sum(1 for r in recs if r["status"] == "success")
        toks = sum(r.get("tokens") or 0 for r in recs)
        lines = [f"累计 {len(recs)} 节点运行 · 成功 {ok} · 失败 {len(recs)-ok} · tokens(有记录) {toks:,}", "",
                 f"{'时间':<20}{'worker':<10}{'状态':<10}{'耗时':<6}{'tokens':<9}任务"]
        for r in recs[-15:][::-1]:
            lines.append(f"{r['ts'][:19]:<20}{r['worker']:<10}{r['status']:<10}"
                         f"{str(r['duration_s'])+'s':<6}{str(r.get('tokens') or '—'):<9}{r['task_id'][:30]}")
        self.page("运行状态", lines)

    # ---------- 5 示例任务 ----------
    def page_examples(self):
        ex = ROOT / "examples" / "tasks"
        picks = sorted(ex.glob("*pipeline*.json")) + sorted(ex.glob("*dag*.json"))
        if not picks:
            return self.page("运行示例", ["examples/tasks/ 下暂无 pipeline/dag 样例文件。"])
        lines = ["选择样例（数字键）：", ""] + [f" [{i+1}] {p.name}" for i, p in enumerate(picks)]
        self.splash("运行示例", lines)
        k = self.s.getch()
        idx = k - ord("1")
        if not (0 <= idx < len(picks)):
            return
        p = picks[idx]
        runner = (["/bin/zsh", str(ROOT / "run_pipeline.sh"), str(p)] if "pipeline" in p.name
                  else ["python3", str(ROOT / "run_dag.py"), str(p)])
        self.splash("执行中", [f"{p.name}", "多节点运行中（最长 10 分钟）……"])
        out, rc = run(runner, timeout=620)
        self.page(f"示例结果 · {p.name}", out.split("\n")[-40:])

    # ---------- 6 运维 ----------
    def page_ops(self):
        items = [("1", "smoke --quick  语法+单元（零成本）"),
                 ("2", "smoke 全量     含 worker ping（有调用成本）"),
                 ("3", "prune         数据清理预览（dry-run）")]
        self.splash("运维工具", ["数字键选择：", ""] + [f" [{k}] {n}" for k, n in items])
        k = self.s.getch()
        if k == ord("1"):
            out, _ = run(["/bin/zsh", str(ROOT / "smoke.sh"), "--quick"], timeout=300)
        elif k == ord("2"):
            out, _ = run(["/bin/zsh", str(ROOT / "smoke.sh")], timeout=900)
        elif k == ord("3"):
            out, _ = run(["/bin/zsh", str(ROOT / "prune.sh")], timeout=60)
        else:
            return
        self.page("运维输出", out.split("\n"))


def main():
    try:
        curses.wrapper(lambda s: TUI(s))
    except KeyboardInterrupt:
        print("agent-mesh TUI 已退出（Ctrl+C）")


if __name__ == "__main__":
    main()
