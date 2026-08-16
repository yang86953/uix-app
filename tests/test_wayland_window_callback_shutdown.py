# 使用路径对象读取仓库内的 Wayland 逐窗生命周期契约。
from pathlib import Path
# 使用标准库测试框架保持聚焦验证无额外依赖。
import unittest

# 解析测试文件所在仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位逐窗 callback shutdown Component。
SHUTDOWN = ROOT / "src/native/backends/linux/wayland/window_callback_shutdown.rs"
# 定位显式窗口关闭编排。
WINDOW_OPS = ROOT / "src/native/backends/linux/wayland/window_ops.rs"
# 定位无返回通道的 Drop adapter。
REGISTRATION = ROOT / "src/native/backends/linux/wayland/surface_registration.rs"
# 定位 Wayland 子模块声明。
BACKEND = ROOT / "src/native/backends/linux/wayland/mod.rs"


# 验证逐窗 xdg-shell callback owners 的确定性关闭契约。
class WaylandWindowCallbackShutdownTests(unittest.TestCase):
    # 确认独立 Component 只消费两个逐窗 callback handles。
    def test_component_clears_callbacks_before_handle_release(self) -> None:
        # 读取逐窗 shutdown Component。
        source = SHUTDOWN.read_text(encoding="utf-8")
        # toplevel owner 必须只被取出一次。
        self.assertEqual(source.count("self.toplevel.take()"), 1)
        # xdg-surface owner 必须只被取出一次。
        self.assertEqual(source.count("self.xdg_surface.take()"), 1)
        # 两个 callback 都必须在局部 handle 存活时注销。
        self.assertEqual(source.count(".clear_callback()"), 2)
        # 记录 toplevel callback 生命周期顺序。
        toplevel_take = source.index("self.toplevel.take()")
        # 记录首个 callback 注销位置。
        toplevel_clear = source.index("toplevel.clear_callback()")
        # 记录 xdg-surface callback 生命周期顺序。
        surface_take = source.index("self.xdg_surface.take()")
        # 记录第二个 callback 注销位置。
        surface_clear = source.index("xdg_surface.clear_callback()")
        # toplevel 必须先从公开 owner 槽取出再注销。
        self.assertLess(toplevel_take, toplevel_clear)
        # toplevel callback 必须先于其依赖的 xdg-surface callback 注销。
        self.assertLess(toplevel_clear, surface_take)
        # xdg-surface handle 必须在注销期间保持存活。
        self.assertLess(surface_take, surface_clear)
        # teardown 不得提交任何新协议请求。
        self.assertNotIn(".destroy(", source)
        # teardown 不得向事件或失败队列入队。
        self.assertNotIn("enqueue", source)

    # 确认显式关闭只在注册事实成功撤销后释放 callbacks。
    def test_explicit_close_preserves_retry_before_callback_shutdown(self) -> None:
        # 读取窗口操作 Module。
        source = WINDOW_OPS.read_text(encoding="utf-8")
        # 定位显式关闭入口。
        start = source.index("fn os_close")
        # 以主动关闭意图入口作为片段终点。
        end = source.index("fn os_request_close", start)
        # 保存显式 teardown 片段。
        close = source[start:end]
        # 定位检查式 surface 注册表注销。
        unregister = close.index("self.unregister_surface()?")
        # 定位逐窗 callback teardown。
        shutdown = close.index("self.shutdown_window_callbacks()")
        # 注册表失败必须在 callback/handle 仍可重试时返回。
        self.assertLess(unregister, shutdown)
        # 装饰 owner 必须先于其依赖的 toplevel handle 释放。
        decoration = close.index("self.xdg_decoration = None")
        # 装饰释放必须先于 xdg-shell callback teardown。
        self.assertLess(decoration, shutdown)
        # 旧的直接 handle 释放不得绕过 callback Component。
        self.assertNotIn("self.toplevel = None", close)
        # xdg-surface 也不得绕过 callback Component。
        self.assertNotIn("self.xdg_surface = None", close)

    # 确认 Drop 即使注册表注销失败也继续释放 callback owners。
    def test_drop_always_runs_callback_shutdown(self) -> None:
        # 读取 Drop adapter。
        source = REGISTRATION.read_text(encoding="utf-8")
        # 定位 Drop 实现。
        drop_impl = source[source.index("impl Drop for WaylandWindowOps") :]
        # 定位 surface 注册事实注销尝试。
        unregister = drop_impl.index("self.unregister_surface()")
        # 定位失败转交通道。
        enqueue = drop_impl.index("self.pending_failures.enqueue(error)")
        # 定位无条件 callback teardown。
        shutdown = drop_impl.index("self.shutdown_window_callbacks()")
        # Drop 先尝试撤销共享注册事实。
        self.assertLess(unregister, enqueue)
        # 无论 if 分支是否执行，后继语句都必须继续释放 callbacks。
        self.assertLess(enqueue, shutdown)
        # teardown 调用不得嵌在错误分支内部。
        self.assertIn("        }\n        // Drop 没有显式重试入口", drop_impl)

    # 确认 Component 已接入唯一 Wayland 模块树。
    def test_component_is_declared_once(self) -> None:
        # 读取 backend 子模块声明。
        source = BACKEND.read_text(encoding="utf-8")
        # 独立 Component 必须只声明一次。
        self.assertEqual(source.count("mod window_callback_shutdown;"), 1)

    # 确认本任务涉及文件满足项目规模门槛。
    def test_touched_files_stay_within_limit(self) -> None:
        # 逐一检查本任务修改的源码与测试文件。
        for path in (SHUTDOWN, WINDOW_OPS, REGISTRATION, BACKEND, Path(__file__)):
            # 计算当前文件物理行数。
            line_count = len(path.read_text(encoding="utf-8").splitlines())
            # 文件必须保持不超过 900 行。
            self.assertLessEqual(line_count, 900, path)


# 允许直接运行本聚焦测试文件。
if __name__ == "__main__":
    # 使用标准 unittest runner 输出精确用例数。
    unittest.main()
