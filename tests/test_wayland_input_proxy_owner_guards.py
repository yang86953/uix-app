# 导入标准单元测试框架。
import unittest
# 使用路径对象定位仓库文件。
from pathlib import Path


# 计算仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 capability callback adapter。
SEAT = ROOT / "src/native/backends/linux/wayland/seat.rs"
# 定位无状态代理 owner Component。
OWNER = ROOT / "src/native/backends/linux/wayland/input_proxy_owner.rs"


# 验证 Wayland capability 绑定 owner 的检查式生命周期。
class WaylandInputProxyOwnerGuardTests(unittest.TestCase):
    # 确认双槽快照在任何 transition 或协议创建前 fail-closed。
    def test_proxy_slot_snapshot_precedes_capability_transitions(self) -> None:
        # 读取 seat capability adapter。
        seat = SEAT.read_text(encoding="utf-8")
        # 限定 Capabilities callback 片段。
        callback_start = seat.index("wl_seat::Event::Capabilities")
        # pointer Bind 分支标记快照决策片段末尾。
        callback_end = seat.index("if pointer_transition == InputProxyTransition::Bind", callback_start)
        # 保存 transition 前置片段。
        snapshot = seat[callback_start:callback_end]
        # seat 必须委托 Component 读取双槽快照。
        snapshot_call = snapshot.index("snapshot_input_proxy_slots(")
        # pointer transition 必须晚于健康快照。
        pointer_transition = snapshot.index("let pointer_transition")
        # keyboard transition 也必须晚于健康快照。
        keyboard_transition = snapshot.index("let keyboard_transition")
        # 双槽 owner 检查先于任何 transition。
        self.assertLess(snapshot_call, pointer_transition)
        # pointer 决策先于同一快照的 keyboard 决策。
        self.assertLess(pointer_transition, keyboard_transition)
        # adapter 前置片段不得直接恢复 poisoned 槽。
        self.assertNotIn("into_inner()", snapshot)
        # 槽 failure 必须立即停止 callback。
        self.assertIn("return", snapshot[snapshot_call:pointer_transition])

    # 确认 Component 固定锁序并回滚未提交协议代理。
    def test_owner_component_checks_slots_and_discards_failed_bind(self) -> None:
        # 读取输入代理 owner Component。
        owner = OWNER.read_text(encoding="utf-8")
        # Component 不得恢复任何 poisoned owner。
        self.assertNotIn("into_inner()", owner)
        # 限定双槽快照端口。
        snapshot_start = owner.index("pub(crate) fn snapshot_input_proxy_slots")
        # generation 端口标记快照末尾。
        snapshot_end = owner.index("pub(crate) fn pointer_generation_checked", snapshot_start)
        # 保存双槽快照实现。
        snapshot = owner[snapshot_start:snapshot_end]
        # pointer 槽必须先取得。
        pointer_lock = snapshot.index("pointer_slot.lock()")
        # keyboard 槽随后取得。
        keyboard_lock = snapshot.index("keyboard_slot.lock()")
        # 固定保持 pointer→keyboard 检查顺序。
        self.assertLess(pointer_lock, keyboard_lock)
        # 两个槽 failure 都稳定分类。
        self.assertEqual(snapshot.count("enqueue_owner_failure"), 2)
        # generation 读取必须有独立诊断。
        self.assertIn("pointer activation registry mutex poisoned during bind", owner)
        # pointer install failure 必须先丢弃局部代理。
        pointer_install = owner[owner.index("pub(crate) fn install_pointer_proxy"):]
        # keyboard install 标记 pointer 端口末尾。
        pointer_install = pointer_install[:pointer_install.index("pub(crate) fn install_keyboard_proxy")]
        # 回滚必须先于 failure 入队。
        self.assertLess(pointer_install.index("discard_pointer_proxy(pointer)"), pointer_install.index("enqueue_owner_failure"))
        # keyboard install failure 也必须回滚局部代理。
        keyboard_install = owner[owner.index("pub(crate) fn install_keyboard_proxy"):]
        # keyboard 回滚必须存在。
        self.assertIn("discard_keyboard_proxy(keyboard)", keyboard_install)
        # 两类局部代理都必须先注销 callback。
        self.assertEqual(owner.count("clear_callback()"), 2)
        # 两类局部代理都保留版本保护。
        self.assertEqual(owner.count("version() >= 3"), 2)
        # 所有状态失败统一分类为 InvalidState。
        self.assertIn("Errc::InvalidState", owner)
        # Component 不得执行最终错误策略或用户代码。
        for forbidden in ["tracing::", ".report(", "attempt_recovery"]:
            # 每项副作用都禁止进入 owner Component。
            self.assertNotIn(forbidden, owner)

    # 确认 seat 的 Bind 分支使用 generation 与 checked install 端口。
    def test_bind_paths_delegate_generation_and_owner_commits(self) -> None:
        # 读取 seat capability adapter。
        seat = SEAT.read_text(encoding="utf-8")
        # pointer Bind 必须在 get_pointer 前读取健康 generation。
        generation = seat.index("pointer_generation_checked(")
        # 协议 pointer 只能在 generation 成功后创建。
        get_pointer = seat.index("seat.get_pointer()")
        # generation 检查先于代理创建。
        self.assertLess(generation, get_pointer)
        # pointer callback 注册后必须 checked install。
        self.assertIn("install_pointer_proxy(", seat)
        # keyboard callback 注册后必须 checked install。
        self.assertIn("install_keyboard_proxy(", seat)
        # 两个 install failure 都停止 capability callback。
        self.assertGreaterEqual(seat.count("槽损坏时局部代理已释放"), 2)

    # 确认 pointer Release 在三个 owners 健康后一次提交 teardown。
    def test_pointer_release_commits_three_owner_state_transactionally(self) -> None:
        # 读取输入代理 owner Component。
        owner = OWNER.read_text(encoding="utf-8")
        # 读取 seat capability adapter。
        seat = SEAT.read_text(encoding="utf-8")
        # 定位 checked pointer Release 端口。
        release_start = owner.index("pub(crate) fn release_pointer_proxy_checked")
        # 保存文件末尾的完整 Release Component。
        release = owner[release_start:]
        # activation registry 必须先取得。
        activation_lock = release.index("pointer_activations.lock()")
        # surface targets 随后取得。
        surface_lock = release.index("surface_windows.lock()")
        # pointer slot 最后取得。
        slot_lock = release.index("pointer_slot.lock()")
        # 固定保持 activation→surface 锁序。
        self.assertLess(activation_lock, surface_lock)
        # 固定保持 surface→slot 锁序。
        self.assertLess(surface_lock, slot_lock)
        # 首项 teardown 是授权失效。
        invalidate = release.index("activations.invalidate_pointer()")
        # 焦点清除紧随授权失效。
        clear_focus = release.index("targets.clear_pointer_focus()")
        # 代理取出是锁内最后提交事实。
        take_proxy = release.index("let pointer = slot.take()")
        # 三把 guards 均健康后才开始修改。
        self.assertLess(slot_lock, invalidate)
        # 授权失效先于焦点清除。
        self.assertLess(invalidate, clear_focus)
        # 焦点清除先于代理取出。
        self.assertLess(clear_focus, take_proxy)
        # callback 清理必须发生在显式释放三把 guards 后。
        self.assertLess(release.index("drop(activations)"), release.index("discard_pointer_proxy(pointer)"))
        # 三个 owner failure 都必须独立可定位。
        for stage in ["activation registry", "surface targets", "proxy slot"]:
            # 每个 Release owner 都保留稳定诊断。
            self.assertIn(f"pointer {stage} mutex poisoned during release", release)
        # seat Release 分支必须委托 checked Component。
        branch_start = seat.index("pointer_transition == InputProxyTransition::Release")
        # keyboard Bind 标记 pointer Release 分支末尾。
        branch_end = seat.index("keyboard_transition == InputProxyTransition::Bind", branch_start)
        # 保存 seat pointer Release adapter。
        branch = seat[branch_start:branch_end]
        # adapter 只允许调用事务 Component。
        self.assertIn("release_pointer_proxy_checked(", branch)
        # adapter 不得直接锁任何 teardown owner。
        self.assertNotIn(".lock()", branch)
        # adapter 不得恢复 poisoned owner。
        self.assertNotIn("into_inner()", branch)

    # 确认 keyboard Release 在六个 owners 健康后一次提交 teardown。
    def test_keyboard_release_commits_six_owner_state_transactionally(self) -> None:
        # 读取输入代理 owner Component。
        owner = OWNER.read_text(encoding="utf-8")
        # 读取 seat capability adapter。
        seat = SEAT.read_text(encoding="utf-8")
        # 定位六 owner 锁 helper。
        helper_start = owner.index("fn lock_keyboard_release_owners")
        # 公开 Release 端口标记 helper 末尾。
        helper_end = owner.index("pub(crate) fn release_keyboard_proxy_checked", helper_start)
        # 保存锁获取实现。
        helper = owner[helper_start:helper_end]
        # 固定列出六个 lock 标记。
        locks = [
            # 第一 owner 是 surface focus。
            "surface_windows.lock()",
            # 第二 owner 是可选 events。
            "events.lock()",
            # 第三 owner 是 keys-down。
            "keys_down.lock()",
            # 第四 owner 是 held-key。
            "held_key_info.lock()",
            # 第五 owner 是 last-time。
            "last_repeat_time.lock()",
            # 最后 owner 是 keyboard slot。
            "keyboard_slot.lock()",
        ]
        # 计算每个 owner 在 helper 中的位置。
        positions = [helper.index(marker) for marker in locks]
        # 六个位置必须严格递增。
        self.assertEqual(positions, sorted(positions))
        # 六类 failure 都必须有独立诊断。
        for stage in ["surface targets", "event queue", "keys-down", "held-key", "last-time", "proxy slot"]:
            # 每个 Release owner 都保留稳定诊断。
            self.assertIn(f"keyboard {stage} mutex poisoned during release", helper)
        # 定位公开提交端口。
        release_start = owner.index("pub(crate) fn release_keyboard_proxy_checked")
        # 保存文件末尾完整提交端口。
        release = owner[release_start:]
        # 所有 locks 必须先于焦点清除。
        clear_focus = release.index("owners.targets.clear_keyboard_focus()")
        # blur 事件必须在同一事务中直接进入已持有队列。
        blur_event = release.index("UiEvent::new(UiEventType::WindowBlur")
        # keys 清理紧随事件事实。
        clear_keys = release.index("owners.keys_down.clear()")
        # held-key 与 last-time 随后清理。
        clear_held = release.index("*owners.held_key = None")
        # last-time 清理标记。
        clear_last = release.index("*owners.last_repeat = None")
        # proxy take 是锁内最后提交事实。
        take_proxy = release.index("let keyboard = owners.slot.take()")
        # 清理顺序保持 focus→event→keys。
        self.assertLess(clear_focus, blur_event)
        # blur 先于 keys 清理。
        self.assertLess(blur_event, clear_keys)
        # keys 先于 held-key。
        self.assertLess(clear_keys, clear_held)
        # held-key 先于 last-time。
        self.assertLess(clear_held, clear_last)
        # last-time 先于代理取出。
        self.assertLess(clear_last, take_proxy)
        # 协议代理清理必须发生在 guards 释放后。
        self.assertLess(release.index("drop(owners)"), release.index("discard_keyboard_proxy(keyboard)"))
        # seat keyboard Release 分支只委托 Component。
        branch_start = seat.index("keyboard_transition == InputProxyTransition::Release")
        # capability callback 结束标记分支末尾。
        branch_end = seat.index("});", branch_start)
        # 保存 seat keyboard Release adapter。
        branch = seat[branch_start:branch_end]
        # adapter 必须调用六 owner事务端口。
        self.assertIn("release_keyboard_proxy_checked(", branch)
        # adapter 不得直接锁 teardown owners。
        self.assertNotIn(".lock()", branch)
        # adapter 不得恢复 poisoned owner。
        self.assertNotIn("into_inner()", branch)
        # adapter 不得二次调用事件队列 helper。
        self.assertNotIn("enqueue_for_window", branch)

    # 确认 capability 收敛不会把 poisoned pointer slot 当作空槽。
    def test_capability_convergence_reports_pointer_slot_failure(self) -> None:
        # 读取 seat owner-thread adapter。
        seat = SEAT.read_text(encoding="utf-8")
        # 五轮循环标记收敛片段起点。
        convergence_start = seat.index("for _ in 0..5")
        # 保存文件末尾的收敛实现。
        convergence = seat[convergence_start:]
        # pointer slot 必须使用 checked match。
        slot_lock = convergence.index("match self.pointer.lock()")
        # failure 必须进入 backend pending source。
        enqueue = convergence.index("self.enqueue_failure")
        # poison 分支必须在 flush 前返回。
        early_return = convergence.index("return", enqueue)
        # 健康空槽才允许进入 flush。
        flush = convergence.index("self.flush_checked")
        # owner 检查先于 failure 转交。
        self.assertLess(slot_lock, enqueue)
        # failure 入队后立即早退。
        self.assertLess(enqueue, early_return)
        # 早退先于任何后续 flush。
        self.assertLess(early_return, flush)
        # 收敛循环不得把 lock error 转为 false。
        self.assertNotIn("unwrap_or(false)", convergence)
        # 状态失败稳定分类为 InvalidState。
        self.assertIn("Errc::InvalidState", convergence)
        # 诊断必须保留 pointer slot 与 convergence 阶段。
        self.assertIn("pointer proxy slot mutex poisoned during convergence", convergence)
        # 健康路径保持原五轮上限。
        self.assertIn("for _ in 0..5", convergence)
        # 健康路径保持 flush 后 dispatch 的调用序列。
        self.assertLess(convergence.index("self.flush_checked"), convergence.index("self.dispatch_pending_checked"))


# 直接执行本文件时运行契约测试。
if __name__ == "__main__":
    # 交给 unittest 输出稳定测试结果。
    unittest.main()
