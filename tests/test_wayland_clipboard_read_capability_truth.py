# 使用路径对象读取 Wayland clipboard 读侧契约。
from pathlib import Path
# 使用标准库测试框架保持聚焦验证无额外依赖。
import unittest

# 解析测试文件所在仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 Wayland clipboard Adapter。
CLIPBOARD = ROOT / "src/native/backends/linux/wayland/clipboard.rs"


# 验证 clipboard 读侧不会把能力缺失伪装为空缓存成功。
class WaylandClipboardReadCapabilityTruthTests(unittest.TestCase):
    # 读取完整 clipboard Module。
    def source(self) -> str:
        # 返回 UTF-8 源码供聚焦契约解析。
        return CLIPBOARD.read_text(encoding="utf-8")

    # 确认统一协议门禁返回稳定 typed errors。
    def test_protocol_gate_distinguishes_global_and_seat_absence(self) -> None:
        # 读取 clipboard Module。
        source = self.source()
        # 定位协议 capability gate。
        start = source.index("fn ensure_clipboard_data_device")
        # shutdown 入口标记 helper 片段终点。
        end = source.index("pub(crate) fn shutdown_clipboard_io", start)
        # 保存统一门禁实现。
        gate = source[start:end]
        # manager 缺失必须返回 capability absence。
        self.assertIn("self.data_device_manager.is_none()", gate)
        # capability absence 使用稳定 NotImplemented。
        self.assertIn("Errc::NotImplemented", gate)
        # 诊断必须保留缺失 global 身份。
        self.assertIn("wl_data_device_manager is unavailable", gate)
        # 当前 seat 缺少 data device 必须独立判断。
        self.assertIn("self.data_device.is_none()", gate)
        # session absence 使用稳定 InvalidOperation。
        self.assertIn("Errc::InvalidOperation", gate)
        # 诊断必须保留 seat/session 上下文。
        self.assertIn("no wl_data_device for active seat", gate)

    # 确认 text 与 has_text 均在 cache lock 前通过两层门禁。
    def test_read_apis_gate_lifecycle_and_protocol_before_cache(self) -> None:
        # 读取 clipboard Module。
        source = self.source()
        # 定位 text 入口。
        text_start = source.index("fn text(&self)")
        # set_text 标记 text 片段终点。
        text_end = source.index("fn set_text", text_start)
        # 保存 text 入口。
        text = source[text_start:text_end]
        # 定位 has_text 入口到文件末尾。
        has_start = source.index("fn has_text")
        # 保存 has_text 入口。
        has_text = source[has_start:]
        # 逐一验证两个读取入口的固定顺序。
        for operation, method in (("text", text), ("has_text", has_text)):
            # 生命周期 gate 必须最先执行。
            lifecycle = method.index(f'self.ensure_clipboard_open("{operation}")?')
            # 协议 capability gate 必须随后执行。
            capability = method.index(f'self.ensure_clipboard_data_device("{operation}")?')
            # 缓存 owner 只能在两层 gate 后访问。
            cache = method.index("self.clipboard_text")
            # 生命周期事实先于协议能力判断。
            self.assertLess(lifecycle, capability)
            # 协议能力判断先于缓存读取。
            self.assertLess(capability, cache)

    # 确认本任务涉及文件满足项目规模门槛。
    def test_touched_files_stay_within_limit(self) -> None:
        # 逐一检查本任务修改的源码与测试文件。
        for path in (CLIPBOARD, Path(__file__)):
            # 计算当前文件物理行数。
            line_count = len(path.read_text(encoding="utf-8").splitlines())
            # 文件必须保持不超过 900 行。
            self.assertLessEqual(line_count, 900, path)


# 允许直接运行本聚焦测试文件。
if __name__ == "__main__":
    # 使用标准 unittest runner 输出精确用例数。
    unittest.main()
