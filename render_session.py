#!/usr/bin/env python3
"""render_session.py — 把一次 worker 运行渲染为可点开的「子会话」页面

输入: results/<id>/（stdout.txt[NDJSON 事件流或纯文本]、stderr.txt、result.json）
输出: results/<id>/session.html

- 有事件流（如 pi --mode json / dsh --json）：时间线渲染（turn/message/tool 调用/token 累计）
- 无事件流：简版（stdout/stderr + 指标卡片）
用法: render_session.py <results_dir>
"""
import html
import json
import sys
from pathlib import Path

WORKER_COLOR = {
    "dsh": "#7c3aed", "codex": "#10a37f", "opencode": "#0ea5e9",
    "hermes": "#f59e0b", "arkcli": "#ff6a00", "claude": "#d97706",
    "qwen": "#615ced", "pi": "#e11d48", "catdesk": "#6e6e73",
}
ROLE_STYLE = {
    "system": ("#8e8e93", "SYS"),
    "user": ("#0a84ff", "YOU"),
    "assistant": ("#7c3aed", "AI"),
    "tool": ("#ff9500", "TOOL"),
}


def esc(x):
    return html.escape(str(x))


def load_events(stdout_txt):
    """检测并解析 NDJSON 事件流；非事件流返回 None。"""
    lines = [l for l in stdout_txt.strip().split("\n") if l.strip()]
    if not lines:
        return None
    try:
        first = json.loads(lines[0])
    except Exception:
        return None
    if not isinstance(first, dict) or "type" not in first:
        return None
    events = []
    for l in lines:
        try:
            events.append(json.loads(l))
        except Exception:
            continue
    return events


def text_of(content):
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        return "\n".join(b.get("text", "") for b in content if isinstance(b, dict) and b.get("type") == "text")
    return ""


def render_message(m, depth=0):
    role = m.get("role", "assistant")
    color, tag = ROLE_STYLE.get(role, ("#6e6e73", role.upper()[:4]))
    content = m.get("content")
    body = []
    # 文本块
    txt = text_of(content)
    if txt.strip():
        body.append(f"<pre class='msg'>{esc(txt[:3000])}</pre>")
    # 工具调用块（Anthropic tool_use / OpenAI tool_calls 两种形态）
    blocks = content if isinstance(content, list) else []
    for b in blocks:
        if not isinstance(b, dict):
            continue
        bt = b.get("type")
        if bt in ("tool_use", "tool_call", "function_call"):
            name = b.get("name") or b.get("function", {}).get("name", "tool")
            args = b.get("input") or b.get("arguments") or b.get("function", {}).get("arguments") or {}
            body.append(
                f"<div class='toolcall'><span class='tooltag' style='background:#ff9500'>⚙ {esc(name)}</span>"
                f"<code>{esc(json.dumps(args, ensure_ascii=False)[:400])}</code></div>")
        elif bt in ("tool_result", "function_result"):
            out = b.get("content") or b.get("output") or ""
            if isinstance(out, list):
                out = text_of(out)
            body.append(f"<div class='toolresult'>↳ <code>{esc(str(out)[:400])}</code></div>")
    # usage 角标
    u = m.get("usage") or {}
    ustr = ""
    if u.get("totalTokens"):
        ustr = f"<span class='usage'>⛁ {u['totalTokens']:,} tok</span>"
    err = m.get("errorMessage")
    if err:
        body.append(f"<div class='err'>⚠ {esc(str(err)[:300])}</div>")
    if not body:
        return ""
    return (f"<div class='turn' style='border-left-color:{color}'>"
            f"<div class='role' style='color:{color}'>{tag}{ustr}</div>{''.join(body)}</div>")


