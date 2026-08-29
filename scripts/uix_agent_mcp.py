#!/usr/bin/env python3
"""把 UIX Agent Hub 暴露为依赖为零的本地 STDIO MCP 服务。"""

from __future__ import annotations

import base64
import binascii
import concurrent.futures
import json
import os
import secrets
import sys
import tempfile
import threading

from agent_client import AgentSession, attach_hub_app, connect_hub, list_hub_apps

MCP_SERVER_NAME = "uix-agent-hub"
MCP_SERVER_VERSION = "0.1.0"
MAX_MCP_LINE_BYTES = 4 * 1024 * 1024
MAX_CONCURRENT_CALLS = 16
MAX_WAIT_TIMEOUT_MS = 30_000

sys.stdout.reconfigure(encoding="utf-8")
sys.stderr.reconfigure(encoding="utf-8")


def compact_json(value):
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"))


def text_result(value, is_error=False):
    return {
        "content": [{"type": "text", "text": compact_json(value)}],
        "isError": is_error,
    }


def require_int(arguments, name, minimum=0, maximum=None):
    value = arguments.get(name)
    if not isinstance(value, int) or isinstance(value, bool) or value < minimum:
        raise ValueError(f"{name} must be an integer >= {minimum}")
    if maximum is not None and value > maximum:
        raise ValueError(f"{name} must be <= {maximum}")
    return value


def require_text(arguments, name, maximum=256):
    value = arguments.get(name)
    if not isinstance(value, str) or not value or len(value) > maximum:
        raise ValueError(f"{name} must be a non-empty string up to {maximum} characters")
    return value


def protocol_result(reply):
    if reply.get("ok"):
        return text_result(reply)
    error = reply.get("error")
    if not isinstance(error, dict):
        error = {"code": "invalid_agent_error"}
        reply = {**reply, "error": error}
    code = error.get("code")
    if code == "timeout":
        error.setdefault("retry_safe", True)
    else:
        error.setdefault("retry_safe", False)
    return text_result(reply, is_error=True)


class AgentLane:
    """一个逻辑流独占一个 Agent 连接，避免 wait 或截屏阻塞控制动作。"""

    def __init__(self, endpoint, token):
        self.endpoint = endpoint
        self.token = token
        self.lock = threading.Lock()
        self.session = None
        self.broken = False

    def send(self, payload):
        with self.lock:
            if self.broken:
                raise RuntimeError("app session lane is terminal; attach the instance again")
            if self.session is None:
                session = AgentSession(self.endpoint, self.token)
                hello = session.hello()
                if not hello.get("ok"):
                    session.close()
                    raise RuntimeError(f"agent hello failed: {hello.get('error')}")
                self.session = session
            try:
                return self.session.send(payload)
            except Exception:
                # 发送失败后动作结果可能未知；绝不在新连接上自动重放。
                self.broken = True
                self.session.close()
                self.session = None
                raise

    def close(self):
        with self.lock:
            self.broken = True
            if self.session is not None:
                self.session.close()
                self.session = None


class AppBundle:
    def __init__(self, app):
        self.session_id = secrets.token_urlsafe(18)
        self.app = {
            key: value for key, value in app.items() if key not in ("endpoint", "token")
        }
        endpoint = app["endpoint"]
        token = app["token"]
        self.control = AgentLane(endpoint, token)
        self.wait = AgentLane(endpoint, token)
        self.media = AgentLane(endpoint, token)
        self.write_gate = threading.Lock()

    def close(self):
        self.control.close()
        self.wait.close()
        self.media.close()


