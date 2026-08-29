#!/usr/bin/env python3
"""UIX Agent Hub：维护当前用户的应用实例注册表。"""

from __future__ import annotations

import json
import os
import signal
import socket
import stat
import sys
import tempfile
import threading
import time
from dataclasses import dataclass

if os.name != "nt":
    import fcntl

HUB_SCHEMA = "uix.agent.hub.v1"
MAX_LINE_BYTES = 128 * 1024
REGISTRY_SETTLE_SECONDS = 0.25

if os.name == "nt":
    import pywintypes
    import win32api
    import win32con
    import win32event
    import win32file
    import win32pipe
    import win32security


def compact_json(value):
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"))


def current_user_sid():
    token = win32security.OpenProcessToken(
        win32api.GetCurrentProcess(), win32con.TOKEN_QUERY
    )
    try:
        sid = win32security.GetTokenInformation(token, win32security.TokenUser)[0]
    finally:
        win32api.CloseHandle(token)
    return sid, win32security.ConvertSidToStringSid(sid)


def hub_endpoint():
    if os.name == "nt":
        _, sid_text = current_user_sid()
        return rf"\\.\pipe\uix-agent-hub-{sid_text}"
    runtime_dir = os.environ.get("XDG_RUNTIME_DIR") or tempfile.gettempdir()
    return os.path.join(runtime_dir, f"uix-agent-{os.getuid()}", "hub-v1.sock")


def validate_text(value, name, maximum=512):
    if not isinstance(value, str) or not value or len(value) > maximum:
        raise ValueError(f"{name} must be a non-empty string up to {maximum} characters")
    if any(ord(character) < 0x20 for character in value):
        raise ValueError(f"{name} contains control characters")
    return value


def validate_registration(message):
    if message.get("schema") != HUB_SCHEMA or message.get("type") != "register_app":
        raise ValueError("first application frame must be register_app")
    process_id = message.get("process_id")
    if not isinstance(process_id, int) or isinstance(process_id, bool) or process_id <= 0:
        raise ValueError("process_id must be a positive integer")
    return {
        "app_id": validate_text(message.get("app_id"), "app_id", 256),
        "display_name": validate_text(message.get("display_name"), "display_name", 512),
        "instance_id": validate_text(message.get("instance_id"), "instance_id", 128),
        "process_id": process_id,
        "endpoint": validate_text(message.get("endpoint"), "endpoint", 4096),
        "token": validate_text(message.get("token"), "token", 512),
    }


@dataclass
class AppRecord:
    app_id: str
    display_name: str
    instance_id: str
    process_id: int
    endpoint: str
    token: str
    owner: object
    registered_at: float
    last_seen: float

    def public(self):
        return {
            "app_id": self.app_id,
            "display_name": self.display_name,
            "instance_id": self.instance_id,
            "process_id": self.process_id,
            "state": "ready",
        }


class AppRegistry:
    """以随机实例身份持有登记；断线只删除属于该连接的记录。"""

    def __init__(self):
        self._lock = threading.Lock()
        self._apps = {}
        self._revision = 0
        self._last_changed = time.monotonic()

    def register(self, fields, owner):
        now = time.monotonic()
        record = AppRecord(**fields, owner=owner, registered_at=now, last_seen=now)
        with self._lock:
            self._apps[record.instance_id] = record
            self._revision += 1
            self._last_changed = now
        return record

    def heartbeat(self, instance_id, owner):
        with self._lock:
            record = self._apps.get(instance_id)
            if record is not None and record.owner is owner:
                record.last_seen = time.monotonic()

    def remove(self, instance_id, owner):
        with self._lock:
            record = self._apps.get(instance_id)
            if record is not None and record.owner is owner:
                del self._apps[instance_id]
                self._revision += 1
                self._last_changed = time.monotonic()

    def snapshot(self):
        with self._lock:
            records = sorted(
                self._apps.values(),
                key=lambda record: (record.app_id, record.process_id, record.instance_id),
            )
            return {
                "apps": [record.public() for record in records],
                "registry_revision": self._revision,
                "stable": time.monotonic() - self._last_changed >= REGISTRY_SETTLE_SECONDS,
            }

    def attach(self, instance_id):
        with self._lock:
            record = self._apps.get(instance_id)
            if record is None:
                return None
            # 凭据只在显式 attach 后交给同用户控制器，不进入 list 或日志。
            return {
                **record.public(),
                "endpoint": record.endpoint,
                "token": record.token,
            }


