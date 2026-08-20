# 使用路径对象读取仓库内的 Wayland backend 生命周期契约。
from pathlib import Path
# 使用标准库测试框架保持聚焦验证无额外依赖。
import unittest

# 解析测试文件所在仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 Wayland backend 主 Module。
BACKEND = ROOT / "src/native/backends/linux/windowing/wayland/mod.rs"
# 定位运行期失败关闭入口。
EVENT_LOOP = ROOT / "src/native/backends/linux/windowing/wayland/event_loop.rs"


# 验证 backend-global callback owners 的确定性关闭契约。
class WaylandGlobalCallbackShutdownTests(unittest.TestCase):
    # 截取指定函数到下一个同级函数的源码片段。
    def function_source(self, source: str, marker: str, next_marker: str) -> str:
        # 定位目标函数起点。
        start = source.index(marker)
        # 定位下一个同级函数起点。
        end = source.index(next_marker, start)
        # 返回目标函数片段。
        return source[start:end]

    # 确认审计到的全局 quick-assign owners 保持显式且有限。
    def test_backend_global_callback_registry_is_explicit(self) -> None:
        # 读取 backend 主 Module。
        source = BACKEND.read_text(encoding="utf-8")
        # wm-base 必须注册唯一 ping callback。
        self.assertEqual(source.count("_wm_base.quick_assign"), 1)
        # output 枚举必须注册唯一 callback 模板。
        self.assertEqual(source.count("output.quick_assign"), 1)
        # backend 必须持有所有 output callback handles。
        self.assertIn("_wl_outputs: Vec<Main<wl_output::WlOutput>>", source)

    # 确认 teardown 先注销 callbacks，再释放 handles 与显示状态。
    def test_shutdown_clears_callbacks_before_owned_state(self) -> None:
        # 读取 backend 主 Module。
        source = BACKEND.read_text(encoding="utf-8")
        # 截取全局关闭端口。
        shutdown = self.function_source(
            # 提供完整源码。
            source,
            # 目标函数标记。
            "pub(crate) fn shutdown_global_callbacks",
            # 后继函数标记。
            "pub(crate) fn create_wake_pipe",
        )
        # 定位 wm-base callback 注销。
        wm_clear = shutdown.index("self._wm_base.clear_callback()")
        # 定位 output callback 注销。
        output_clear = shutdown.index("output.clear_callback()")
        # 定位 output handles 释放。
        handles_clear = shutdown.index("self._wl_outputs.clear()")
        # 定位显示状态释放。
        state_clear = shutdown.index(".clear();", handles_clear + 1)
        # 注销顺序必须先 wm-base 后 output。
        self.assertLess(wm_clear, output_clear)
        # 所有 callback 必须先于 handle 释放。
        self.assertLess(output_clear, handles_clear)
        # handle 必须先于共享显示状态释放。
        self.assertLess(handles_clear, state_clear)
        # teardown 必须显式恢复中毒锁所有权。
        self.assertIn("unwrap_or_else(|poisoned| poisoned.into_inner())", shutdown)
        # teardown 不得发起任何协议请求。
        self.assertNotIn(".pong(", shutdown)
        # teardown 不得投递 UI 或失败事件。
        self.assertNotIn("enqueue", shutdown)

    # 确认运行期失败按既定组件顺序关闭全局回调。
    def test_runtime_failure_wires_global_shutdown_last(self) -> None:
        # 读取运行期 event-loop Module。
        source = EVENT_LOOP.read_text(encoding="utf-8")
        # 截取致命失败关闭端口。
        shutdown = self.function_source(
            # 提供完整源码。
            source,
            # 目标函数标记。
            "fn close_after_failure",
            # 后继函数标记。
            "pub(crate) fn dispatch_pending_checked",
        )
        # 定位 text-input teardown。
        text_input = shutdown.index("self.shutdown_text_input()")
        # 定位 seat/input teardown。
        seat = shutdown.index("self.shutdown_seat_and_input()")
        # 定位 clipboard teardown。
        clipboard = shutdown.index("self.shutdown_clipboard_io()")
        # 定位 global callback teardown。
        globals_shutdown = shutdown.index("self.shutdown_global_callbacks()")
        # 关闭顺序必须反映 callback 的依赖层次。
        self.assertLess(text_input, seat)
        # seat/input 必须先于 clipboard I/O owners 停止。
        self.assertLess(seat, clipboard)
        # clipboard 必须先于 backend-global callbacks 停止。
        self.assertLess(clipboard, globals_shutdown)

    # 确认 Drop 在关闭 failure source 前注销全局 callbacks。
    def test_drop_wires_global_shutdown_before_failure_source(self) -> None:
        # 读取 backend 主 Module。
        source = BACKEND.read_text(encoding="utf-8")
        # 截取 Drop 实现到文件末尾。
        drop_impl = source[source.index("impl Drop for WaylandBackend") :]
        # 定位 clipboard teardown。
        clipboard = drop_impl.index("self.shutdown_clipboard_io()")
        # 定位 global callback teardown。
        globals_shutdown = drop_impl.index("self.shutdown_global_callbacks()")
        # 定位 failure source 关闭。
        failure_source = drop_impl.index("self.pending_failures.close()")
        # clipboard owner 必须先停止。
        self.assertLess(clipboard, globals_shutdown)
        # failure source 必须保持到所有 callback 都已注销。
        self.assertLess(globals_shutdown, failure_source)

    # 确认本任务涉及文件满足项目规模门槛。
    def test_touched_files_stay_within_limit(self) -> None:
        # 逐一检查本任务修改的源码与测试文件。
        for path in (BACKEND, EVENT_LOOP, Path(__file__)):
            # 计算当前文件物理行数。
            line_count = len(path.read_text(encoding="utf-8").splitlines())
            # 文件必须保持不超过 900 行。
            self.assertLessEqual(line_count, 900, path)


# 允许直接运行本聚焦测试文件。
if __name__ == "__main__":
    # 使用标准 unittest runner 输出精确用例数。
    unittest.main()
