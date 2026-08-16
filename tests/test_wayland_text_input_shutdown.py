# 使用路径对象读取仓库内的 Wayland text-input 关闭契约。
from pathlib import Path
# 使用标准库测试框架保持聚焦验证无额外依赖。
import unittest

# 解析测试文件所在仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 text-input Module。
TEXT_INPUT = ROOT / "src/native/backends/linux/wayland/text_input.rs"
# 定位运行期失败关闭边沿。
EVENT_LOOP = ROOT / "src/native/backends/linux/wayland/event_loop.rs"
# 定位 Wayland backend 最终 Drop。
BACKEND = ROOT / "src/native/backends/linux/wayland/mod.rs"


# 验证 text-input session 与 callback owner 的关闭顺序。
class WaylandTextInputShutdownTests(unittest.TestCase):
    # 确认 fatal/Drop teardown 先失效 generation 再清理本地 owners。
    def test_shutdown_invalidates_session_and_callback_without_protocol_requests(self) -> None:
        # 读取 text-input Module。
        source = TEXT_INPUT.read_text(encoding="utf-8")
        # 限定 shutdown Component。
        shutdown_start = source.index("pub(crate) fn shutdown_text_input")
        # ITextInput 实现标记关闭端口末尾。
        shutdown_end = source.index("impl ITextInput for WaylandBackend", shutdown_start)
        # 保存关闭事务片段。
        shutdown = source[shutdown_start:shutdown_end]
        # generation 必须首先失效旧 callback。
        generation = shutdown.index("self.text_input_generation.fetch_add")
        # enabled=false 随后发布关闭事实。
        disabled = shutdown.index("self.text_input_enabled.store(false")
        # composition guard 在 callback 失效后取得。
        composition_lock = shutdown.index("let mut composition")
        # 未完成 composition 必须清空。
        composition_clear = shutdown.index("composition.active = false")
        # 活跃窗口随后清空。
        active_clear = shutdown.index("self.active_text_input_window_id = None")
        # 目标窗口随后清空。
        target_clear = shutdown.index("self.text_input_window_id = None")
        # proxy owner 最后从槽取走。
        proxy_take = shutdown.index("self.text_input.take()")
        # 兼容 callback 必须显式注销。
        callback_clear = shutdown.index("text_input.clear_callback()")
        # generation 必须早于 disabled 事实。
        self.assertLess(generation, disabled)
        # disabled 事实必须早于 composition lock。
        self.assertLess(disabled, composition_lock)
        # composition lock 必须早于状态清空。
        self.assertLess(composition_lock, composition_clear)
        # composition 清空必须早于窗口身份清空。
        self.assertLess(composition_clear, active_clear)
        # 活跃窗口必须早于目标窗口清空。
        self.assertLess(active_clear, target_clear)
        # 窗口身份清空必须早于 proxy take。
        self.assertLess(target_clear, proxy_take)
        # proxy take 必须早于 callback unregister。
        self.assertLess(proxy_take, callback_clear)
        # teardown 不得在失效连接上发送 disable。
        self.assertNotIn(".disable()", shutdown)
        # teardown 不得发送 commit。
        self.assertNotIn(".commit()", shutdown)
        # teardown 不得发布 unmark UI 事件。
        self.assertNotIn("on_unmark_text_for_window", shutdown)
        # teardown 不得关闭或写入 pending source。
        self.assertNotIn("pending_failures", shutdown)

    # 确认正常 stop 在协议与状态提交后显式注销 callback。
    def test_normal_stop_unregisters_callback_after_unmark(self) -> None:
        # 读取 text-input Module。
        source = TEXT_INPUT.read_text(encoding="utf-8")
        # 限定正常 stop 入口。
        stop_start = source.index("fn stop(&mut self)")
        # cursor rect 入口标记 stop 末尾。
        stop_end = source.index("fn set_cursor_rect", stop_start)
        # 保存正常 stop 片段。
        stop = source[stop_start:stop_end]
        # stop 首先推进 callback generation。
        generation = stop.index("self.text_input_generation.fetch_add")
        # 活跃 proxy 保持 disable 请求。
        disable = stop.index("ti.disable()")
        # disable 后保持 commit 请求。
        commit = stop.index("ti.commit()")
        # 活跃 composition 保持定向 unmark。
        unmark = stop.index("on_unmark_text_for_window_in_queue")
        # 状态提交后从 owner 槽取走 proxy。
        proxy_take = stop.index("self.text_input.take()")
        # 取走后显式注销 callback。
        callback_clear = stop.index("text_input.clear_callback()")
        # enabled=false 是最终发布事实。
        disabled = stop.index("self.text_input_enabled.store(false")
        # generation 先于 disable。
        self.assertLess(generation, disable)
        # disable 先于 commit。
        self.assertLess(disable, commit)
        # commit 先于 unmark 状态提交。
        self.assertLess(commit, unmark)
        # unmark 先于 proxy take。
        self.assertLess(unmark, proxy_take)
        # proxy take 先于 callback clear。
        self.assertLess(proxy_take, callback_clear)
        # callback clear 先于最终 disabled 事实。
        self.assertLess(callback_clear, disabled)
        # stop 不得再只赋 None 而遗漏 registry 清理。
        self.assertNotIn("self.text_input = None", stop)

    # 确认 runtime failure 在 seat/input 前停止独立 IME callback。
    def test_runtime_failure_orders_text_input_before_seat_and_clipboard(self) -> None:
        # 读取事件循环实现。
        source = EVENT_LOOP.read_text(encoding="utf-8")
        # 限定 runtime failure 关闭端口。
        close_start = source.index("fn close_after_failure")
        # checked dispatch 标记关闭端口末尾。
        close_end = source.index("pub(crate) fn dispatch_pending_checked", close_start)
        # 保存运行期关闭片段。
        close = source[close_start:close_end]
        # closed 事实先建立。
        closed = close.index("self.closed = true")
        # text-input teardown 是第一资源阶段。
        text_input = close.index("self.shutdown_text_input()")
        # seat/input teardown 随后执行。
        seat = close.index("self.shutdown_seat_and_input()")
        # clipboard teardown 最后执行。
        clipboard = close.index("self.shutdown_clipboard_io()")
        # closed 先于 text-input teardown。
        self.assertLess(closed, text_input)
        # text-input 先于 seat teardown。
        self.assertLess(text_input, seat)
        # seat teardown 先于 clipboard teardown。
        self.assertLess(seat, clipboard)

    # 确认最终 Drop 幂等复用相同资源关闭顺序。
    def test_drop_orders_text_input_before_seat_and_final_resources(self) -> None:
        # 读取 backend owner 实现。
        source = BACKEND.read_text(encoding="utf-8")
        # 限定 WaylandBackend Drop。
        drop_start = source.index("impl Drop for WaylandBackend")
        # 保存最终 Drop 片段。
        drop = source[drop_start:]
        # text-input teardown 是第一阶段。
        text_input = drop.index("self.shutdown_text_input()")
        # seat/input teardown 是第二阶段。
        seat = drop.index("self.shutdown_seat_and_input()")
        # clipboard teardown 是第三阶段。
        clipboard = drop.index("self.shutdown_clipboard_io()")
        # pending source 随后关闭。
        source_close = drop.index("self.pending_failures.close()")
        # text-input 先于 seat teardown。
        self.assertLess(text_input, seat)
        # seat 先于 clipboard teardown。
        self.assertLess(seat, clipboard)
        # clipboard 先于 pending source 关闭。
        self.assertLess(clipboard, source_close)

    # 确认本任务涉及文件满足项目规模门槛。
    def test_touched_files_stay_within_limit(self) -> None:
        # 逐文件验证不超过 900 行。
        for path in [TEXT_INPUT, EVENT_LOOP, BACKEND]:
            # 计算当前物理行数。
            line_count = len(path.read_text(encoding="utf-8").splitlines())
            # 超限时返回具体文件路径。
            self.assertLessEqual(line_count, 900, str(path))


# 允许直接运行本聚焦测试文件。
if __name__ == "__main__":
    # 使用标准 unittest runner 输出精确用例数。
    unittest.main()
