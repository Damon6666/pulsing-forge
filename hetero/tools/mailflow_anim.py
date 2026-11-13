#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""mailflow_anim.py — 从账本生成信箱流行动画（GIF）
用法: python3 tools/mailflow_anim.py [ledger.json] [out.gif]
产物: assets/mailflow.gif + assets/mailflow-preview.png
"""
import json, os, sys
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import matplotlib.font_manager as fm
from matplotlib.patches import FancyArrowPatch, FancyBboxPatch
from matplotlib.animation import FuncAnimation, PillowWriter

HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LEDGER = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "datasets", "ledger-sample.json")
OUT_GIF = sys.argv[2] if len(sys.argv) > 2 else os.path.join(HERE, "assets", "mailflow.gif")
OUT_PNG = os.path.join(os.path.dirname(OUT_GIF), "mailflow-preview.png")

for f in ["Microsoft YaHei", "SimHei", "Noto Sans CJK SC", "WenQuanYi Zen Hei"]:
    try:
        fm.findfont(f, fallback_to_default=False)
        plt.rcParams["font.family"] = f
        break
    except Exception:
        continue
plt.rcParams["axes.unicode_minus"] = False

letters = json.load(open(LEDGER, encoding="utf-8"))
# 前 10 封为前置规程信（自环），动画只播放关键协作段 #11 起
letters = [l for l in letters if int(l.get("seq", 0)) >= 11]
N = len(letters)

# 角色（按信中出现的名字）
ROLE = {
    "driver": "driver 外部驱动",
    "chief": "chief 总编排",
    "e1": "e1 Claude · 审查",
    "e2": "e2 Aider · 修复",
    "e3": "e3 诊断",
    "e4": "e4 方案",
    "e5": "e5 终验",
    "e6": "e6 自愈",
    "e7": "e7", "e8": "e8",
}
ACTORS = [a for a in ["driver", "chief", "e1", "e2", "e3", "e4", "e5", "e6"]]
Y = {a: len(ACTORS) - 1 - i for i, a in enumerate(ACTORS)}

COLOR = {"driver": "#7F7F7F", "chief": "#C55A11", "e1": "#538135", "e2": "#B03A2E",
         "e3": "#4472C4", "e4": "#4472C4", "e5": "#7030A0", "e6": "#7030A0"}

PHASES = [("A 诊断", 0.0, 0.408, "#4472C4"),
          ("B 修复", 0.408, 0.675, "#B03A2E"),
          ("C 审查", 0.675, 0.765, "#538135"),
          ("D 自愈", 0.765, 1.0, "#7030A0")]

def phase_at(p):
    for name, s, e, c in PHASES:
        if s <= p <= e:
            return name, c
    return PHASES[-1][0], PHASES[-1][3]

VW, VH = 12, 6.4
FIG_DPI = 90
HOLD = 5                       # 每个状态停 5 帧
FPS = 8

fig, ax = plt.subplots(figsize=(VW, VH))
fig.subplots_adjust(left=0.02, right=0.98, top=0.90, bottom=0.14)

def draw(state, final=False):
    ax.clear()
    ax.set_xlim(0, 100); ax.set_ylim(-2.6, 8.6); ax.axis("off")

    fig.suptitle("异构多 Agent · 一张账本上的协作（一次真实 Issue 修复）",
                 fontsize=14, fontweight="bold", y=0.965)

    # 泳道
    for a in ACTORS:
        y = Y[a]
        ax.plot([10, 97], [y, y], color="#E3E8EE", lw=1, zorder=0)
        ax.text(8.6, y, ROLE.get(a, a), ha="right", va="center", fontsize=9,
                fontweight="bold" if a in ("chief",) else "normal",
                color=COLOR.get(a, "#333"))

    # 已发生的信：细箭头；最新一封：粗箭头 + 摘要
    for i in range(state):
        l = letters[i]
        frm, to = l.get("from", "?"), l.get("to", "?")
        if frm not in Y or to not in Y:
            continue
        x = 12 + (i / max(N, 1)) * 80
        y0, y1 = Y[frm], Y[to]
        alpha = 0.5 if i < state - 1 else 1.0
        lw = 1.3 if i < state - 1 else 2.6
        col = COLOR.get(frm, "#555")
        ax.add_patch(FancyArrowPatch((x, y0), (x, y1), arrowstyle="-|>",
                                     mutation_scale=14 if i == state - 1 else 7,
                                     color=col, lw=lw, alpha=alpha, zorder=2,
                                     shrinkA=3, shrinkB=3))
        if i == state - 1:
            body = str(l.get("body", "")).replace("\n", " ")[:26] + "…"
            ax.text(x + 0.8, max(y0, y1) + 0.42, f"#{l.get('seq','?')} {body}",
                    fontsize=8.2, color=col, fontweight="bold")

    # 阶段进度条
    ax.add_patch(FancyBboxPatch((10, -1.9), 87, 0.55, boxstyle="round,pad=0.02",
                                fc="#F4F6F8", ec="none", zorder=1))
    p = state / max(N, 1)
    for name, s, e, c in PHASES:
        x0 = 10 + s * 87; w = (e - s) * 87
        ax.add_patch(FancyBboxPatch((x0, -1.9), max(w - 0.3, 0.5), 0.55,
                                    boxstyle="round,pad=0.01", fc=c, alpha=0.85, ec="none", zorder=2))
        ax.text(x0 + max(w - 0.3, 0.5) / 2, -1.62, name, ha="center", va="center",
                fontsize=7.5, color="white", fontweight="bold", zorder=3)
    ax.plot([10 + p * 87, 10 + p * 87], [-2.12, -1.12], color="#1A1A1A", lw=2.2, zorder=4)

    # 计数
    ph, _ = phase_at(p)
    ax.text(10, -2.45, f"信件 {state}/{N}", fontsize=9.5, color="#1A1A1A", fontweight="bold")
    ax.text(50, -2.45, f"阶段：{ph}", fontsize=9.5, color="#1A1A1A", ha="center")
    ax.text(97, -2.45, "关键路径：一把 API Key · 总墙钟 1019s", fontsize=8.5,
            color="#666", ha="right")

    if final:
        ax.add_patch(FancyBboxPatch((24, 5.2), 52, 2.6, boxstyle="round,pad=0.06",
                                    fc="#162235", ec="#C55A11", lw=1.5, alpha=0.96, zorder=6))
        ax.text(50, 7.15, "完成：20 封信 · 17 分钟", ha="center", fontsize=11,
                color="white", fontweight="bold", zorder=7)
        ax.text(50, 6.55, "异构审查抓获 1 次「机械门全绿、功能被删光」的坏修复",
                ha="center", fontsize=8.6, color="#F0D9C6", zorder=7)
        ax.text(50, 6.05, "最终交付：泄露清零 + 函数数量不变",
                ha="center", fontsize=8.6, color="#F0D9C6", zorder=7)
        ax.text(50, 5.55, "三个框架零改造接入 · 账本共 20 封（动画为关键协作段 #11–#20）",
                ha="center", fontsize=8.6, color="#F0D9C6", zorder=7)

def frame(k):
    total_states = N + 1          # 0..N，最后一个为总览帧
    per = HOLD
    state = min(k // per, total_states)
    final = state >= N
    draw(state if not final else N, final=final)

total_frames = (N + 1) * HOLD + HOLD
anim = FuncAnimation(fig, frame, frames=total_frames, interval=1000 / FPS)
os.makedirs(os.path.dirname(OUT_GIF), exist_ok=True)
anim.save(OUT_GIF, writer=PillowWriter(fps=FPS))

# 预览帧（关键段中段）
draw(min(6, N))
fig.savefig(OUT_PNG, dpi=120)
print("saved:", OUT_GIF, f"({os.path.getsize(OUT_GIF)//1024} KB),", OUT_PNG)