class UixMcpServer:
    def __init__(self):
        self.stdout_lock = threading.Lock()
        self.hub_lock = threading.Lock()
        self.hub = connect_hub()
        self.sessions_lock = threading.Lock()
        self.sessions = {}
        self.call_slots = threading.BoundedSemaphore(MAX_CONCURRENT_CALLS)
        self.executor = concurrent.futures.ThreadPoolExecutor(
            max_workers=4, thread_name_prefix="uix-mcp"
        )
        self.capture_dir = tempfile.mkdtemp(prefix="uix-agent-mcp-")
        os.chmod(self.capture_dir, 0o700)

    def write_message(self, message):
        encoded = compact_json(message)
        with self.stdout_lock:
            sys.stdout.write(encoded + "\n")
            sys.stdout.flush()

    def hub_call(self, request):
        with self.hub_lock:
            return self.hub.send(request)

    def get_bundle(self, arguments):
        session_id = require_text(arguments, "session_id", 128)
        with self.sessions_lock:
            bundle = self.sessions.get(session_id)
        if bundle is None:
            raise ValueError("session_id is unknown or detached")
        return bundle

    def initialize(self, params):
        protocol_version = params.get("protocolVersion")
        if not isinstance(protocol_version, str) or not protocol_version:
            protocol_version = "2024-11-05"
        return {
            "protocolVersion": protocol_version,
            "capabilities": {"tools": {"listChanged": False}},
            "serverInfo": {"name": MCP_SERVER_NAME, "version": MCP_SERVER_VERSION},
            "instructions": (
                "先调用 uix_list_apps，再用 instance_id 调用 uix_attach_app。"
                "所有界面调用都必须携带返回的 session_id；应用退出或重启后旧会话终止，"
                "不得自动改绑另一个实例。动作出现 outcome_unknown 或传输失败时先读状态，"
                "禁止盲目重试。后台动作可用；无可呈现 surface 时只读语义快照，不得伪造截图。"
                "优先用 uix_interact 完成动作、呈现等待和快照。"
            ),
        }

    def tools(self):
        session_property = {
            "session_id": {"type": "string", "description": "attach 返回的实例绑定会话"}
        }
        window_properties = {
            **session_property,
            "window_id": {"type": "integer", "minimum": 1},
            "generation": {"type": "integer", "minimum": 0},
        }
        action_properties = {
            **window_properties,
            "target": {"type": "object"},
            "action": {"type": "object"},
            "expected_revision": {"type": "integer", "minimum": 0},
        }
        return [
            {
                "name": "uix_list_apps",
                "description": "列出当前用户 Hub 中所有 UIX 应用实例，不含端点和凭据。",
                "inputSchema": {"type": "object", "properties": {}, "additionalProperties": False},
                "annotations": {"readOnlyHint": True, "idempotentHint": True},
            },
            {
                "name": "uix_attach_app",
                "description": "显式绑定一个 instance_id，返回后续工具必需的 session_id。",
                "inputSchema": {
                    "type": "object",
                    "properties": {"instance_id": {"type": "string"}},
                    "required": ["instance_id"],
                    "additionalProperties": False,
                },
                "annotations": {"readOnlyHint": True, "idempotentHint": False},
            },
            {
                "name": "uix_detach_app",
                "description": "关闭一个实例绑定会话及其三条本地连接。",
                "inputSchema": {
                    "type": "object",
                    "properties": session_property,
                    "required": ["session_id"],
                    "additionalProperties": False,
                },
                "annotations": {"readOnlyHint": True, "idempotentHint": True},
            },
            {
                "name": "uix_list_windows",
                "description": "列出已绑定应用实例的窗口和真实 generation。",
                "inputSchema": {
                    "type": "object",
                    "properties": session_property,
                    "required": ["session_id"],
                    "additionalProperties": False,
                },
                "annotations": {"readOnlyHint": True, "idempotentHint": True},
            },
            {
                "name": "uix_snapshot",
                "description": "读取指定窗口的当前语义快照。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        **session_property,
                        "window_id": {"type": "integer", "minimum": 1},
                    },
                    "required": ["session_id", "window_id"],
                    "additionalProperties": False,
                },
                "annotations": {"readOnlyHint": True, "idempotentHint": True},
            },
            {
                "name": "uix_perform",
                "description": "执行一个语义动作或窗口动作；失败时不会自动重放。",
                "inputSchema": {
                    "type": "object",
                    "properties": action_properties,
                    "required": ["session_id", "window_id", "generation", "action"],
                    "additionalProperties": False,
                },
                "annotations": {"readOnlyHint": False, "idempotentHint": False},
            },
            {
                "name": "uix_interact",
                "description": "完成 perform、呈现等待和快照；后台无 surface 时保留动作并回退语义快照。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        **action_properties,
                        "wait_timeout_ms": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": MAX_WAIT_TIMEOUT_MS,
                        },
                    },
                    "required": ["session_id", "window_id", "generation", "action"],
                    "additionalProperties": False,
                },
                "annotations": {"readOnlyHint": False, "idempotentHint": False},
            },
            {
                "name": "uix_wait",
                "description": "在独立等待流中等待 revision 前进或 presented_revision 达标。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        **window_properties,
                        "after_revision": {"type": "integer", "minimum": 0},
                        "presented_revision": {"type": "integer", "minimum": 0},
                        "timeout_ms": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": MAX_WAIT_TIMEOUT_MS,
                        },
                    },
                    "required": ["session_id", "window_id", "generation", "timeout_ms"],
                    "additionalProperties": False,
                },
                "annotations": {"readOnlyHint": True, "idempotentHint": True},
            },
            {
                "name": "uix_confirm",
                "description": "提交应用内确认流程的一次性 confirm_id。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        **window_properties,
                        "confirm_id": {"type": "string"},
                    },
                    "required": ["session_id", "window_id", "generation", "confirm_id"],
                    "additionalProperties": False,
                },
                "annotations": {"readOnlyHint": False, "idempotentHint": False},
            },
            {
                "name": "uix_screenshot",
                "description": "在独立媒体流截屏并写入 0600 临时 PNG；无可呈现 surface 时明确失败。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        **session_property,
                        "window_id": {"type": "integer", "minimum": 1},
                    },
                    "required": ["session_id", "window_id"],
                    "additionalProperties": False,
                },
                "annotations": {"readOnlyHint": True, "idempotentHint": False},
            },
        ]

    def call_tool(self, name, arguments):
        if not isinstance(arguments, dict):
            raise ValueError("tool arguments must be an object")
        if name == "uix_list_apps":
            reply = self.hub_call({"type": "list_apps"})
            if not reply.get("ok"):
                return text_result(reply, is_error=True)
            return text_result({"apps": reply.get("apps") or []})
        if name == "uix_attach_app":
            instance_id = require_text(arguments, "instance_id", 128)
            with self.hub_lock:
                app = attach_hub_app(self.hub, instance_id)
            bundle = AppBundle(app)
            # attach 必须先真实握手，不能仅凭 Hub 中的旧登记宣称成功。
            hello = bundle.control.send({"type": "list_windows"})
            if not hello.get("ok"):
                bundle.close()
                return protocol_result(hello)
            with self.sessions_lock:
                self.sessions[bundle.session_id] = bundle
            return text_result(
                {"session_id": bundle.session_id, "app": bundle.app, "windows": hello.get("windows") or []}
            )
        if name == "uix_detach_app":
            session_id = require_text(arguments, "session_id", 128)
            with self.sessions_lock:
                bundle = self.sessions.pop(session_id, None)
            if bundle is not None:
                bundle.close()
            return text_result({"detached": bundle is not None, "session_id": session_id})

        bundle = self.get_bundle(arguments)
        if name == "uix_list_windows":
            return protocol_result(bundle.control.send({"type": "list_windows"}))
        if name == "uix_snapshot":
            window_id = require_int(arguments, "window_id", 1)
            return protocol_result(bundle.control.send({"type": "snapshot", "window_id": window_id}))
        if name in ("uix_perform", "uix_interact"):
            return self.perform(bundle, arguments, interact=name == "uix_interact")
        if name == "uix_wait":
            return self.wait(bundle, arguments)
        if name == "uix_confirm":
            request = {
                "type": "confirm",
                "window_id": require_int(arguments, "window_id", 1),
                "generation": require_int(arguments, "generation"),
                "confirm_id": require_text(arguments, "confirm_id", 256),
            }
            with bundle.write_gate:
                return protocol_result(bundle.control.send(request))
        if name == "uix_screenshot":
            return self.screenshot(bundle, arguments)
        raise ValueError(f"unknown tool: {name}")

    def perform(self, bundle, arguments, interact):
        action = arguments.get("action")
        if not isinstance(action, dict):
            raise ValueError("action must be an object")
        request = {
            "type": "perform",
            "window_id": require_int(arguments, "window_id", 1),
            "generation": require_int(arguments, "generation"),
            "action": action,
        }
        target = arguments.get("target")
        if target is not None:
            if not isinstance(target, dict):
                raise ValueError("target must be an object")
            request["target"] = target
        if "expected_revision" in arguments:
            request["expected_revision"] = require_int(arguments, "expected_revision")
        with bundle.write_gate:
            performed = bundle.control.send(request)
            if not interact or not performed.get("ok"):
                return protocol_result(performed)
            revision = performed.get("revision")
            if not isinstance(revision, int) or isinstance(revision, bool):
                return text_result(
                    {
                        "ok": True,
                        "action_performed": True,
                        "perform": performed,
                        "observation": {"ok": False, "error": {"code": "missing_revision"}},
                        "retry_action": False,
                    }
                )
            timeout_ms = arguments.get("wait_timeout_ms", 1000)
            if not isinstance(timeout_ms, int) or isinstance(timeout_ms, bool):
                raise ValueError("wait_timeout_ms must be an integer")
            if timeout_ms < 1 or timeout_ms > MAX_WAIT_TIMEOUT_MS:
                raise ValueError(f"wait_timeout_ms must be between 1 and {MAX_WAIT_TIMEOUT_MS}")
            try:
                waited = bundle.wait.send(
                    {
                        "type": "wait",
                        "window_id": request["window_id"],
                        "generation": request["generation"],
                        "presented_revision": revision,
                        "timeout_ms": timeout_ms,
                    }
                )
            except Exception as error:
                return text_result(
                    {
                        "ok": True,
                        "action_performed": True,
                        "perform": performed,
                        "observation": {
                            "ok": False,
                            "error": {"code": "connector_error", "message": str(error)},
                        },
                        "retry_action": False,
                    }
                )
            if not waited.get("ok"):
                try:
                    snapshot = bundle.control.send(
                        {"type": "snapshot", "window_id": request["window_id"]}
                    )
                except Exception as error:
                    snapshot = {
                        "ok": False,
                        "error": {"code": "connector_error", "message": str(error)},
                    }
                return text_result(
                    {
                        "ok": True,
                        "action_performed": True,
                        "perform": performed,
                        "wait": waited,
                        "snapshot": snapshot,
                        "retry_action": False,
                    }
                )
            try:
                snapshot = bundle.control.send(
                    {"type": "snapshot", "window_id": request["window_id"]}
                )
            except Exception as error:
                snapshot = {
                    "ok": False,
                    "error": {"code": "connector_error", "message": str(error)},
                }
            return text_result(
                {
                    "ok": True,
                    "action_performed": True,
                    "perform": performed,
                    "wait": waited,
                    "snapshot": snapshot,
                    "retry_action": False,
                }
            )

    def wait(self, bundle, arguments):
        request = {
            "type": "wait",
            "window_id": require_int(arguments, "window_id", 1),
            "generation": require_int(arguments, "generation"),
            "timeout_ms": require_int(
                arguments, "timeout_ms", 1, MAX_WAIT_TIMEOUT_MS
            ),
        }
        conditions = [name for name in ("after_revision", "presented_revision") if name in arguments]
        if len(conditions) != 1:
            raise ValueError("exactly one of after_revision or presented_revision is required")
        condition = conditions[0]
        request[condition] = require_int(arguments, condition)
        return protocol_result(bundle.wait.send(request))

    def screenshot(self, bundle, arguments):
        window_id = require_int(arguments, "window_id", 1)
        reply = bundle.media.send({"type": "screenshot", "window_id": window_id})
        if not reply.get("ok"):
            return protocol_result(reply)
        encoded = reply.get("data_base64")
        if not isinstance(encoded, str):
            raise RuntimeError("agent screenshot response is missing data_base64")
        try:
            data = base64.b64decode(encoded, validate=True)
        except (ValueError, binascii.Error) as error:
            raise RuntimeError("agent screenshot is not valid base64") from error
        if len(data) > bundle.media.session.max_screenshot_bytes:
            raise RuntimeError("agent screenshot exceeds the negotiated protocol limit")
        descriptor, path = tempfile.mkstemp(
            prefix=f"window-{window_id}-", suffix=".png", dir=self.capture_dir
        )
        try:
            os.fchmod(descriptor, 0o600)
            with os.fdopen(descriptor, "wb") as output:
                output.write(data)
        except Exception:
            try:
                os.close(descriptor)
            except OSError:
                pass
            raise
        return text_result(
            {
                "ok": True,
                "window_id": reply.get("window_id"),
                "width": reply.get("width"),
                "height": reply.get("height"),
                "format": reply.get("format"),
                "bytes": len(data),
                "path": path,
            }
        )

    def dispatch(self, message):
        request_id = message.get("id")
        method = message.get("method")
        params = message.get("params")
        if params is None:
            params = {}
        if not isinstance(params, dict):
            raise ValueError("JSON-RPC params must be an object")
        if method == "initialize":
            return {"jsonrpc": "2.0", "id": request_id, "result": self.initialize(params)}
        if method == "ping":
            return {"jsonrpc": "2.0", "id": request_id, "result": {}}
        if method == "tools/list":
            return {"jsonrpc": "2.0", "id": request_id, "result": {"tools": self.tools()}}
        if method == "tools/call":
            if not self.call_slots.acquire(blocking=False):
                result = text_result(
                    {"ok": False, "error": {"code": "server_busy", "retry_safe": True}},
                    is_error=True,
                )
            else:
                try:
                    result = self.call_tool(params.get("name"), params.get("arguments") or {})
                finally:
                    self.call_slots.release()
            return {"jsonrpc": "2.0", "id": request_id, "result": result}
        return {
            "jsonrpc": "2.0",
            "id": request_id,
            "error": {"code": -32601, "message": "Method not found"},
        }

    def dispatch_safely(self, message):
        request_id = message.get("id") if isinstance(message, dict) else None
        try:
            response = self.dispatch(message)
        except (TypeError, ValueError, RuntimeError, OSError) as error:
            response = {
                "jsonrpc": "2.0",
                "id": request_id,
                "result": text_result(
                    {
                        "ok": False,
                        "error": {
                            "code": "connector_error",
                            "message": str(error),
                            "retry_safe": False,
                        },
                    },
                    is_error=True,
                ),
            }
        self.write_message(response)

    def run(self):
        for raw in sys.stdin.buffer:
            if len(raw) > MAX_MCP_LINE_BYTES:
                self.write_message(
                    {
                        "jsonrpc": "2.0",
                        "id": None,
                        "error": {"code": -32600, "message": "MCP request is too large"},
                    }
                )
                continue
            try:
                message = json.loads(raw.decode("utf-8"))
            except (UnicodeDecodeError, json.JSONDecodeError):
                self.write_message(
                    {"jsonrpc": "2.0", "id": None, "error": {"code": -32700, "message": "Parse error"}}
                )
                continue
            if not isinstance(message, dict) or message.get("jsonrpc") != "2.0":
                self.write_message(
                    {"jsonrpc": "2.0", "id": None, "error": {"code": -32600, "message": "Invalid Request"}}
                )
                continue
            if "id" not in message:
                # initialized / cancelled 等通知没有回复；现有 Agent 请求仍受自身超时约束。
                continue
            self.executor.submit(self.dispatch_safely, message)

    def close(self):
        self.executor.shutdown(wait=True, cancel_futures=True)
        with self.sessions_lock:
            bundles = list(self.sessions.values())
            self.sessions.clear()
        for bundle in bundles:
            bundle.close()
        self.hub.close()


def main():
    server = UixMcpServer()
    try:
        server.run()
    finally:
        server.close()


if __name__ == "__main__":
    main()
