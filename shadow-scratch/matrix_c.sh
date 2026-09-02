#!/bin/zsh
# B3/B4: 程序化 resize 交叉最大化；B5: 客户端标题栏按钮路径
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

def act(action, **kw):
    payload = {"window_id": wid, "generation": gen, "action": {"kind": action}}
    payload.update(kw)
    return s.perform(payload).get("ok")

def shot(name):
    subprocess.run(["spectacle", "-b", "-n", "-f", "-o",
                    f"/home/yang/data/code/uix-app/shadow-scratch/{name}.png"],
                   capture_output=True)
    time.sleep(0.8)

def windows():
    return [(w["window_id"], w["logical_width"], w["logical_height"], w["maximized"])
            for w in s.list_windows().get("windows", [])]

# B3: 最大化状态下程序化 resize → restore
act("maximize_window"); time.sleep(2.0)
r = act("resize_window", width=1100, height=750)
print("B3 resize-while-maximized ok:", r, windows())
shot("b3-max-resized")
act("restore_window"); time.sleep(2.0)
shot("b3-restored")
print("B3 after restore:", windows())

# B4: 普通态程序化 resize 往返
act("resize_window", width=1000, height=700); time.sleep(2.0)
shot("b4-resized-1000")
print("B4 resized:", windows())
act("resize_window", width=1200, height=800); time.sleep(2.0)
shot("b4-resized-back")
print("B4 back:", windows())

# B5: 找客户端标题栏 maximize 按钮并点击
snap = s.snapshot(wid)
def find_nodes(node, out):
    if isinstance(node, dict):
        name = str(node.get("name", ""))
        if any(k in name for k in ("最大化", "还原", "最小化", "关闭")) or \
           "max" in str(node.get("automation_id", "")).lower():
            out.append(node)
        for c in node.get("children", []) or []:
            find_nodes(c, out)
nodes = []
if isinstance(snap, dict):
    find_nodes(snap.get("root") or snap.get("tree") or {}, nodes)
print("B5 candidate nodes:", json.dumps(
    [{"name": n.get("name"), "aid": n.get("automation_id"),
      "rid": n.get("role"), "frame": n.get("frame")} for n in nodes[:10]],
    ensure_ascii=False))
s.close()
PYEOF
for f in $S/b3-max-resized $S/b3-restored $S/b4-resized-1000 $S/b4-resized-back; do
  python3 $S/shadow_probe.py $f.png $S/r2-desktop-bg.png --label "$f" 2>/dev/null | head -2
done