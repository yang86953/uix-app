#!/usr/bin/env python3
"""修复后全套状态矩阵验收：KWin 几何记账 + 桌面阴影测量。"""
import json
import math
import subprocess
import sys
import time

sys.path.insert(0, "scripts")
from agent_client import AgentSession, load_discovery, resolve_window

SCRATCH = "/home/yang/data/code/uix-app/shadow-scratch"

ep, tok = load_discovery()
s = AgentSession(ep, tok)
s.hello()
wid, gen = resolve_window(s, 1)

failures = []


def kwin_frame():
    subprocess.run(["zsh", f"{SCRATCH}/kwin_probe.sh"], capture_output=True)
    time.sleep(0.6)
    out = subprocess.run(
        ["journalctl", "--user", "-u", "plasma-kwin_wayland.service",
         "--since", "-30s", "--no-pager"], capture_output=True, text=True).stdout
    lines = [ln for ln in out.splitlines() if "UIXWIN" in ln]
    if not lines:
        return None
    return json.loads(lines[-1].split("UIXWIN ", 1)[1])


def agent_act(action, **params):
    return s.perform({"window_id": wid, "generation": gen,
                      "action": {"kind": action, **params}}).get("ok")


def shot(name):
    subprocess.run(["spectacle", "-b", "-n", "-f", "-o", f"{SCRATCH}/{name}.png"],
                   capture_output=True)
    time.sleep(0.8)


def maximize_node():
    snap = s.snapshot(wid)
    return next(n["node_id"] for n in snap["snapshot"]["nodes"]
                if str(n.get("name", "")).startswith("Maximize or restore"))


def invoke(node_id):
    return s.perform({"window_id": wid, "generation": gen,
                      "target": {"node_id": node_id},
                      "action": {"kind": "invoke"}}).get("ok")


def expect(label, cond, detail=""):
    print(f"  {'✓' if cond else '✗'} {label} {detail}")
    if not cond:
        failures.append(label)


# ── 1. 程序化 resize 往返 ────────────────────────────────
print("[1] 程序化 resize")
expect("resize 1000x700", agent_act("resize_window", width=1000, height=700))
time.sleep(1.5)
f = kwin_frame()
expect("KWin frame=1000x700", f and f["frame"][2:] == [1000, 700], str(f and f["frame"]))
shot("v1-resized")
expect("resize back 1200x800", agent_act("resize_window", width=1200, height=800))
time.sleep(1.5)
f = kwin_frame()
expect("KWin frame=1200x800", f and f["frame"][2:] == [1200, 800], str(f and f["frame"]))

# ── 2. Agent maximize→restore ×2（快+慢） ─────────────────
print("[2] Agent maximize→restore")
agent_act("maximize_window"); time.sleep(1.8)
f = kwin_frame()
expect("max KWin frame≈1920", f and f["frame"][2] >= 1900, str(f and f["frame"]))
agent_act("restore_window"); time.sleep(1.8)
f = kwin_frame()
expect("restore KWin frame=1200x800", f and f["frame"][2:] == [1200, 800],
       str(f and f["frame"]))
shot("v2-agent-restored")
# 快速连发
agent_act("maximize_window"); agent_act("restore_window"); time.sleep(1.8)
f = kwin_frame()
expect("rapid cycle KWin frame=1200x800", f and f["frame"][2:] == [1200, 800], str(f and f["frame"]))

# ── 3. 应用按钮 maximize→restore ─────────────────────────
print("[3] 应用标题栏按钮")
node = maximize_node()
expect("按钮 maximize", invoke(node)); time.sleep(1.8)
f = kwin_frame()
expect("max KWin frame≈1920", f and f["frame"][2] >= 1900, str(f and f["frame"]))
expect("按钮 restore", invoke(node)); time.sleep(1.8)
f = kwin_frame()
expect("restore KWin frame=1200x800", f and f["frame"][2:] == [1200, 800],
       str(f and f["frame"]))
shot("v3-button-restored")

# ── 4. minimize → compositor unminimize ──────────────────
print("[4] minimize→unminimize")
expect("minimize", agent_act("minimize_window")); time.sleep(1.8)
f = kwin_frame()
expect("KWin minimized", f and f["minimized"], str(f and f["minimized"]))
subprocess.run(["zsh", f"{SCRATCH}/kwin_probe.sh"], capture_output=True)  # warm
js = f"{SCRATCH}/unmin.js"
open(js, "w").write('for (const w of workspace.windowList()) { if (w.resourceClass == "uix-app") w.minimized = false; }')
sid = subprocess.run(["/usr/sbin/qdbus6", "org.kde.KWin", "/Scripting",
                      "org.kde.kwin.Scripting.loadScript", js],
                     capture_output=True, text=True).stdout.strip()
time.sleep(0.3)
subprocess.run(["/usr/sbin/qdbus6", "org.kde.KWin", f"/Scripting/Script{sid}",
                "org.kde.kwin.Script.run"], capture_output=True)
time.sleep(2.0)
f = kwin_frame()
expect("unminimize KWin frame=1200x800", f and f["frame"][2:] == [1200, 800] and not f["minimized"],
       str(f and f["frame"]))
shot("v4-unminimized")

# ── 5. resize→maximize→restore（还原到 resize 后尺寸） ────
print("[5] resize→maximize→restore")
agent_act("resize_window", width=1100, height=760); time.sleep(1.5)
agent_act("maximize_window"); time.sleep(1.8)
agent_act("restore_window"); time.sleep(1.8)
f = kwin_frame()
expect("restore 到 1100x760", f and f["frame"][2:] == [1100, 760], str(f and f["frame"]))
agent_act("resize_window", width=1200, height=800); time.sleep(1.5)
f = kwin_frame()
expect("回到 1200x800", f and f["frame"][2:] == [1200, 800], str(f and f["frame"]))

print()
print("失败项:", failures if failures else "无 — 全部通过")
s.close()