def render(rd):
    result = json.loads((rd / "result.json").read_text())
    stdout_txt = (rd / "stdout.txt").read_text(errors="replace") if (rd / "stdout.txt").exists() else ""
    stderr_txt = (rd / "stderr.txt").read_text(errors="replace") if (rd / "stderr.txt").exists() else ""
    worker = result.get("worker", "?")
    color = WORKER_COLOR.get(worker, "#8e8e93")
    sid = result.get("session_id")
    events = load_events(stdout_txt)

    # 头部指标
    metrics = []
    metrics.append(("状态", result.get("status")))
    metrics.append(("耗时", f"{result.get('duration_s')}s"))
    if result.get("tokens"):
        metrics.append(("Tokens", f"{result['tokens']:,}"))
    if result.get("cost_usd"):
        metrics.append(("成本", f"${result['cost_usd']:.4f}"))
    if result.get("attempts", 1) > 1:
        metrics.append(("尝试", str(result["attempts"])))
    metric_html = "".join(
        f"<div class='metric'><div class='mv'>{esc(v)}</div><div class='mk'>{esc(k)}</div></div>"
        for k, v in metrics)

    # 会话续接提示
    continue_html = ""
    if sid:
        continue_html = (
            f"<div class='continue'>↩ 在同一上下文继续：下轮任务单带 "
            f"<code>\"session_id\": \"{esc(sid)}\"</code></div>")

    # 主体：事件流时间线 或 简版输出
    if events:
        parts = []
        for e in events:
            t = e.get("type")
            if t in ("message_start", "message_end"):
                h = render_message(e.get("message", {}))
                if h and t == "message_end":
                    parts.append(h)
            elif t == "turn_start":
                parts.append("<div class='turnmark'>▸ turn</div>")
            elif t == "agent_end" or t == "turn_end":
                u = e.get("usage") or e.get("message", {}).get("usage") or {}
                if u.get("totalTokens"):
                    parts.append(f"<div class='turnmark turnend'>▪ turn end · ⛁ {u['totalTokens']:,} tok</div>")
        body = "".join(parts) or f"<pre class='msg'>{esc(stdout_txt[:4000])}</pre>"
        mode_note = "事件流时间线（含思考与工具调用）"
    else:
        body = (f"<h3>stdout</h3><pre class='msg'>{esc(stdout_txt[:6000])}</pre>"
                f"<h3>stderr</h3><pre class='msg err2'>{esc(stderr_txt[:3000])}</pre>")
        mode_note = "终端输出（该 worker 无过程事件流）"

    return f"""<!DOCTYPE html>
<html lang="zh-CN"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>子会话 · {esc(result['task_id'])}</title>
<style>
  body{{font-family:-apple-system,'SF Pro Text','PingFang SC',sans-serif;background:#f5f5f7;color:#1d1d1f;margin:0}}
  .wrap{{max-width:860px;margin:0 auto;padding:28px 18px 64px}}
  h1{{font-size:22px;margin:0 0 2px}}
  .badge{{display:inline-block;color:#fff;font-size:12px;font-weight:600;padding:3px 12px;border-radius:99px;background:{color};vertical-align:middle;margin-left:8px}}
  .sub{{color:#86868b;font-size:12px;margin-bottom:16px}}
  .metrics{{display:flex;gap:10px;flex-wrap:wrap;margin-bottom:14px}}
  .metric{{background:#fff;border-radius:12px;padding:10px 16px;box-shadow:0 1px 3px rgba(0,0,0,.06)}}
  .mv{{font-size:19px;font-weight:700}} .mk{{font-size:11px;color:#86868b}}
  .continue{{background:#eef6ff;border:1px solid #c8e0ff;border-radius:12px;padding:10px 14px;font-size:13px;margin-bottom:18px;word-break:break-all}}
  .continue code{{background:#fff;padding:1px 6px;border-radius:6px;font-size:12px}}
  .brief{{background:#fff;border-radius:12px;padding:14px 16px;margin-bottom:18px;box-shadow:0 1px 3px rgba(0,0,0,.06)}}
  .brief .k{{font-size:11px;color:#86868b;margin-bottom:4px}}
  .turn{{background:#fff;border-left:4px solid #ccc;border-radius:0 12px 12px 0;padding:12px 16px;margin-bottom:10px;box-shadow:0 1px 2px rgba(0,0,0,.05)}}
  .role{{font-size:11px;font-weight:700;letter-spacing:.5px;margin-bottom:6px}}
  .usage{{float:right;color:#aeaeb2;font-weight:400}}
  pre.msg{{background:#f8f8fa;border-radius:8px;padding:10px;font-size:12.5px;overflow:auto;max-height:420px;white-space:pre-wrap;word-break:break-word;margin:4px 0 0}}
  .toolcall{{margin-top:8px;font-size:12px}}
  .toolcall code,.toolresult code{{display:block;background:#fff8ec;border-radius:6px;padding:6px 8px;margin-top:4px;font-size:11.5px;word-break:break-all}}
  .toolresult{{font-size:12px;margin-top:4px;color:#6e6e73}}
  .err{{color:#ff3b30;font-size:12px;margin-top:6px}}
  .err2{{color:#ff9500}}
  .turnmark{{text-align:center;color:#aeaeb2;font-size:11px;margin:14px 0 8px;letter-spacing:1px}}
  .turnend{{color:#8e8e93}}
  h3{{font-size:14px;color:#6e6e73;margin:16px 0 8px}}
  .foot{{text-align:center;color:#aeaeb2;font-size:11px;margin-top:24px}}
</style></head><body><div class="wrap">
<h1>子会话<span class="badge">{esc(worker.upper())}</span></h1>
<div class="sub">{esc(result['task_id'])} · {esc(result.get('started_at',''))} · {mode_note}</div>
<div class="metrics">{metric_html}</div>
{continue_html}
<div class="brief"><div class="k">BRIEF</div><pre class="msg" style="max-height:200px">{esc(result.get('brief_preview') or '(brief 见任务单)')}</pre></div>
{body}
<div class="foot">agent-mesh · render_session.py · 此子任务运行于 worker 自身的认证与权限模型</div>
</div></body></html>"""


def main():
    rd = Path(sys.argv[1])
    out = rd / "session.html"
    out.write_text(render(rd))
    print(str(out))


if __name__ == "__main__":
    main()