REGISTRY = AppRegistry()


def parse_line(raw):
    if len(raw) > MAX_LINE_BYTES:
        raise ValueError("hub frame exceeds the maximum size")
    try:
        message = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError("hub frame is not valid UTF-8 JSON") from error
    if not isinstance(message, dict):
        raise ValueError("hub frame must be a JSON object")
    return message


def reply_for(message):
    request_id = message.get("request_id")
    base = {"schema": HUB_SCHEMA, "request_id": request_id}
    if message.get("schema") != HUB_SCHEMA:
        return {**base, "ok": False, "error": {"code": "schema_mismatch"}}
    message_type = message.get("type")
    if message_type == "ping":
        return {**base, "ok": True, "state": "ready"}
    if message_type == "list_apps":
        return {**base, "ok": True, **REGISTRY.snapshot()}
    if message_type == "attach":
        instance_id = message.get("instance_id")
        if not isinstance(instance_id, str) or not instance_id:
            return {**base, "ok": False, "error": {"code": "invalid_instance_id"}}
        app = REGISTRY.attach(instance_id)
        if app is None:
            return {
                **base,
                "ok": False,
                "error": {"code": "app_instance_not_found", "retry_safe": False},
            }
        return {**base, "ok": True, "app": app}
    return {**base, "ok": False, "error": {"code": "unknown_request"}}


def handle_connection(stream):
    owner = object()
    instance_id = None
    try:
        raw = stream.readline(MAX_LINE_BYTES + 1)
        if not raw:
            return
        first = parse_line(raw.rstrip(b"\r\n"))
        if first.get("type") == "register_app":
            record = REGISTRY.register(validate_registration(first), owner)
            instance_id = record.instance_id
            while True:
                raw = stream.readline(MAX_LINE_BYTES + 1)
                if not raw:
                    return
                message = parse_line(raw.rstrip(b"\r\n"))
                if message.get("schema") != HUB_SCHEMA or message.get("type") != "heartbeat":
                    return
                REGISTRY.heartbeat(instance_id, owner)
        else:
            stream.write_json(reply_for(first))
            while True:
                raw = stream.readline(MAX_LINE_BYTES + 1)
                if not raw:
                    return
                stream.write_json(reply_for(parse_line(raw.rstrip(b"\r\n"))))
    except (BrokenPipeError, ConnectionError, OSError, ValueError):
        return
    finally:
        if instance_id is not None:
            REGISTRY.remove(instance_id, owner)
        stream.close()


class SocketStream:
    def __init__(self, connection):
        self.connection = connection
        self.reader = connection.makefile("rb")
        self.writer = connection.makefile("wb")

    def readline(self, maximum):
        return self.reader.readline(maximum)

    def write_json(self, value):
        self.writer.write(compact_json(value).encode("utf-8") + b"\n")
        self.writer.flush()

    def close(self):
        try:
            self.reader.close()
        finally:
            try:
                self.writer.close()
            finally:
                self.connection.close()


class PipeStream:
    def __init__(self, handle):
        self.handle = handle
        self.buffer = bytearray()

    def readline(self, maximum):
        while True:
            newline = self.buffer.find(b"\n")
            if newline >= 0:
                raw = bytes(self.buffer[: newline + 1])
                del self.buffer[: newline + 1]
                return raw
            if len(self.buffer) >= maximum:
                return bytes(self.buffer)
            try:
                _, data = win32file.ReadFile(self.handle, min(65536, maximum - len(self.buffer)))
            except pywintypes.error as error:
                if error.winerror in (109, 232):
                    return b""
                raise
            self.buffer.extend(data)

    def write_json(self, value):
        win32file.WriteFile(self.handle, compact_json(value).encode("utf-8") + b"\n")

    def close(self):
        try:
            win32pipe.DisconnectNamedPipe(self.handle)
        except pywintypes.error:
            pass
        win32file.CloseHandle(self.handle)


def ensure_unix_directory(path):
    directory = os.path.dirname(path)
    try:
        os.mkdir(directory, 0o700)
    except FileExistsError:
        pass
    metadata = os.lstat(directory)
    if not stat.S_ISDIR(metadata.st_mode) or stat.S_ISLNK(metadata.st_mode):
        raise RuntimeError("hub parent path is not a private directory")
    if metadata.st_uid != os.geteuid():
        raise RuntimeError("hub parent directory has a different owner")
    os.chmod(directory, 0o700)


