# 使用路径对象读取仓库内的 Rust 关闭契约。
from pathlib import Path
# 使用标准库测试框架保持聚焦验证无额外依赖。
import unittest

# 解析测试文件所在仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 Wayland 事件循环 owner。
EVENT_LOOP = ROOT / "src/native/backends/linux/wayland/event_loop.rs"
# 定位 Wayland backend 最终 Drop owner。
BACKEND = ROOT / "src/native/backends/linux/wayland/mod.rs"


# 验证运行期致命失败与最终 Drop 的职责边界。
class WaylandRuntimeFailureShutdownTests(unittest.TestCase):
    # 确认首次失败按根因、closed、输入 teardown 的顺序提交。
    def test_close_after_failure_is_idempotent_and_stops_input(self) -> None:
        # 读取事件循环实现。
        source = EVENT_LOOP.read_text(encoding="utf-8")
        # 限定运行期失败关闭端口。
        close_start = source.index("fn close_after_failure")
        # checked dispatch 标记关闭端口末尾。
        close_end = source.index("pub(crate) fn dispatch_pending_checked", close_start)
        # 保存关闭事务片段。
        close = source[close_start:close_end]
        # 幂等门槛必须先检查 closed。
        closed_guard = close.index("if self.closed")
        # 首次根因随后进入 pending source。
        root_failure = close.index("self.enqueue_failure(Error::new(code, message))")
        # closed 事实必须在根因入队后建立。
        closed_commit = close.index("self.closed = true")
        # 输入 teardown 必须晚于 closed 事实。
        input_shutdown = close.index("self.shutdown_seat_and_input()")
        # 幂等门槛先于所有首次关闭副作用。
        self.assertLess(closed_guard, root_failure)
        # 根因入队先于 closed 事实。
        self.assertLess(root_failure, closed_commit)
        # closed 事实先于输入 callback teardown。
        self.assertLess(closed_commit, input_shutdown)
        # 运行期关闭不得提前关闭 pending source。
        self.assertNotIn("pending_failures.close", close)
        # 单个关闭端口只允许一次根因入队。
        self.assertEqual(close.count("self.enqueue_failure"), 1)
        # 单个关闭端口只允许一次输入 teardown。
        self.assertEqual(close.count("self.shutdown_seat_and_input"), 1)

    # 确认三个 dispatch 入口在 closed 后不再开始协议工作。
    def test_dispatch_entrypoints_stop_after_closed_fact(self) -> None:
        # 读取事件循环实现。
        source = EVENT_LOOP.read_text(encoding="utf-8")
        # 列出所有可开始协议调度的公开 owner 入口。
        entries = [
            # 非阻塞入口。
            ("pub(crate) fn try_dispatch", "pub(crate) fn dispatch_blocking"),
            # 阻塞入口。
            ("pub(crate) fn dispatch_blocking", "pub(crate) fn dispatch_timeout"),
            # 超时入口。
            ("pub(crate) fn dispatch_timeout", "pub(crate) fn waker"),
        ]
        # 逐入口验证 closed 门槛位于协议工作之前。
        for start_marker, end_marker in entries:
            # 定位当前入口起点。
            start = source.index(start_marker)
            # 定位下一个入口边界。
            end = source.index(end_marker, start)
            # 保存单个入口片段。
            entry = source[start:end]
            # 每个入口都必须读取同一 closed 事实。
            self.assertIn("if self.closed", entry)
            # closed 分支必须直接终止调度。
            self.assertIn("return false", entry)

    # 确认 flush 的致命失败也进入同一 backend 关闭端口。
    def test_flush_fatal_errors_share_runtime_shutdown_edge(self) -> None:
        # 读取事件循环实现。
        source = EVENT_LOOP.read_text(encoding="utf-8")
        # 限定 flush 端口到实现末尾。
        flush_start = source.index("pub(crate) fn flush_checked")
        # 保存 flush 失败映射片段。
        flush = source[flush_start:]
        # flush 必须取得可变 backend owner 以提交关闭。
        self.assertIn("&mut self", flush)
        # WouldBlock 继续保持健康非阻塞语义。
        self.assertIn("ErrorKind::WouldBlock => true", flush)
        # I/O 与协议错误都必须委托同一关闭端口。
        self.assertEqual(flush.count("self.close_after_failure("), 2)
        # flush 不得绕过关闭端口直接入队后继续存活。
        self.assertNotIn("self.enqueue_failure", flush)
        # I/O 错误保留既有分类。
        self.assertIn("Errc::IoError", flush)
        # 协议错误保留既有平台分类。
        self.assertIn("Errc::PlatformError", flush)

    # 确认最终 Drop 仍拥有 source 与 wake pipe 的最终回收。
    def test_drop_retains_final_resource_ownership(self) -> None:
        # 读取 backend owner 实现。
        source = BACKEND.read_text(encoding="utf-8")
        # 限定 WaylandBackend 的 Drop 实现。
        drop_start = source.index("impl Drop for WaylandBackend")
        # 保存最终 Drop 片段。
        drop = source[drop_start:]
        # Drop 重试幂等输入 teardown。
        self.assertIn("self.shutdown_seat_and_input()", drop)
        # 最终 owner 销毁才关闭 pending source。
        self.assertIn("self.pending_failures.close()", drop)
        # 最终 owner 销毁关闭 wake pipe 两端。
        self.assertEqual(drop.count("libc::close"), 2)

    # 确认本任务涉及文件满足项目规模门槛。
    def test_touched_files_stay_within_limit(self) -> None:
        # 逐文件验证不超过 900 行。
        for path in [EVENT_LOOP, BACKEND]:
            # 计算当前物理行数。
            line_count = len(path.read_text(encoding="utf-8").splitlines())
            # 超限时返回具体文件路径。
            self.assertLessEqual(line_count, 900, str(path))


# 允许直接运行本聚焦测试文件。
if __name__ == "__main__":
    # 使用标准 unittest runner 输出精确用例数。
    unittest.main()
