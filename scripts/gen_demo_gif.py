#!/usr/bin/env python3
"""gen_demo_gif.py — 用真实 run manifest 生成 DAG 演示 GIF（README hero 动图）

数据驱动：读 run_manifest.json 的节点/分层/真实耗时，逐帧点亮。
用法: python3 scripts/gen_demo_gif.py <run_manifest.json> <out.gif>
"""
import json
import math
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

W, H = 960, 620
FPS = 8
FONT_CANDIDATES = [
    "/System/Library/Fonts/Hiragino Sans GB.ttc",
    "/System/Library/Fonts/PingFang.ttc",
    "/System/Library/Fonts/STHeiti Medium.ttc",
]

WORKER_COLOR = {
    "dsh": "#7c3aed", "codex": "#10a37f", "opencode": "#0ea5e9",
    "hermes": "#f59e0b", "arkcli": "#ff6a00", "claude": "#d97706",
}
WORKER_NAME = {
    "dsh": "DSH", "codex": "Codex", "opencode": "OpenCode",
    "hermes": "Hermes", "arkcli": "Ark", "claude": "Claude",
}
BG = (245, 245, 247)
CARD = (255, 255, 255)
GRAY = (198, 198, 204)
GREEN = (52, 199, 89)
TEXT = (29, 29, 31)
SUB = (134, 134, 139)


def font(size, bold=False):
    for p in FONT_CANDIDATES:
        try:
            return ImageFont.truetype(p, size, index=0)
        except Exception:
            continue
    return ImageFont.load_default()


def rr(d, box, radius, fill=None, outline=None, width=1):
    d.rounded_rectangle(box, radius=radius, fill=fill, outline=outline, width=width)


def draw_node(d, x, y, w, h, node, phase, pulse):
    """phase: hidden/spawn/running/done"""
    color = WORKER_COLOR.get(node["worker"], "#8e8e93")
    crgb = tuple(int(color.lstrip("#")[i:i+2], 16) for i in (0, 2, 4))
    if phase == "hidden":
        return
    if phase == "spawn":
        rr(d, [x, y, x + w, y + h], 14, fill=CARD, outline=GRAY, width=2)
        return
    border = crgb if phase == "done" else tuple(min(255, c + 60) for c in crgb)
    rr(d, [x, y, x + w, y + h], 14, fill=CARD, outline=border, width=3)
    # 顶部色条
    d.rounded_rectangle([x, y, x + w, y + 6], 3, fill=crgb)
    # worker 徽章
    bw = 92
    rr(d, [x + 14, y + 18, x + 14 + bw, y + 42], 12, fill=crgb)
    d.text((x + 14 + bw / 2, y + 30), WORKER_NAME.get(node["worker"], node["worker"]),
           font=font(17), fill="white", anchor="mm")
    # 标签
    d.text((x + 118, y + 30), node.get("node_label") or node["task_id"], font=font(18), fill=TEXT, anchor="lm")
    # 状态
    if phase == "running":
        r = 7 + int(3 * abs(math.sin(pulse)))
        d.ellipse([x + 20, y + h - 34, x + 20 + 2 * r, y + h - 34 + 2 * r], fill=crgb)
        d.text((x + 44, y + h - 34 + r), "running…", font=font(15), fill=SUB, anchor="lm")
    else:
        # 手绘勾（不依赖字体的 ✓ 字符）
        d.line([x + 20, y + h - 30, x + 26, y + h - 24], fill=GREEN, width=3)
        d.line([x + 26, y + h - 24, x + 36, y + h - 38], fill=GREEN, width=3)
        d.text((x + 44, y + h - 33), f"{node['duration_s']}s", font=font(16), fill=GREEN, anchor="lm")


def arrow(d, x1, y1, x2, y2, col=GRAY, width=3):
    d.line([x1, y1, x2, y2], fill=col, width=width)
    ang = math.atan2(y2 - y1, x2 - x1)
    s = 10
    d.polygon([(x2, y2),
               (x2 - s * math.cos(ang - 0.45), y2 - s * math.sin(ang - 0.45)),
               (x2 - s * math.cos(ang + 0.45), y2 - s * math.sin(ang + 0.45))], fill=col)


