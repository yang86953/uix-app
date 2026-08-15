#!/usr/bin/env python3
"""UIX Agent Bridge 客户端：连接 demo 的命名管道，发送 JSON Lines 命令。

用法:
  python agent_client.py hello
  python agent_client.py list_windows
  python agent_client.py snapshot [window_id]
  python agent_client.py perform <json-file>   # perform 请求体从 JSON 文件读取
  python agent_client.py click <x> <y> [window_id]
"""
import json
import os
import sys
import time

import win32file
import win32pipe

# 统一以 UTF-8 输出，避免 Windows 控制台默认 GBK 编码破坏 JSON。
sys.stdout.reconfigure(encoding="utf-8")

# 发现文件目录：与 demo 的 private_discovery_directory 一致。
LOCALAPPDATA = os.environ.get("LOCALAPPDATA", "")
DISCOVERY_DIR = os.path.join(LOCALAPPDATA, "uix-agent")


def load_discovery():
    """读取最新的 discovery 文件，返回 (endpoint, token)。"""
    files = [
        os.path.join(DISCOVERY_DIR, name)
        for name in os.listdir(DISCOVERY_DIR)
        if name.startswith("uix-") and name.endswith(".json")
    ]
    if not files:
        sys.exit("未找到 discovery 文件，请确认 demo 已带 --agent-control 启动")
    latest = max(files, key=os.path.getmtime)
    with open(latest, encoding="utf-8") as fh:
        data = json.load(fh)
    return data["endpoint"], data["token"]


class AgentSession:
    """管理命名管道连接与 JSON Lines 请求/响应。"""

    def __init__(self, endpoint, token):
        self.token = token
        # 打开命名管道并等待服务端就绪。
        self.handle = win32file.CreateFile(
            endpoint,
            win32file.GENERIC_READ | win32file.GENERIC_WRITE,
            0,
            None,
            win32file.OPEN_EXISTING,
            0,
            None,
        )
        # 保持默认字节模式读取，服务端按 JSON Lines 写入。
        self.buffer = b""

    def send(self, payload):
        """发送一个 JSON 对象并读取一行响应。"""
        # 每个请求都必须携带 request_id 与协议 schema。
        payload.setdefault("request_id", f"req-{time.time_ns()}")
        payload.setdefault("schema", "uix.agent.v1")
        line = (json.dumps(payload, ensure_ascii=False) + "\n").encode("utf-8")
        win32file.WriteFile(self.handle, line)
        # 读取直到收到完整行。
        while b"\n" not in self.buffer:
            _, data = win32file.ReadFile(self.handle, 65536)
            self.buffer += data
        raw, self.buffer = self.buffer.split(b"\n", 1)
        return json.loads(raw.decode("utf-8"))

    def hello(self):
        return self.send({"schema": "uix.agent.v1", "type": "hello", "token": self.token})

    def list_windows(self):
        return self.send({"type": "list_windows"})

    def snapshot(self, window_id):
        return self.send({"type": "snapshot", "window_id": window_id})

    def perform(self, request):
        request.setdefault("schema", "uix.agent.v1")
        request.setdefault("type", "perform")
        return self.send(request)

    def close(self):
        win32file.CloseHandle(self.handle)


def first_window(session):
    """从 list_windows 取第一个窗口的 id 与 generation。"""
    reply = session.list_windows()
    windows = reply.get("windows") or []
    if not windows:
        sys.exit("无可用窗口")
    first = windows[0]
    return first["window_id"], first["generation"]


def main():
    args = sys.argv[1:]
    if not args:
        sys.exit(__doc__)
    endpoint, token = load_discovery()
    session = AgentSession(endpoint, token)
    try:
        print("== hello ==")
        print(json.dumps(session.hello(), ensure_ascii=False))
        command = args[0]
        if command == "list_windows":
            print(json.dumps(session.list_windows(), ensure_ascii=False))
        elif command == "snapshot":
            window_id = int(args[1]) if len(args) > 1 else None
            if window_id is None:
                window_id, _ = first_window(session)
            print(json.dumps(session.snapshot(window_id), ensure_ascii=False))
        elif command == "click":
            x, y = float(args[1]), float(args[2])
            window_id = int(args[3]) if len(args) > 3 else None
            if window_id is None:
                window_id, generation = first_window(session)
            else:
                generation = 0
            print(json.dumps(session.perform({
                "window_id": window_id,
                "generation": generation,
                "action": {"kind": "click_at", "x": x, "y": y},
            }), ensure_ascii=False))
        elif command == "perform":
            with open(args[1], encoding="utf-8") as fh:
                request = json.load(fh)
            print(json.dumps(session.perform(request), ensure_ascii=False))
        else:
            sys.exit(f"未知命令: {command}")
    finally:
        session.close()


if __name__ == "__main__":
    main()
