#!/usr/bin/env python3
"""Agent Bridge 示例客户端的窗口解析回归测试。"""

import importlib.util
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "agent_client.py"
SPEC = importlib.util.spec_from_file_location("agent_client", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("无法加载 agent_client.py")
AGENT_CLIENT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(AGENT_CLIENT)


class FakeSession:
    def list_windows(self):
        return {
            "windows": [
                {"window_id": 1, "generation": 3},
                {"window_id": 7, "generation": 9},
            ]
        }


class AgentClientTests(unittest.TestCase):
    def test_explicit_window_uses_reported_generation(self):
        self.assertEqual(AGENT_CLIENT.resolve_window(FakeSession(), 7), (7, 9))


if __name__ == "__main__":
    unittest.main()
