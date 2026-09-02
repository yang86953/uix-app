#!/usr/bin/env python3
"""窗口动作便捷入口：win_action.py <action> [window_id] [json-args]
action: activate_window / resize_window / move_window / maximize_window /
        minimize_window / restore_window / close_window / wait
"""
import json
import sys
import time

sys.path.insert(0, "scripts")
from agent_client import AgentSession, load_discovery, resolve_window  # noqa: E402

action = sys.argv[1]
window_id = int(sys.argv[2]) if len(sys.argv) > 2 else None
extra = json.loads(sys.argv[3]) if len(sys.argv) > 3 else {}

endpoint, token = load_discovery()
session = AgentSession(endpoint, token)
try:
    session.hello()
    wid, generation = resolve_window(session, window_id)
    if action == "wait":
        reply = session.send({"type": "wait", "window_id": wid,
                              "generation": generation, "timeout_ms": 30000})
    else:
        request = {"window_id": wid, "generation": generation,
                   "action": {"kind": action}}
        request.update(extra)
        reply = session.perform(request)
    print(json.dumps(reply, ensure_ascii=False))
finally:
    session.close()
