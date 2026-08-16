# 使用路径对象读取仓库内的 Wayland clipboard 生命周期契约。
from pathlib import Path
# 使用标准库测试框架保持聚焦验证无额外依赖。
import unittest

# 解析测试文件所在仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 clipboard Module。
CLIPBOARD = ROOT / "src/native/backends/linux/wayland/clipboard.rs"


# 验证 closed backend 不再允许 clipboard 状态复活。
class WaylandClipboardClosedGateTests(unittest.TestCase):
    # 确认 lifecycle gate 只读取 closed 事实并返回 typed error。
    def test_gate_rejects_closed_backend_without_owner_side_effects(self) -> None:
        # 读取 clipboard Module。
        source = CLIPBOARD.read_text(encoding="utf-8")
        # 限定 lifecycle gate。
        gate_start = source.index("fn ensure_clipboard_open")
        # shutdown 端口标记 gate 末尾。
        gate_end = source.index("pub(crate) fn shutdown_clipboard_io", gate_start)
        # 保存 gate 片段。
        gate = source[gate_start:gate_end]
        # gate 必须检查 backend owner 的 closed 事实。
        self.assertIn("if self.closed", gate)
        # closed 必须返回稳定 InvalidState。
        self.assertIn("Errc::InvalidState", gate)
        # 诊断必须保留调用 operation。
        self.assertIn("Wayland clipboard {operation} requested after backend shutdown", gate)
        # 健康 backend 必须继续返回成功。
        self.assertIn("Ok(())", gate)
        # gate 不得取得任何共享 owner lock。
        self.assertNotIn(".lock()", gate)
        # gate 不得向 pending source 入队副作用。
        self.assertNotIn("enqueue", gate)
        # gate 不得修改 closed 事实。
        self.assertNotIn("self.closed =", gate)

    # 确认 text 在读取缓存前执行 lifecycle gate。
    def test_text_checks_gate_before_cache_owner(self) -> None:
        # 读取 clipboard Module。
        source = CLIPBOARD.read_text(encoding="utf-8")
        # 定位 text trait 入口。
        text_start = source.index("fn text(&self)")
        # set_text 标记 text 入口末尾。
        text_end = source.index("fn set_text", text_start)
        # 保存 text 入口片段。
        text = source[text_start:text_end]
        # lifecycle gate 必须是首个 owner 决策。
        gate = text.index('self.ensure_clipboard_open("text")?')
        # 文本 mutex 访问必须随后发生。
        cache_lock = text.index("self.clipboard_text.lock()")
        # gate 必须早于缓存 owner。
        self.assertLess(gate, cache_lock)

    # 确认 set_text 在缓存写入与协议访问前执行 lifecycle gate。
    def test_set_text_checks_gate_before_state_and_protocol(self) -> None:
        # 读取 clipboard Module。
        source = CLIPBOARD.read_text(encoding="utf-8")
        # 定位 set_text trait 入口。
        set_start = source.index("fn set_text")
        # has_text 标记 set_text 入口末尾。
        set_end = source.index("fn has_text", set_start)
        # 保存 set_text 入口片段。
        set_text = source[set_start:set_end]
        # lifecycle gate 必须是首个 owner 决策。
        gate = set_text.index('self.ensure_clipboard_open("set_text")?')
        # 缓存 mutex 是 lifecycle gate 后需要验证的共享 owner 访问。
        cache_lock = set_text.index("self.clipboard_text.lock()")
        # data-device manager 检查必须晚于 gate。
        manager_access = set_text.index("self.data_device_manager")
        # Wayland selection 请求也必须晚于 gate。
        selection_request = set_text.index("set_selection")
        # gate 必须早于缓存 owner。
        self.assertLess(gate, cache_lock)
        # gate 必须早于 data-device manager。
        self.assertLess(gate, manager_access)
        # gate 必须早于协议请求。
        self.assertLess(gate, selection_request)

    # 确认 has_text 在读取缓存前执行 lifecycle gate。
    def test_has_text_checks_gate_before_cache_owner(self) -> None:
        # 读取 clipboard Module。
        source = CLIPBOARD.read_text(encoding="utf-8")
        # 定位 has_text trait 入口到文件末尾。
        has_start = source.index("fn has_text")
        # 保存 has_text 入口片段。
        has_text = source[has_start:]
        # lifecycle gate 必须是首个 owner 决策。
        gate = has_text.index('self.ensure_clipboard_open("has_text")?')
        # 文本 owner lock 必须随后发生。
        cache_lock = has_text.index(".lock()")
        # gate 必须早于缓存 owner。
        self.assertLess(gate, cache_lock)

    # 确认本任务涉及文件满足项目规模门槛。
    def test_touched_file_stays_within_limit(self) -> None:
        # 计算 clipboard Module 当前物理行数。
        line_count = len(CLIPBOARD.read_text(encoding="utf-8").splitlines())
        # 文件必须保持不超过 900 行。
        self.assertLessEqual(line_count, 900)


# 允许直接运行本聚焦测试文件。
if __name__ == "__main__":
    # 使用标准 unittest runner 输出精确用例数。
    unittest.main()
