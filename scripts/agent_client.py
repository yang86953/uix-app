#!/usr/bin/env python3
"""UIX Agent Bridge 客户端：连接 demo 的本地 IPC，发送 JSON Lines 命令。

用法:
  python agent_client.py apps
  python agent_client.py --instance <instance_id> list_windows
  python agent_client.py hello
  python agent_client.py list_windows
  python agent_client.py snapshot [window_id]
  python agent_client.py screenshot [window_id] [output.png]
  python agent_client.py perform <json-file>   # perform 请求体从 JSON 文件读取
  python agent_client.py click <x> <y> [window_id]
  python agent_client.py session               # 持久连接；stdin/stdout 各一行 JSON
"""
import base64
import json
import os
import socket
import subprocess
import sys
import tempfile
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
HUB_PROTOCOL_SCHEMA = "uix.agent.hub.v1"
HUB_MAX_RESPONSE_BYTES = 128 * 1024


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
    runtime_dir = os.environ.get("XDG_RUNTIME_DIR") or tempfile.gettempdir()
    return os.path.join(runtime_dir, f"uix-agent-{os.getuid()}")


def load_discoveries():
    """读取全部兼容 discovery；它只用于 Hub 尚未登记时的迁移回退。"""
    directory = discovery_dir()
    try:
        names = os.listdir(directory)
    except FileNotFoundError:
        return []
    files = [
        os.path.join(directory, name)
        for name in names
        if name.startswith("uix-") and name.endswith(".json")
    ]
    discoveries = []
    for path in sorted(files):
        try:
            with open(path, encoding="utf-8") as fh:
                data = json.load(fh)
        except (OSError, json.JSONDecodeError):
            continue
        if (
            isinstance(data, dict)
            and data.get("schema") == AGENT_PROTOCOL_SCHEMA
            and isinstance(data.get("endpoint"), str)
            and isinstance(data.get("token"), str)
        ):
            discoveries.append(data)
    return discoveries


def load_discovery():
    """兼容单应用脚本：存在多个实例时拒绝猜测目标。"""
    discoveries = load_discoveries()
    if not discoveries:
        sys.exit("未找到 UIX Agent 应用，请确认应用已带 --agent-control 启动")
    if len(discoveries) != 1:
        sys.exit("检测到多个 UIX Agent 应用，请先运行 apps 并用 --instance 显式选择")
    return discoveries[0]["endpoint"], discoveries[0]["token"]


def hub_endpoint():
    """返回与 UIX 运行时登记线程一致的每用户 Hub 地址。"""
    if os.name == "nt":
        import win32api
        import win32con
        import win32security

        process_token = win32security.OpenProcessToken(
            win32api.GetCurrentProcess(), win32con.TOKEN_QUERY
        )
        try:
            sid = win32security.GetTokenInformation(
                process_token, win32security.TokenUser
            )[0]
        finally:
            win32api.CloseHandle(process_token)
        sid_text = win32security.ConvertSidToStringSid(sid)
        return rf"\\.\pipe\uix-agent-hub-{sid_text}"
    return os.path.join(discovery_dir(), "hub-v1.sock")


