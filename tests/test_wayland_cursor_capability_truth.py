# 使用路径对象读取 Wayland cursor 与 App 交接契约。
from pathlib import Path
# 使用标准库测试框架保持聚焦验证无额外依赖。
import unittest

# 解析测试文件所在仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 Wayland cursor Adapter。
WAYLAND_CURSOR = ROOT / "src/native/backends/linux/wayland/cursor.rs"
# 定位 App pointer-cursor 交接 Component。
APP_CURSOR = ROOT / "src/app/event_loop/pointer_cursor.rs"


# 验证未接线 Wayland cursor 能力不再伪装成功。
class WaylandCursorCapabilityTruthTests(unittest.TestCase):
    # 确认 set/show 返回稳定 NotImplemented。
    def test_wayland_set_and_show_return_typed_capability_errors(self) -> None:
        # 读取 Wayland cursor Adapter。
        source = WAYLAND_CURSOR.read_text(encoding="utf-8")
        # 定位 set_cursor 入口。
        set_start = source.index("fn set_cursor")
        # show_cursor 标记 set 片段终点。
        show_start = source.index("fn show_cursor", set_start)
        # cursor_position 标记 show 片段终点。
        show_end = source.index("fn cursor_position", show_start)
        # 保存两个入口片段。
        set_cursor = source[set_start:show_start]
        # 保存可见性入口片段。
        show_cursor = source[show_start:show_end]
        # 两个未接线入口都必须使用 NotImplemented。
        self.assertIn("Errc::NotImplemented", set_cursor)
        # show_cursor 保持相同稳定分类。
        self.assertIn("Errc::NotImplemented", show_cursor)
        # set_cursor 诊断必须保留待接线协议。
        self.assertIn("wp_cursor_shape_manager_v1 is not wired", set_cursor)
        # show_cursor 诊断必须明确可见性未接线。
        self.assertIn("Wayland cursor visibility is not wired", show_cursor)
        # 两个入口都不得返回伪成功。
        self.assertNotIn("Ok(())", set_cursor + show_cursor)
        # 两个入口都不得用日志替代平台动作。
        self.assertNotIn("tracing::", set_cursor + show_cursor)

    # 确认 App 只对稳定能力缺失去重。
    def test_app_deduplicates_not_implemented_but_retries_other_errors(self) -> None:
        # 读取 App cursor 交接 Component。
        source = APP_CURSOR.read_text(encoding="utf-8")
        # 定位窄端口交接函数。
        start = source.index("fn apply_cursor_port")
        # 测试模块标记实现片段终点。
        end = source.index("#[cfg(test)]", start)
        # 保存交接实现。
        apply = source[start:end]
        # NotImplemented 必须有独立 guard。
        self.assertIn("error.code() == Errc::NotImplemented", apply)
        # capability absence 分支必须更新 active 以去重。
        capability_start = apply.index("Err(error) if error.code()")
        # 普通失败分支标记 capability 片段终点。
        retry_start = apply.index("Err(error) =>", capability_start)
        # 保存 capability absence 分支。
        capability = apply[capability_start:retry_start]
        # 稳定缺失必须缓存请求值。
        self.assertIn("active.set(Some(requested))", capability)
        # 稳定缺失不得产生 warn。
        self.assertNotIn("tracing::warn!", capability)
        # 普通失败仍必须产生可观察 warning。
        self.assertIn("tracing::warn!", apply[retry_start:])
        # 普通失败分支不得更新 active。
        self.assertNotIn("active.set", apply[retry_start:])

    # 确认本任务涉及文件满足项目规模门槛。
    def test_touched_files_stay_within_limit(self) -> None:
        # 逐一检查本任务修改的源码与测试文件。
        for path in (WAYLAND_CURSOR, APP_CURSOR, Path(__file__)):
            # 计算当前文件物理行数。
            line_count = len(path.read_text(encoding="utf-8").splitlines())
            # 文件必须保持不超过 900 行。
            self.assertLessEqual(line_count, 900, path)


# 允许直接运行本聚焦测试文件。
if __name__ == "__main__":
    # 使用标准 unittest runner 输出精确用例数。
    unittest.main()
