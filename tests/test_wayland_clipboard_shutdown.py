# 使用路径对象读取仓库内的 Wayland clipboard 关闭契约。
from pathlib import Path
# 使用标准库测试框架保持聚焦验证无额外依赖。
import unittest

# 解析测试文件所在仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 clipboard Module。
CLIPBOARD = ROOT / "src/native/backends/linux/wayland/clipboard.rs"
# 定位运行期失败关闭边沿。
EVENT_LOOP = ROOT / "src/native/backends/linux/wayland/event_loop.rs"
# 定位 Wayland backend 最终 Drop。
BACKEND = ROOT / "src/native/backends/linux/wayland/mod.rs"


# 验证 clipboard 在途 I/O owner 的确定性关闭顺序。
class WaylandClipboardShutdownTests(unittest.TestCase):
    # 确认 shutdown Component 固定取得五类 owner guards。
    def test_shutdown_locks_clipboard_owners_in_global_order(self) -> None:
        # 读取 clipboard Module。
        source = CLIPBOARD.read_text(encoding="utf-8")
        # 限定 shutdown Component。
        shutdown_start = source.index("pub(crate) fn shutdown_clipboard_io")
        # IClipboard 实现标记关闭端口末尾。
        shutdown_end = source.index("impl IClipboard for WaylandBackend", shutdown_start)
        # 保存关闭事务片段。
        shutdown = source[shutdown_start:shutdown_end]
        # 第一 owner 是 selection ownership。
        owns_lock = shutdown.index("let mut owns_clipboard")
        # 第二 owner 是在途 read。
        read_lock = shutdown.index("let mut clipboard_read")
        # 第三 owner 是文本缓存。
        text_lock = shutdown.index("let mut clipboard_text")
        # 第四 owner 是在途 writes。
        writes_lock = shutdown.index("let mut clipboard_writes")
        # 第五 owner 是输入 serial。
        serial_lock = shutdown.index("let mut input_serial")
        # 固定 owns→read 顺序。
        self.assertLess(owns_lock, read_lock)
        # 固定 read→text 顺序。
        self.assertLess(read_lock, text_lock)
        # 固定 text→writes 顺序。
        self.assertLess(text_lock, writes_lock)
        # 固定 writes→input-serial 顺序。
        self.assertLess(writes_lock, serial_lock)
        # 五类 teardown owner 都必须确定性恢复 guard。
        self.assertEqual(shutdown.count("into_inner()"), 5)

    # 确认所有 guards 健康或已恢复后一次清除业务与 FD owners。
    def test_shutdown_commits_state_and_fd_release_after_all_guards(self) -> None:
        # 读取 clipboard Module。
        source = CLIPBOARD.read_text(encoding="utf-8")
        # 限定 shutdown Component。
        shutdown_start = source.index("pub(crate) fn shutdown_clipboard_io")
        # IClipboard 实现标记关闭端口末尾。
        shutdown_end = source.index("impl IClipboard for WaylandBackend", shutdown_start)
        # 保存关闭事务片段。
        shutdown = source[shutdown_start:shutdown_end]
        # 最后一把 guard 是 input serial。
        serial_lock = shutdown.index("let mut input_serial")
        # 第一项提交失效 selection ownership。
        owns_commit = shutdown.index("*owns_clipboard = false")
        # 第二项提交 Drop read FD owner。
        read_commit = shutdown.index("*clipboard_read = None")
        # 第三项提交清除文本缓存。
        text_commit = shutdown.index("clipboard_text.clear()")
        # 第四项提交 Drop 全部 write FD owners。
        writes_commit = shutdown.index("clipboard_writes.clear()")
        # 最后清除输入 serial。
        serial_commit = shutdown.index("*input_serial = Default::default()")
        # 所有业务写入必须晚于最后一把 guard。
        self.assertLess(serial_lock, owns_commit)
        # ownership 先于 read owner 释放。
        self.assertLess(owns_commit, read_commit)
        # read owner 释放先于文本清理。
        self.assertLess(read_commit, text_commit)
        # 文本清理先于 write owners 释放。
        self.assertLess(text_commit, writes_commit)
        # write owners 释放先于 serial 清空。
        self.assertLess(writes_commit, serial_commit)
        # shutdown 不得发起新 Wayland selection 请求。
        self.assertNotIn("set_selection", shutdown)
        # shutdown 不得关闭 pending failure source。
        self.assertNotIn("pending_failures.close", shutdown)

    # 确认 read/write 类型确实独占会在 clear 时 Drop 的文件描述符。
    def test_clipboard_io_values_own_their_files(self) -> None:
        # 读取 clipboard Module。
        source = CLIPBOARD.read_text(encoding="utf-8")
        # ClipboardRead 必须保存独占 File owner。
        read_start = source.index("pub(crate) struct ClipboardRead")
        # read 构造 helper 标记结构体片段末尾。
        read_end = source.index("impl ClipboardRead", read_start)
        # 保存 read owner 定义。
        read_owner = source[read_start:read_end]
        # read owner 内必须存在 File 字段。
        self.assertIn("file: File", read_owner)
        # ClipboardWrite 必须保存独占 File owner。
        write_start = source.index("pub(crate) struct ClipboardWrite")
        # write 构造 helper 标记结构体片段末尾。
        write_end = source.index("impl ClipboardWrite", write_start)
        # 保存 write owner 定义。
        write_owner = source[write_start:write_end]
        # write owner 内必须存在 File 字段。
        self.assertIn("file: File", write_owner)

    # 确认运行期 failure 在输入 callback 停止后释放 clipboard owners。
    def test_runtime_failure_orders_input_before_clipboard_shutdown(self) -> None:
        # 读取事件循环实现。
        source = EVENT_LOOP.read_text(encoding="utf-8")
        # 限定 runtime failure 关闭端口。
        close_start = source.index("fn close_after_failure")
        # checked dispatch 标记关闭端口末尾。
        close_end = source.index("pub(crate) fn dispatch_pending_checked", close_start)
        # 保存运行期关闭片段。
        close = source[close_start:close_end]
        # 根因先进入 pending source。
        failure_commit = close.index("self.enqueue_failure")
        # 随后建立 closed 事实。
        closed_commit = close.index("self.closed = true")
        # 输入 callback teardown 必须先执行。
        input_shutdown = close.index("self.shutdown_seat_and_input()")
        # clipboard I/O teardown 必须随后执行。
        clipboard_shutdown = close.index("self.shutdown_clipboard_io()")
        # 根因入队先于 closed。
        self.assertLess(failure_commit, closed_commit)
        # closed 先于输入 teardown。
        self.assertLess(closed_commit, input_shutdown)
        # 输入 teardown 先于 clipboard teardown。
        self.assertLess(input_shutdown, clipboard_shutdown)

    # 确认最终 Drop 幂等复用并保持资源关闭总顺序。
    def test_drop_reuses_clipboard_shutdown_before_final_resources(self) -> None:
        # 读取 backend owner 实现。
        source = BACKEND.read_text(encoding="utf-8")
        # 限定 WaylandBackend Drop。
        drop_start = source.index("impl Drop for WaylandBackend")
        # 保存最终 Drop 片段。
        drop = source[drop_start:]
        # 输入 teardown 是第一阶段。
        input_shutdown = drop.index("self.shutdown_seat_and_input()")
        # clipboard teardown 是第二阶段。
        clipboard_shutdown = drop.index("self.shutdown_clipboard_io()")
        # pending source 随后关闭。
        source_close = drop.index("self.pending_failures.close()")
        # wake pipe 最后开始关闭。
        wake_close = drop.index("libc::close")
        # 输入先于 clipboard teardown。
        self.assertLess(input_shutdown, clipboard_shutdown)
        # clipboard 先于 pending source 关闭。
        self.assertLess(clipboard_shutdown, source_close)
        # pending source 先于 wake pipe 关闭。
        self.assertLess(source_close, wake_close)

    # 确认本任务涉及文件满足项目规模门槛。
    def test_touched_files_stay_within_limit(self) -> None:
        # 逐文件验证不超过 900 行。
        for path in [CLIPBOARD, EVENT_LOOP, BACKEND]:
            # 计算当前物理行数。
            line_count = len(path.read_text(encoding="utf-8").splitlines())
            # 超限时返回具体文件路径。
            self.assertLessEqual(line_count, 900, str(path))


# 允许直接运行本聚焦测试文件。
if __name__ == "__main__":
    # 使用标准 unittest runner 输出精确用例数。
    unittest.main()
