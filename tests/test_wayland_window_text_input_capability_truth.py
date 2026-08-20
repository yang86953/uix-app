# 使用路径对象读取窗口与真实文本输入契约。
from pathlib import Path
# 使用标准库测试框架保持聚焦验证无额外依赖。
import unittest

# 解析测试文件所在仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 Wayland WindowBackend Adapter。
WAYLAND_WINDOW_OPS = ROOT / "src/native/backends/linux/windowing/wayland/window_ops.rs"
# 定位 Wayland TextInputSession Adapter。
WAYLAND_TEXT_INPUT = ROOT / "src/native/backends/linux/windowing/wayland/text_input.rs"
# 定位共享 PlatformWindow 事务实现。
SHARED_WINDOW = ROOT / "src/native/windowing/shared/window.rs"


# 验证 Wayland 旧窗口文本输入入口不再伪装成功。
class WaylandWindowTextInputCapabilityTruthTests(unittest.TestCase):
    # 确认旧 start/stop 入口返回稳定能力错误。
    def test_legacy_window_entry_points_return_typed_capability_errors(self) -> None:
        # 读取 Wayland WindowBackend Adapter。
        source = WAYLAND_WINDOW_OPS.read_text(encoding="utf-8")
        # 定位旧 start 入口。
        start_begin = source.index("fn os_start_text_input")
        # 旧 stop 入口标记 start 片段终点。
        stop_begin = source.index("fn os_stop_text_input", start_begin)
        # file-drop 入口标记 stop 片段终点。
        stop_end = source.index("fn os_enable_file_drop", stop_begin)
        # 保存旧 start 入口片段。
        start = source[start_begin:stop_begin]
        # 保存旧 stop 入口片段。
        stop = source[stop_begin:stop_end]
        # 两个旧入口都必须使用稳定 NotImplemented。
        self.assertIn("Errc::NotImplemented", start)
        # stop 保持相同能力分类。
        self.assertIn("Errc::NotImplemented", stop)
        # start 诊断必须指向真正的能力根入口。
        self.assertIn("Platform::text_input().start()", start)
        # stop 诊断必须指向真正的能力根入口。
        self.assertIn("Platform::text_input().stop()", stop)
        # 旧入口不得再返回伪成功。
        self.assertNotIn("Ok(())", start + stop)

    # 确认真实 TextInputSession 与共享状态事务仍保持原有边界。
    def test_real_text_input_path_and_state_transaction_remain_intact(self) -> None:
        # 读取真实 Wayland TextInputSession Adapter。
        text_input = WAYLAND_TEXT_INPUT.read_text(encoding="utf-8")
        # 真实入口继续实现独立 ITextInput 契约。
        self.assertIn("impl ITextInput for WaylandBackend", text_input)
        # start 仍向 compositor 启用 session。
        self.assertIn("ti.enable();", text_input)
        # stop 仍向 compositor 禁用 session。
        self.assertIn("ti.disable();", text_input)
        # 读取共享 PlatformWindow 事务实现。
        shared = SHARED_WINDOW.read_text(encoding="utf-8")
        # 定位公开 start 方法。
        start_begin = shared.index("fn start_text_input(&mut self)")
        # 公开 stop 方法标记 start 片段终点。
        stop_begin = shared.index("fn stop_text_input(&mut self)", start_begin)
        # file-drop 方法标记 stop 片段终点。
        stop_end = shared.index("fn enable_file_drop", stop_begin)
        # 保存共享 start 事务。
        start = shared[start_begin:stop_begin]
        # 保存共享 stop 事务。
        stop = shared[stop_begin:stop_end]
        # start 必须先传播 Adapter 错误再写 active 状态。
        self.assertLess(start.index("os_start_text_input()?"), start.index("text_input_active, true"))
        # stop 必须先传播 Adapter 错误再写 inactive 状态。
        self.assertLess(stop.index("os_stop_text_input()?"), stop.index("text_input_active, false"))

    # 确认本任务涉及文件满足项目规模门槛。
    def test_touched_files_stay_within_limit(self) -> None:
        # 逐一检查本任务修改的源码与测试文件。
        for path in (WAYLAND_WINDOW_OPS, Path(__file__)):
            # 计算当前文件物理行数。
            line_count = len(path.read_text(encoding="utf-8").splitlines())
            # 文件必须保持不超过 900 行。
            self.assertLessEqual(line_count, 900, path)


# 允许直接运行本聚焦测试文件。
if __name__ == "__main__":
    # 使用标准 unittest runner 输出精确用例数。
    unittest.main()
