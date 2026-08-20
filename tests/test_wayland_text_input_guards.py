# 导入标准单元测试框架。
import unittest
# 使用路径对象定位仓库文件。
from pathlib import Path


# 计算仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 Wayland text-input adapter。
TEXT_INPUT = ROOT / "src/native/backends/linux/windowing/wayland/text_input.rs"
# 定位共享 IME 事件 Component。
IME_EVENTS = ROOT / "src/native/windowing/shared/ime_events.rs"


# 验证 Wayland IME owner-thread 与 callback 事务契约。
class WaylandTextInputGuardTests(unittest.TestCase):
    # 确认共享 queue-guard 窄端口不会自行二次锁事件队列。
    def test_shared_ime_queue_guard_ports_do_not_relock(self) -> None:
        # 读取共享 IME Component。
        source = IME_EVENTS.read_text(encoding="utf-8")
        # 定位 batch queue-guard 入口。
        apply_start = source.index("pub(crate) fn apply_for_window_in_queue")
        # 下一个公开 marked-text 入口标记 batch 实现末尾。
        apply_end = source.index("pub(crate) fn on_marked_text(", apply_start)
        # 保存 batch 无锁窄端口。
        apply_port = source[apply_start:apply_end]
        # 窄端口不得重新取得 Arc<Mutex> 事件队列。
        self.assertNotIn("event_queue(", apply_port)
        # 窄端口不得直接访问 mutex。
        self.assertNotIn(".lock()", apply_port)
        # 窄端口必须复用调用方传入的 VecDeque。
        self.assertIn("events: &mut VecDeque<UiEvent>", apply_port)
        # commit 必须先于 preedit 应用。
        self.assertLess(apply_port.index("batch.commit"), apply_port.index("batch.preedit"))
        # 定位公开 unmark queue-guard 入口。
        unmark_start = source.index("pub(crate) fn on_unmark_text_for_window_in_queue")
        # 私有兼容 wrapper 标记入口末尾。
        unmark_end = source.index("fn on_unmark_text_targeted(", unmark_start)
        # 保存 unmark 无锁窄端口。
        unmark_port = source[unmark_start:unmark_end]
        # unmark 窄端口不得二次锁。
        self.assertNotIn(".lock()", unmark_port)
        # unmark 必须委托同一 queue-guard 实现。
        self.assertIn("on_unmark_text_targeted_in_queue", unmark_port)

    # 确认 callback 只在 surface/composition/events owners 健康后提交。
    def test_callbacks_enqueue_typed_failures_before_local_state_changes(self) -> None:
        # 读取 Wayland text-input adapter。
        source = TEXT_INPUT.read_text(encoding="utf-8")
        # 限定 callback 注册片段。
        callback_start = source.index("ti.quick_assign")
        # 协议 enable 标记 callback 片段末尾。
        callback_end = source.index("ti.enable()", callback_start)
        # 保存 callback 状态机。
        callback = source[callback_start:callback_end]
        # 协议 callback 不得恢复 poisoned owner；最终 teardown 可只为释放资源取回 guard。
        self.assertNotIn("into_inner()", callback)
        # Enter 必须 checked lock surface registry。
        self.assertIn("match surface_windows.lock()", callback)
        # surface failure 必须稳定分类为 InvalidState。
        self.assertIn("enter callback surface registry mutex poisoned", callback)
        # 三个 composition/event 阶段必须使用统一双 guard helper。
        for stage in ["enter-unmark callback", "leave callback", "done callback"]:
            # 每个阶段都必须保留稳定诊断参数。
            self.assertIn(f'"{stage}"', callback)
        # callback failure 必须进入既有 pending source。
        self.assertEqual(callback.count("pending_failures.enqueue"), 4)
        # Enter/Leave 必须使用共享无锁 unmark 入口。
        self.assertEqual(callback.count("on_unmark_text_for_window_in_queue"), 2)
        # Done 必须使用共享无锁 batch apply 入口。
        self.assertIn("pending.apply_for_window_in_queue", callback)
        # callback 不得调用带 Arc<Mutex> 的兼容 batch 入口。
        self.assertNotIn("pending.apply_for_window(&events", callback)
        # callback 不得执行最终错误策略或用户代码。
        for forbidden in ["tracing::", ".report(", "attempt_recovery"]:
            # 每项副作用都禁止进入协议 callback。
            self.assertNotIn(forbidden, callback)

    # 确认同步 stop 在协议/session 改写前验证 IME owners。
    def test_stop_checks_ime_owners_before_session_teardown(self) -> None:
        # 读取 Wayland text-input adapter。
        source = TEXT_INPUT.read_text(encoding="utf-8")
        # 定位双 owner helper。
        helper_start = source.index("fn lock_ime_state_checked")
        # backend 生命周期实现标记双 owner helper 末尾。
        helper_end = source.index("impl WaylandBackend", helper_start)
        # 保存固定锁序 helper。
        helper = source[helper_start:helper_end]
        # composition owner 必须先取得。
        composition_lock = helper.index("composition.lock()")
        # event queue owner 随后取得。
        event_lock = helper.index("events.lock()")
        # 固定保持 composition 后 events 的锁顺序。
        self.assertLess(composition_lock, event_lock)
        # 两类状态失败都稳定分类为 InvalidState。
        self.assertEqual(helper.count("Errc::InvalidState"), 2)
        # helper 不得恢复 poisoned owner。
        self.assertNotIn("into_inner()", helper)
        # 定位同步 stop 实现。
        stop_start = source.index("fn stop(&mut self)")
        # cursor rect 入口标记 stop 末尾。
        stop_end = source.index("fn set_cursor_rect", stop_start)
        # 保存同步 teardown 事务。
        stop = source[stop_start:stop_end]
        # owner 检查必须先于 generation 失效。
        owner_check = stop.index("lock_ime_state_checked(")
        # generation 是首个 callback 生命周期改写。
        generation = stop.index("self.text_input_generation.fetch_add")
        # 协议 disable 只能发生在 owner 检查后。
        disable = stop.index("ti.disable()")
        # active window 只能在 unmark 成功后释放。
        release_window = stop.index("self.active_text_input_window_id = None")
        # owner 检查先于 callback generation 失效。
        self.assertLess(owner_check, generation)
        # generation 失效先于协议 disable。
        self.assertLess(generation, disable)
        # 无锁 unmark 先于 active window 释放。
        self.assertLess(stop.index("on_unmark_text_for_window_in_queue"), release_window)
        # stop 不得提前 take active window owner。
        self.assertNotIn("active_text_input_window_id.take()", stop)


# 直接执行本文件时运行契约测试。
if __name__ == "__main__":
    # 交给 unittest 输出稳定测试结果。
    unittest.main()