def main():
    manifest = json.loads(Path(sys.argv[1]).read_text())
    out_path = sys.argv[2]
    layers = manifest["layers"]
    seq2node = {n["seq"]: n for n in manifest["nodes"]}

    # 布局：每层一行，层内均分
    NW, NH, VGAP = 300, 108, 74
    layer_y = [96 + i * (NH + VGAP) for i in range(len(layers))]
    node_pos = {}
    for li, layer in enumerate(layers):
        n = len(layer)
        total_w = n * NW + (n - 1) * 40
        x0 = (W - total_w) // 2
        for k, s in enumerate(layer):
            node_pos[s] = (x0 + k * (NW + 40), layer_y[li])

    # 时间轴：每层 spawn(3f) → running(帧数∝sqrt(最长耗时)) → done(2f)
    timeline = []
    t = 4  # 标题帧
    for li, layer in enumerate(layers):
        longest = max(seq2node[s]["duration_s"] for s in layer)
        run_frames = max(5, int(math.sqrt(longest) * 2))
        timeline.append((li, "spawn", t, t + 3)); t += 3
        timeline.append((li, "running", t, t + run_frames)); t += run_frames
        timeline.append((li, "done", t, t + 3)); t += 3
    TOTAL = t + 14  # 结尾统计帧

    def phase_of(li, f):
        for (l, ph, a, b) in timeline:
            if l == li and a <= f < b:
                return ph
        if f < timeline[0][2] and li > 0 and f < [x for x in timeline if x[0] == li][0][2]:
            return "hidden"
        return "done" if f >= [x for x in timeline if x[0] == li][-1][3] else \
               ("hidden" if f < [x for x in timeline if x[0] == li][0][2] else "done")

    frames = []
    for f in range(TOTAL):
        img = Image.new("RGB", (W, H), BG)
        d = ImageDraw.Draw(img)
        d.text((W // 2, 34), manifest.get("title", "agent-mesh DAG"), font=font(26), fill=TEXT, anchor="mm")
        d.text((W // 2, 62), "agent-mesh · real run replay", font=font(14), fill=SUB, anchor="mm")

        # 边（依赖线）：当源节点 done 且目标 spawn 起画
        for n in manifest["nodes"]:
            for dep in n.get("depends_on", []):
                if dep in node_pos and n["seq"] in node_pos:
                    x1, y1 = node_pos[dep]; x2, y2 = node_pos[n["seq"]]
                    src_li = next(i for i, l in enumerate(layers) if dep in l)
                    dst_li = next(i for i, l in enumerate(layers) if n["seq"] in l)
                    if phase_of(src_li, f) == "done":
                        arrow(d, x1 + NW // 2, y1 + NH, x2 + NW // 2, y2 - 6)

        for s, (x, y) in node_pos.items():
            li = next(i for i, l in enumerate(layers) if s in l)
            draw_node(d, x, y, NW, NH, seq2node[s], phase_of(li, f), f * 0.6)

        # 结尾统计条
        if f >= TOTAL - 12:
            s = manifest["summary"]
            txt = f"{s['success']}/{s['total']} nodes OK · total {s['duration_s']}s · fresh context per node"
            rr(d, [W // 2 - 300, H - 52, W // 2 + 300, H - 16], 18, fill=(232, 249, 240))
            d.text((W // 2, H - 34), txt, font=font(17), fill=(22, 130, 70), anchor="mm")

        frames.append(img.quantize(colors=200))

    frames[0].save(out_path, save_all=True, append_images=frames[1:],
                   duration=int(1000 / FPS), loop=0, optimize=True)
    print(f"{out_path}: {TOTAL} frames, {Path(out_path).stat().st_size // 1024} KB")


if __name__ == "__main__":
    main()