def unix_hub_is_running(path):
    if not os.path.exists(path):
        return False
    probe = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    try:
        probe.settimeout(0.2)
        probe.connect(path)
        return True
    except OSError:
        return False
    finally:
        probe.close()


def serve_unix():
    path = hub_endpoint()
    ensure_unix_directory(path)
    lock_path = path + ".lock"
    lock_descriptor = os.open(lock_path, os.O_CREAT | os.O_RDWR, 0o600)
    os.fchmod(lock_descriptor, 0o600)
    try:
        fcntl.flock(lock_descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        os.close(lock_descriptor)
        return 0
    if unix_hub_is_running(path):
        os.close(lock_descriptor)
        return 0
    if os.path.lexists(path):
        metadata = os.lstat(path)
        if not stat.S_ISSOCK(metadata.st_mode) or metadata.st_uid != os.geteuid():
            raise RuntimeError("refusing to replace a non-owned hub endpoint")
        os.unlink(path)
    listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    listener.bind(path)
    os.chmod(path, 0o600)
    listener.listen(64)
    stopping = threading.Event()

    def stop(_signum, _frame):
        stopping.set()
        listener.close()

    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)
    try:
        while not stopping.is_set():
            try:
                connection, _ = listener.accept()
            except OSError:
                if stopping.is_set():
                    break
                raise
            if not unix_peer_is_current_user(connection):
                connection.close()
                continue
            threading.Thread(
                target=handle_connection,
                args=(SocketStream(connection),),
                name="uix-agent-hub-connection",
                daemon=True,
            ).start()
    finally:
        listener.close()
        if os.path.lexists(path) and stat.S_ISSOCK(os.lstat(path).st_mode):
            os.unlink(path)
        os.close(lock_descriptor)
    return 0


def unix_peer_is_current_user(connection):
    if hasattr(socket, "SO_PEERCRED"):
        import struct

        credentials = connection.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12)
        _, uid, _ = struct.unpack("3i", credentials)
        return uid == os.geteuid()
    if hasattr(connection, "getpeereid"):
        uid, _ = connection.getpeereid()
        return uid == os.geteuid()
    return False


def windows_security_attributes():
    sid, _ = current_user_sid()
    descriptor = win32security.SECURITY_DESCRIPTOR()
    descriptor.SetSecurityDescriptorOwner(sid, False)
    acl = win32security.ACL()
    acl.AddAccessAllowedAce(
        win32security.ACL_REVISION,
        win32con.GENERIC_READ | win32con.GENERIC_WRITE,
        sid,
    )
    descriptor.SetSecurityDescriptorDacl(True, acl, False)
    attributes = pywintypes.SECURITY_ATTRIBUTES()
    attributes.SECURITY_DESCRIPTOR = descriptor
    return attributes


def serve_windows():
    _, sid_text = current_user_sid()
    mutex = win32event.CreateMutex(
        windows_security_attributes(), False, f"Local\\uix-agent-hub-{sid_text}"
    )
    if win32api.GetLastError() == win32con.ERROR_ALREADY_EXISTS:
        win32api.CloseHandle(mutex)
        return 0
    pipe_name = hub_endpoint()
    attributes = windows_security_attributes()
    try:
        while True:
            handle = win32pipe.CreateNamedPipe(
                pipe_name,
                win32pipe.PIPE_ACCESS_DUPLEX,
                win32pipe.PIPE_TYPE_BYTE | win32pipe.PIPE_READMODE_BYTE | win32pipe.PIPE_WAIT,
                win32pipe.PIPE_UNLIMITED_INSTANCES,
                65536,
                65536,
                0,
                attributes,
            )
            try:
                win32pipe.ConnectNamedPipe(handle, None)
            except pywintypes.error as error:
                if error.winerror != 535:
                    win32file.CloseHandle(handle)
                    raise
            threading.Thread(
                target=handle_connection,
                args=(PipeStream(handle),),
                name="uix-agent-hub-connection",
                daemon=True,
            ).start()
    finally:
        win32api.CloseHandle(mutex)


def main():
    command = sys.argv[1] if len(sys.argv) > 1 else "serve"
    if command == "endpoint":
        print(hub_endpoint())
        return 0
    if command != "serve":
        raise SystemExit("用法: uix_agent_hub.py [serve|endpoint]")
    return serve_windows() if os.name == "nt" else serve_unix()


if __name__ == "__main__":
    raise SystemExit(main())
