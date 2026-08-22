#!/usr/bin/env python3
"""自动化复测：依次切换各页面并停留，配合 FrameDiag 日志定位卡顿页面。"""
import json
import sys
import time

import agent_client

def wait(label, seconds):
    """打印动作标记并等待，日志按此对应页面段。"""
    print(f"[ACTION] {label} (now, wait {seconds}s)", flush=True)
    time.sleep(seconds)


def main():
    endpoint, token = agent_client.load_discovery()
    session = agent_client.AgentSession(endpoint, token)
    session.hello()
    window_id, generation = agent_client.first_window(session)
    print("[ACTION] repro start", flush=True)

    def switch(automation_id):
        """语义 invoke 点击侧边栏按钮切页。"""
        request = {
            "window_id": window_id,
            "generation": generation,
            "target": {"automation_id": automation_id},
            "action": {"kind": "invoke"},
        }
        started = time.perf_counter()
        reply = session.perform(request)
        ok = reply.get("ok")
        presented = False
        if ok:
            waited = session.wait_presented(
                window_id,
                generation,
                reply["revision"],
            )
            presented = waited.get("ok", False)
            ok = ok and presented
        elapsed_ms = (time.perf_counter() - started) * 1000
        print(
            f"[ACTION] switch {automation_id} ok={ok} "
            f"presented={presented} presented_ms={elapsed_ms:.1f}",
            flush=True,
        )
        if not ok:
            print("  reply:", json.dumps(reply, ensure_ascii=False)[:300], flush=True)
        return ok

    # 阶段 0：先确保回首页，再记录基线。
    switch("sidebar-page-0")
    wait("page0-home-baseline", 8)

    # 阶段 1：应用能力页（含每秒 tick）。
    switch("sidebar-page-1")
    wait("page1-runtime-tick", 8)

    # 阶段 2：图表页。
    switch("sidebar-page-8")
    wait("page8-charts", 8)

    # 阶段 3：覆盖清单页。
    switch("sidebar-page-11")
    wait("page11-gallery", 8)

    # 阶段 4：回首页，确认卡顿是否跟随首页。
    switch("sidebar-page-0")
    wait("page0-home-again", 8)

    # 阶段 5：点击首页 +1 按钮，观察交互动画帧。
    request = {
        "window_id": window_id,
        "generation": generation,
        "target": {"automation_id": "home-count-increment"},
        "action": {"kind": "invoke"},
    }
    reply = session.perform(request)
    presented = False
    if reply.get("ok"):
        presented = session.wait_presented(
            window_id,
            generation,
            reply["revision"],
        ).get("ok", False)
    print(
        f"[ACTION] click home-count-increment ok={reply.get('ok')} "
        f"presented={presented}",
        flush=True,
    )
    wait("after-increment-click", 5)

    print("[ACTION] repro done", flush=True)
    session.close()


if __name__ == "__main__":
    main()
