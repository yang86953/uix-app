#!/bin/zsh
# 快速 maximize↔restore 矩阵：每轮后桌面截图 + 阴影探测
cd /home/yang/data/code/uix-app
S=shadow-scratch
python3 - <<'PYEOF'
import json, subprocess, sys, time
sys.path.insert(0, "scripts")
from agent_client import AgentSession, load_discovery, resolve_window

ep, tok = load_discovery()
s = AgentSession(ep, tok)
s.hello()
wid, gen = resolve_window(s, 1)

def act(action):
    r = s.perform({"window_id": wid, "generation": gen, "action": {"kind": action}})
    return r.get("ok")

def shot(name):
    subprocess.run(["spectacle", "-b", "-n", "-f", "-o",
                    f"/home/yang/data/code/uix-app/shadow-scratch/{name}.png"],
                   capture_output=True)
    time.sleep(0.8)

# B1: maximize→restore 无间隔连发 ×3
for i in range(3):
    ok1, ok2 = act("maximize_window"), act("restore_window")
    time.sleep(2.5)
    shot(f"b1-cycle{i}")
    print(f"B1 cycle {i}: max={ok1} restore={ok2}")

# B2: maximize → settle → restore ×3（带间隔）
for i in range(3):
    act("maximize_window")
    time.sleep(2.0)
    shot(f"b2-max{i}")
    act("restore_window")
    time.sleep(2.0)
    shot(f"b2-restore{i}")
    print(f"B2 cycle {i} done")
s.close()
PYEOF
for f in $S/b1-cycle0 $S/b1-cycle1 $S/b1-cycle2 $S/b2-restore0 $S/b2-restore1 $S/b2-restore2; do
  python3 $S/shadow_probe.py $f.png $S/r2-desktop-bg.png --label "$f" 2>/dev/null | head -2
done