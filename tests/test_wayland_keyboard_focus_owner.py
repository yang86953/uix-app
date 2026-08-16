# 导入标准单元测试框架。
import unittest
# 使用路径对象定位仓库文件。
from pathlib import Path


# 计算仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 Wayland 子模块目录。
WAYLAND = ROOT / "src/native/backends/linux/wayland"
# 定位 keyboard callback adapter。
SEAT = WAYLAND / "seat.rs"
# 定位 keyboard focus owner Component。
OWNER = WAYLAND / "keyboard_focus_owner.rs"
# 定位 Wayland 模块注册表。
MODULE = WAYLAND / "mod.rs"
# 定位共享 surface 路由契约。
TARGETS = ROOT / "src/native/windowing/shared/window_target.rs"


# 验证 keyboard Enter/Leave 的多 owner 事务边界。
class WaylandKeyboardFocusOwnerTests(unittest.TestCase):
    # 确认新 Component 已注册且 seat 导入两个事务端口。
    def test_component_is_registered_and_imported(self) -> None:
        # 读取模块注册表。
        module = MODULE.read_text(encoding="utf-8")
        # 读取 seat callback adapter。
        seat = SEAT.read_text(encoding="utf-8")
        # Wayland System 必须注册 keyboard focus Component。
        self.assertIn("pub(crate) mod keyboard_focus_owner;", module)
        # seat 必须导入 Enter 事务端口。
        self.assertIn("handle_keyboard_enter,", seat)
        # seat 必须导入 Leave 事务端口。
        self.assertIn("handle_keyboard_leave,", seat)

    # 确认 Enter callback 只转交 surface 身份。
    def test_enter_callback_delegates_without_direct_owner_locks(self) -> None:
        # 读取 seat callback adapter。
        seat = SEAT.read_text(encoding="utf-8")
        # keyboard Enter 分支从协议模式开始。
        start = seat.index("wl_keyboard::Event::Enter")
        # Leave 分支标记 Enter adapter 末尾。
        end = seat.index("wl_keyboard::Event::Leave", start)
        # 保存完整 Enter adapter。
        branch = seat[start:end]
        # Enter 必须委托五 owner Component。
        self.assertIn("handle_keyboard_enter(", branch)
        # adapter 不得直接取得共享锁。
        self.assertNotIn(".lock()", branch)
        # adapter 不得恢复 poisoned owner。
        self.assertNotIn("into_inner()", branch)
        # adapter 不得绕过 Component 直接入队。
        self.assertNotIn("push_back", branch)
        # adapter 不得直接修改 keyboard focus。
        self.assertNotIn("keyboard_enter", branch.replace("handle_keyboard_enter", ""))

    # 确认 Leave callback 只转交 surface 身份。
    def test_leave_callback_delegates_without_direct_owner_locks(self) -> None:
        # 读取 seat callback adapter。
        seat = SEAT.read_text(encoding="utf-8")
        # keyboard Leave 分支从协议模式开始。
        start = seat.index("wl_keyboard::Event::Leave")
        # Key 分支标记 Leave adapter 末尾。
        end = seat.index("wl_keyboard::Event::Key", start)
        # 保存完整 Leave adapter。
        branch = seat[start:end]
        # Leave 必须委托五 owner Component。
        self.assertIn("handle_keyboard_leave(", branch)
        # adapter 不得直接取得共享锁。
        self.assertNotIn(".lock()", branch)
        # adapter 不得恢复 poisoned owner。
        self.assertNotIn("into_inner()", branch)
        # adapter 不得绕过 Component 直接入队。
        self.assertNotIn("push_back", branch)
        # adapter 不得直接修改 keyboard focus。
        self.assertNotIn("keyboard_leave", branch.replace("handle_keyboard_leave", ""))

    # 确认共享路由提供只读 keyboard focus 身份投影。
    def test_surface_targets_exposes_keyboard_focus_identity(self) -> None:
        # 读取共享 surface 路由契约。
        targets = TARGETS.read_text(encoding="utf-8")
        # 定位只读 keyboard identity 端口。
        start = targets.index("pub(crate) fn keyboard_target_identity")
        # 下一 clear 端口标记实现末尾。
        end = targets.index("pub(crate) fn clear_keyboard_focus", start)
        # 保存完整只读投影。
        identity = targets[start:end]
        # 投影必须同时返回 surface 与 WindowId。
        self.assertIn("Option<(u32, WindowId)>", identity)
        # 投影只读取 keyboard focus。
        self.assertIn("self.keyboard_focus.map", identity)
        # 投影不得修改 focus。
        self.assertNotIn("self.keyboard_focus =", identity)

    # 确认 Component 对所有 poisoned owners 显式失败。
    def test_component_fails_closed_without_poison_recovery(self) -> None:
        # 读取 keyboard focus owner Component。
        owner = OWNER.read_text(encoding="utf-8")
        # Component 不得恢复访问 poisoned owner。
        self.assertNotIn("into_inner()", owner)
        # Component 不得静默跳过失败锁。
        self.assertNotIn("if let Ok", owner)
        # 所有状态失败统一分类为 InvalidState。
        self.assertIn("Errc::InvalidState", owner)
        # Enter 与 Leave 的五个 owners 必须稳定可定位。
        for event in ["Enter", "Leave"]:
            # surface owner 使用公开端口独立诊断。
            self.assertIn(f"keyboard {event} surface targets mutex poisoned", owner)
            # 其余四类 owner 使用统一锁 helper 诊断。
            for stage in ["event queue", "keys-down", "held-key", "last-repeat"]:
                # 每个阶段都必须保留事件身份。
                self.assertIn(f"keyboard {event} {stage} mutex poisoned", owner)

    # 确认共享锁 helper 沿 capability Release 的权威顺序。
    def test_state_helper_uses_events_keys_held_last_order(self) -> None:
        # 读取 keyboard focus owner Component。
        owner = OWNER.read_text(encoding="utf-8")
        # 统一锁 helper 起点。
        start = owner.index("fn lock_keyboard_state_owners")
        # blur helper 标记锁 helper 末尾。
        end = owner.index("fn push_window_blur", start)
        # 保存完整锁 helper。
        helper = owner[start:end]
        # event queue 是 surface 后的可选第二 owner。
        event_lock = helper.index("events.lock()")
        # keys-down 是固定第三 owner。
        keys_lock = helper.index("keys_down.lock()")
        # held-key 是固定第四 owner。
        held_lock = helper.index("held_key_info.lock()")
        # 先定位 last-repeat guard 的构造语句。
        last_owner = helper.index("let last_repeat = last_repeat_time")
        # last-repeat 是固定第五 owner。
        last_lock = helper.index(".lock()", last_owner)
        # 四个后续 locks 必须严格单调。
        locks = [event_lock, keys_lock, held_lock, last_lock]
        # 顺序必须与 capability Release 一致。
        self.assertEqual(locks, sorted(locks))

    # 确认 Enter 在全部 guards 健康后提交焦点、事件与清理。
    def test_enter_commits_focus_events_and_repeat_state_transactionally(self) -> None:
        # 读取 keyboard focus owner Component。
        owner = OWNER.read_text(encoding="utf-8")
        # Enter 端口起点。
        start = owner.index("pub(crate) fn handle_keyboard_enter")
        # Leave 端口标记 Enter 末尾。
        end = owner.index("pub(crate) fn handle_keyboard_leave", start)
        # 保存完整 Enter 实现。
        enter = owner[start:end]
        # surface lock 必须先于统一后续 owner helper。
        surface_lock = enter.index("surface_windows.lock()")
        # helper 调用代表其余 guards 全部健康。
        state_locks = enter.index("lock_keyboard_state_owners(")
        # keyboard focus 是第一份可变事实。
        focus_commit = enter.index("owners.targets.keyboard_enter(surface_id)")
        # keys 清理在焦点事件后提交。
        keys_commit = enter.index("owners.keys_down.clear()")
        # held-key 清理随后提交。
        held_commit = enter.index("*owners.held_key = None")
        # last-repeat 清理最后提交。
        last_commit = enter.index("*owners.last_repeat = None")
        # 第一 owner 必须先于后续 helper。
        self.assertLess(surface_lock, state_locks)
        # 全部 guards 健康后才修改 focus。
        self.assertLess(state_locks, focus_commit)
        # 输入状态保持 keys→held→last 提交顺序。
        self.assertEqual([keys_commit, held_commit, last_commit], sorted([keys_commit, held_commit, last_commit]))
        # 旧窗口 Blur 必须先于新窗口 Focus。
        self.assertLess(enter.index("push_window_blur"), enter.index("push_window_focus"))

    # 确认 Leave 精确匹配 surface 后才获取其余 owners 并提交。
    def test_leave_rejects_stale_surface_before_transaction(self) -> None:
        # 读取 keyboard focus owner Component。
        owner = OWNER.read_text(encoding="utf-8")
        # 保存完整 Leave 实现。
        leave = owner[owner.index("pub(crate) fn handle_keyboard_leave"):]
        # 先读取 surface 与窗口身份快照。
        identity = leave.index("targets.keyboard_target_identity()")
        # 随后拒绝迟到的其他 surface。
        stale_guard = leave.index("focused_surface != surface_id")
        # 最后才取得其余 owners。
        state_locks = leave.index("lock_keyboard_state_owners(")
        # 身份读取、迟到拒绝、事务锁必须依次发生。
        self.assertEqual([identity, stale_guard, state_locks], sorted([identity, stale_guard, state_locks]))
        # 焦点清理必须晚于全部 guards。
        clear_focus = leave.index("owners.targets.keyboard_leave(surface_id)")
        # WindowBlur 随后进入同一事务。
        blur = leave.index("push_window_blur(event_queue, window_id)")
        # 全部 guards 健康后才清除焦点。
        self.assertLess(state_locks, clear_focus)
        # 清焦点后投递对应旧窗口 Blur。
        self.assertLess(clear_focus, blur)


# 直接执行本文件时运行契约测试。
if __name__ == "__main__":
    # 交给 unittest 输出稳定测试结果。
    unittest.main()
