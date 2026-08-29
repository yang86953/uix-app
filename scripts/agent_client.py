#!/usr/bin/env python3
"""UIX Agent Bridge 客户端：连接 demo 的本地 IPC，发送 JSON Lines 命令。

用法:
  python agent_client.py hello
  python agent_client.py list_windows
  python agent_client.py snapshot [window_id]
  python agent_client.py screenshot [window_id] [output.png]
  python agent_client.py perform <json-file>   # perform 请求体从 JSON 文件读取
  python agent_client.py click <x> <y> [window_id]
"""
import base64
import json
import os
import socket
import sys
import time

if os.name == "nt":
    import win32file

# 统一以 UTF-8 输出，避免 Windows 控制台默认 GBK 编码破坏 JSON。
sys.stdout.reconfigure(encoding="utf-8")

AGENT_PROTOCOL_SCHEMA = "uix.agent.v1"
INITIAL_MAX_REQUEST_BYTES = 4 * 1024 * 1024
INITIAL_MAX_RESPONSE_BYTES = 4 * 1024 * 1024
HARD_MAX_REQUEST_BYTES = 16 * 1024 * 1024
HARD_MAX_RESPONSE_BYTES = 48 * 1024 * 1024
HARD_MAX_SCREENSHOT_BYTES = 32 * 1024 * 1024


def negotiated_limit(limits, name, legacy_name, fallback, hard_max):
    """读取服务端发布的正整数上限，同时保留客户端自身的硬边界。"""
    value = limits.get(name, limits.get(legacy_name, fallback))
    if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
        raise RuntimeError(f"agent hello returned invalid {name}")
    if value > hard_max:
        raise RuntimeError(f"agent hello {name} exceeds the client safety limit")
    return value


def discovery_dir():
    """返回与框架 private_discovery_directory 一致的平台目录。"""
    if os.name == "nt":
        return os.path.join(os.environ.get("LOCALAPPDATA", ""), "uix-agent")
    runtime_dir = os.environ.get("XDG_RUNTIME_DIR") or f"/run/user/{os.getuid()}"
    return os.path.join(runtime_dir, f"uix-agent-{os.getuid()}")