class HubSession:
    """管理到每用户 Hub 的持久 JSON Lines 控制连接。"""

    def __init__(self):
        endpoint = hub_endpoint()
        if os.name == "nt":
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
            self.socket = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            self.socket.connect(endpoint)
            self.handle = None
        self.buffer = bytearray()

    def send(self, payload):
        request_id = payload.setdefault("request_id", f"hub-{time.time_ns()}")
        payload.setdefault("schema", HUB_PROTOCOL_SCHEMA)
        encoded = json.dumps(payload, ensure_ascii=False).encode("utf-8") + b"\n"
        if len(encoded) > HUB_MAX_RESPONSE_BYTES:
            raise RuntimeError("hub request exceeds the protocol limit")
        if self.socket is not None:
            self.socket.sendall(encoded)
        else:
            win32file.WriteFile(self.handle, encoded)
        while True:
            newline = self.buffer.find(b"\n")
            if newline >= 0:
                break
            if len(self.buffer) >= HUB_MAX_RESPONSE_BYTES:
                raise RuntimeError("hub response exceeds the protocol limit")
            if self.socket is not None:
                data = self.socket.recv(65536)
                if not data:
                    raise RuntimeError("hub closed before reply")
            else:
                _, data = win32file.ReadFile(self.handle, 65536)
            self.buffer.extend(data)
        raw = bytes(self.buffer[:newline])
        del self.buffer[: newline + 1]
        try:
            reply = json.loads(raw.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise RuntimeError("hub response is not valid UTF-8 JSON") from error
        if (
            not isinstance(reply, dict)
            or reply.get("schema") != HUB_PROTOCOL_SCHEMA
            or reply.get("request_id") != request_id
        ):
            raise RuntimeError("hub response envelope does not match the request")
        return reply

    def close(self):
        if self.socket is not None:
            self.socket.close()
        else:
            win32file.CloseHandle(self.handle)


def connect_hub(auto_start=True):
    """连接 Hub；需要时只启动仓库自带的同用户后台进程。"""
    try:
        return HubSession()
    except OSError:
        if not auto_start:
            raise
    hub_script = os.path.join(os.path.dirname(__file__), "uix_agent_hub.py")
    options = {
        "stdin": subprocess.DEVNULL,
        "stdout": subprocess.DEVNULL,
        "stderr": subprocess.DEVNULL,
        "close_fds": True,
    }
    if os.name == "nt":
        options["creationflags"] = subprocess.CREATE_NEW_PROCESS_GROUP
    else:
        options["start_new_session"] = True
    subprocess.Popen([sys.executable, hub_script, "serve"], **options)
    deadline = time.monotonic() + 2.0
    while time.monotonic() < deadline:
        try:
            return HubSession()
        except OSError:
            time.sleep(0.025)
    raise RuntimeError("UIX Agent Hub failed to start")


def list_hub_apps(hub, settle=True):
    deadline = time.monotonic() + 0.75
    while True:
        reply = hub.send({"type": "list_apps"})
        if not reply.get("ok"):
            raise RuntimeError(f"hub list_apps failed: {reply.get('error')}")
        if not settle or reply.get("stable", True) or time.monotonic() >= deadline:
            return reply.get("apps") or []
        time.sleep(0.025)


def attach_hub_app(hub, instance_id):
    reply = hub.send({"type": "attach", "instance_id": instance_id})
    if not reply.get("ok"):
        raise RuntimeError(f"hub attach failed: {reply.get('error')}")
    app = reply.get("app")
    if not isinstance(app, dict):
        raise RuntimeError("hub attach response is missing app data")
    return app


def select_agent_endpoint(instance_id=None):
    """显式绑定实例；仅有一个实例时允许安全省略选择。"""
    hub = connect_hub()
    try:
        deadline = time.monotonic() + 2.25
        while True:
            apps = list_hub_apps(hub)
            if apps or time.monotonic() >= deadline:
                break
            time.sleep(0.05)
        if instance_id is not None:
            app = attach_hub_app(hub, instance_id)
            return app["endpoint"], app["token"], app
        if len(apps) == 1:
            app = attach_hub_app(hub, apps[0]["instance_id"])
            return app["endpoint"], app["token"], app
        if len(apps) > 1:
            raise RuntimeError("检测到多个 UIX 应用，请用 --instance 显式选择")
    finally:
        hub.close()
    endpoint, token = load_discovery()
    return endpoint, token, None


class AgentSession:
    """管理命名管道连接与 JSON Lines 请求/响应。"""

    def __init__(self, endpoint, token):
        self.token = token
        self.max_request_bytes = INITIAL_MAX_REQUEST_BYTES
        self.max_response_bytes = INITIAL_MAX_RESPONSE_BYTES
        self.max_screenshot_bytes = HARD_MAX_SCREENSHOT_BYTES
        self.closed = False
        self.isolated_workspace = False
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
        if self.closed:
            raise RuntimeError("agent session is closed")
        if payload.get("type") != "hello" and not self.isolated_workspace:
            raise RuntimeError("background_control_required: refusing foreground application control")
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
            capabilities = reply.get("capabilities", {})
            background = capabilities.get("background_control", {}) if isinstance(capabilities, dict) else {}
            if not isinstance(background, dict) or background.get("isolated_workspace") is not True:
                self.close()
                raise RuntimeError("background_control_required: rebuild the application with an independent agent_root")
            self.isolated_workspace = True
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
        if self.closed:
            return
        self.closed = True
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


def run_persistent_session(session, hello_reply):
    """复用已认证连接，按 stdin/stdout JSON Lines 转发多次请求。"""
    print(json.dumps(hello_reply, ensure_ascii=False), flush=True)
    for line_number, line in enumerate(sys.stdin, start=1):
        if not line.strip():
            continue
        try:
            request = json.loads(line)
        except json.JSONDecodeError as error:
            raise RuntimeError(
                f"session stdin line {line_number} is not valid JSON"
            ) from error
        if not isinstance(request, dict):
            raise RuntimeError(f"session stdin line {line_number} must be a JSON object")
        reply = session.send(request)
        print(json.dumps(reply, ensure_ascii=False), flush=True)


def main():
    args = sys.argv[1:]
    if not args:
        sys.exit(__doc__)
    instance_id = None
    if args[:1] == ["--instance"]:
        if len(args) < 3:
            sys.exit("用法: agent_client.py --instance <instance_id> <command>")
        instance_id = args[1]
        args = args[2:]
    command = args[0]
    if command == "apps":
        hub = connect_hub()
        try:
            print(json.dumps({"apps": list_hub_apps(hub)}, ensure_ascii=False))
        finally:
            hub.close()
        return
    endpoint, token, selected_app = select_agent_endpoint(instance_id)
    session = AgentSession(endpoint, token)
    try:
        hello_reply = session.hello()
        if command == "attach":
            public_app = None
            if selected_app is not None:
                public_app = {
                    key: value
                    for key, value in selected_app.items()
                    if key not in ("endpoint", "token")
                }
            print(json.dumps({"app": public_app, "hello": hello_reply}, ensure_ascii=False))
            return
        if command == "hello":
            print(json.dumps(hello_reply, ensure_ascii=False))
            return
        if not hello_reply.get("ok"):
            print(json.dumps(hello_reply, ensure_ascii=False))
            sys.exit(1)
        if command == "session":
            run_persistent_session(session, hello_reply)
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