def load_discovery():
    """读取最新的 discovery 文件，返回 (endpoint, token)。"""
    directory = discovery_dir()
    files = [
        os.path.join(directory, name)
        for name in os.listdir(directory)
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
        self.max_request_bytes = INITIAL_MAX_REQUEST_BYTES
        self.max_response_bytes = INITIAL_MAX_RESPONSE_BYTES
        self.max_screenshot_bytes = HARD_MAX_SCREENSHOT_BYTES
        if os.name == "nt":
            # Windows 使用命名管道。
            self.handle = win32file.CreateFile(
                endpoint,
                win32file.GENERIC_READ | win32file.GENERIC_WRITE,
                0,
                None,
                win32file.OPEN_EXISTING,
                0,
                None,
            )
            self.socket = None
        else:
            # Unix 使用框架发现文件声明的私有本地 socket。
            self.socket = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            self.socket.connect(endpoint)
            self.handle = None
        # 保持默认字节模式读取，服务端按 JSON Lines 写入。
        self.buffer = bytearray()

    def send(self, payload):
        """发送一个 JSON 对象并读取一行响应。"""
        # 每个请求都必须携带 request_id 与协议 schema。
        request_id = payload.setdefault("request_id", f"req-{time.time_ns()}")
        schema = payload.setdefault("schema", AGENT_PROTOCOL_SCHEMA)
        if not isinstance(request_id, str) or not request_id:
            raise RuntimeError("agent request_id must be a non-empty string")
        if schema != AGENT_PROTOCOL_SCHEMA:
            raise RuntimeError("agent request schema does not match the client schema")
        encoded = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        if len(encoded) > self.max_request_bytes:
            raise RuntimeError("agent request exceeds the negotiated protocol limit")
        line = encoded + b"\n"
        if self.socket is not None:
            self.socket.sendall(line)
        else:
            win32file.WriteFile(self.handle, line)
        # 读取直到收到完整行。
        while True:
            newline = self.buffer.find(b"\n")
            if newline >= 0:
                if newline + 1 > self.max_response_bytes:
                    raise RuntimeError("agent response exceeds the negotiated protocol limit")
                break
            if len(self.buffer) >= self.max_response_bytes:
                raise RuntimeError("agent response exceeds the negotiated protocol limit")
            if self.socket is not None:
                data = self.socket.recv(65536)
                if not data:
                    raise RuntimeError("agent IPC closed before reply")
            else:
                _, data = win32file.ReadFile(self.handle, 65536)
            self.buffer.extend(data)
        raw = bytes(self.buffer[:newline])
        del self.buffer[:newline + 1]
        try:
            reply = json.loads(raw.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise RuntimeError("agent response is not valid UTF-8 JSON") from error
        if not isinstance(reply, dict):
            raise RuntimeError("agent response must be a JSON object")
        if reply.get("schema") != AGENT_PROTOCOL_SCHEMA:
            raise RuntimeError("agent response schema does not match the client schema")
        if reply.get("request_id") != request_id:
            raise RuntimeError("agent response request_id does not match the request")
        return reply

    def hello(self):
        reply = self.send({"schema": AGENT_PROTOCOL_SCHEMA, "type": "hello", "token": self.token})
        if reply.get("ok"):
            limits = reply.get("limits")
            if not isinstance(limits, dict):
                raise RuntimeError("agent hello response is missing protocol limits")
            self.max_request_bytes = negotiated_limit(
                limits, "max_request_bytes", "max_message_bytes",
                INITIAL_MAX_REQUEST_BYTES, HARD_MAX_REQUEST_BYTES,
            )
            self.max_response_bytes = negotiated_limit(
                limits, "max_response_bytes", "max_message_bytes",
                INITIAL_MAX_RESPONSE_BYTES, HARD_MAX_RESPONSE_BYTES,
            )
            self.max_screenshot_bytes = negotiated_limit(
                limits, "max_screenshot_bytes", "max_screenshot_bytes",
                HARD_MAX_SCREENSHOT_BYTES, HARD_MAX_SCREENSHOT_BYTES,
            )
        return reply

    def list_windows(self):
        return self.send({"type": "list_windows"})

    def snapshot(self, window_id):
        return self.send({"type": "snapshot", "window_id": window_id})

    def perform(self, request):
        request.setdefault("schema", "uix.agent.v1")
        request.setdefault("type", "perform")
        return self.send(request)

    def wait_presented(self, window_id, generation, revision, timeout_ms=30000):
        """等待动作对应语义修订真正完成呈现。"""
        return self.send({
            "type": "wait",
            "window_id": window_id,
            "generation": generation,
            "presented_revision": revision,
            "timeout_ms": timeout_ms,
        })

    def close(self):
        if self.socket is not None:
            self.socket.close()
        else:
            win32file.CloseHandle(self.handle)


def resolve_window(session, window_id=None):
    """从 list_windows 解析目标窗口的 id 与真实 generation。"""
    reply = session.list_windows()
    windows = reply.get("windows") or []
    if not windows:
        sys.exit("无可用窗口")
    if window_id is None:
        selected = windows[0]
    else:
        selected = next(
            (window for window in windows if window.get("window_id") == window_id),
            None,
        )
        if selected is None:
            sys.exit(f"窗口不存在: {window_id}")
    return selected["window_id"], selected["generation"]


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
        if command == "hello":
            # 握手已在上方完成并输出；该命令不再发送第二次 hello。
            pass
        elif command == "list_windows":
            print(json.dumps(session.list_windows(), ensure_ascii=False))
        elif command == "snapshot":
            window_id = int(args[1]) if len(args) > 1 else None
            if window_id is None:
                window_id, _ = resolve_window(session)
            print(json.dumps(session.snapshot(window_id), ensure_ascii=False))
        elif command == "click":
            x, y = float(args[1]), float(args[2])
            requested_window_id = int(args[3]) if len(args) > 3 else None
            window_id, generation = resolve_window(session, requested_window_id)
            print(json.dumps(session.perform({
                "window_id": window_id,
                "generation": generation,
                "action": {"kind": "click_at", "x": x, "y": y},
            }), ensure_ascii=False))
        elif command == "screenshot":
            # 用法: screenshot [window_id] [output.png]；第二参数为纯数字时按
            # window_id 解析，否则视为输出路径；第三个参数始终是输出路径。
            window_id = None
            out_path = "uix-screenshot.png"
            if len(args) > 1:
                if args[1].isdigit():
                    window_id = int(args[1])
                else:
                    out_path = args[1]
            if len(args) > 2:
                out_path = args[2]
            if window_id is None:
                window_id, _ = resolve_window(session)
            reply = session.send({"type": "screenshot", "window_id": window_id})
            if not reply.get("ok"):
                print(json.dumps(reply, ensure_ascii=False))
                sys.exit(1)
            data = base64.b64decode(reply["data_base64"], validate=True)
            if len(data) > session.max_screenshot_bytes:
                raise RuntimeError("agent screenshot exceeds the negotiated protocol limit")
            with open(out_path, "wb") as fh:
                fh.write(data)
            print(json.dumps({
                "window_id": reply.get("window_id"),
                "width": reply.get("width"),
                "height": reply.get("height"),
                "format": reply.get("format"),
                "bytes": len(data),
                "path": out_path,
            }, ensure_ascii=False))
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
