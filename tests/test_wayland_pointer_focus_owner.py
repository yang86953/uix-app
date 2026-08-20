# 导入标准单元测试框架。
import unittest
# 使用路径对象定位仓库文件。
from pathlib import Path


# 计算仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 Wayland 子模块目录。
WAYLAND = ROOT / "src/native/backends/linux/windowing/wayland"
# 定位 pointer callback adapter。
SEAT = WAYLAND / "seat.rs"
# 定位 pointer focus owner Component。
OWNER = WAYLAND / "pointer_focus_owner.rs"
# 定位 Wayland 模块注册表。
MODULE = WAYLAND / "mod.rs"


# 验证 pointer Enter、Motion 与 Leave 的多 owner 事务边界。
class WaylandPointerFocusOwnerTests(unittest.TestCase):
    # 确认新 Component 已注册且 seat 只导入公开事务端口。
    def test_component_is_registered_and_imported(self) -> None:
        # 读取模块注册表。
        module = MODULE.read_text(encoding="utf-8")
        # 读取 seat callback adapter。
        seat = SEAT.read_text(encoding="utf-8")
        # Wayland System 必须注册 pointer focus Component。
        self.assertIn("pub(crate) mod pointer_focus_owner;", module)
        # seat 必须导入 Enter 事务端口。
        self.assertIn("handle_pointer_enter,", seat)
        # seat 必须导入 Motion 事务端口。
        self.assertIn("handle_pointer_motion,", seat)
        # seat 必须导入 Leave 事务端口。
        self.assertIn("handle_pointer_leave,", seat)
        # seat 必须导入 Axis 事务端口。
        self.assertIn("handle_pointer_axis,", seat)

    # 确认 Enter callback 不再直接访问共享 owners。
    def test_enter_callback_delegates_without_direct_owner_locks(self) -> None:
        # 读取 seat callback adapter。
        seat = SEAT.read_text(encoding="utf-8")
        # Enter 分支从协议模式开始。
        start = seat.index("wl_pointer::Event::Enter")
        # Motion 分支标记 Enter adapter 末尾。
        end = seat.index("wl_pointer::Event::Motion", start)
        # 保存完整 Enter adapter。
        branch = seat[start:end]
        # Enter 必须委托三 owner Component。
        self.assertIn("handle_pointer_enter(", branch)
        # adapter 不得直接取得共享锁。
        self.assertNotIn(".lock()", branch)
        # adapter 不得恢复 poisoned owner。
        self.assertNotIn("into_inner()", branch)
        # adapter 不得绕过 Component 直接入队。
        self.assertNotIn("enqueue_for_window", branch)

    # 确认 Motion callback 不再直接访问共享 owners。
    def test_motion_callback_delegates_without_direct_owner_locks(self) -> None:
        # 读取 seat callback adapter。
        seat = SEAT.read_text(encoding="utf-8")
        # Motion 分支从协议模式开始。
        start = seat.index("wl_pointer::Event::Motion")
        # Leave 分支标记 Motion adapter 末尾。
        end = seat.index("wl_pointer::Event::Leave", start)
        # 保存完整 Motion adapter。
        branch = seat[start:end]
        # Motion 必须委托三 owner Component。
        self.assertIn("handle_pointer_motion(", branch)
        # adapter 不得直接取得共享锁。
        self.assertNotIn(".lock()", branch)
        # adapter 不得恢复 poisoned owner。
        self.assertNotIn("into_inner()", branch)
        # adapter 不得绕过 Component 直接入队。
        self.assertNotIn("enqueue_for_window", branch)

    # 确认 Leave callback 不再直接访问共享 owners。
    def test_leave_callback_delegates_without_direct_owner_locks(self) -> None:
        # 读取 seat callback adapter。
        seat = SEAT.read_text(encoding="utf-8")
        # Leave 分支从协议模式开始。
        start = seat.index("wl_pointer::Event::Leave")
        # Button 分支标记 Leave adapter 末尾。
        end = seat.index("wl_pointer::Event::Button", start)
        # 保存完整 Leave adapter。
        branch = seat[start:end]
        # Leave 必须委托双 owner Component。
        self.assertIn("handle_pointer_leave(", branch)
        # adapter 不得直接取得共享锁。
        self.assertNotIn(".lock()", branch)
        # adapter 不得恢复 poisoned owner。
        self.assertNotIn("into_inner()", branch)
        # adapter 不得直接撤销激活授权。
        self.assertNotIn("revoke_pointer_focus", branch)

    # 确认 Axis callback 不再直接访问共享 owners。
    def test_axis_callback_delegates_without_direct_owner_locks(self) -> None:
        # 读取 seat callback adapter。
        seat = SEAT.read_text(encoding="utf-8")
        # Axis 分支从协议模式开始。
        start = seat.index("wl_pointer::Event::Axis")
        # callback 默认分支标记 Axis adapter 末尾。
        end = seat.index("_ => {}", start)
        # 保存完整 Axis adapter。
        branch = seat[start:end]
        # Axis 必须委托三 owner Component。
        self.assertIn("handle_pointer_axis(", branch)
        # adapter 不得直接取得共享锁。
        self.assertNotIn(".lock()", branch)
        # adapter 不得恢复 poisoned owner。
        self.assertNotIn("into_inner()", branch)
        # adapter 不得绕过 Component 直接入队。
        self.assertNotIn("enqueue_for_window", branch)
        # adapter 不得伪造默认滚轮位置。
        self.assertNotIn("unwrap_or_default", branch)

    # 确认 Component 对所有 poisoned owners 显式失败。
    def test_component_fails_closed_without_poison_recovery(self) -> None:
        # 读取 pointer focus owner Component。
        owner = OWNER.read_text(encoding="utf-8")
        # Component 不得恢复访问 poisoned owner。
        self.assertNotIn("into_inner()", owner)
        # Component 不得静默跳过失败锁。
        self.assertNotIn("if let Ok", owner)
        # 所有状态失败统一分类为 InvalidState。
        self.assertIn("Errc::InvalidState", owner)
        # 七个事件/owner 失败都必须稳定可定位。
        for message in [
            # Enter surface owner 失败。
            "pointer Enter surface targets mutex poisoned",
            # Enter position owner 失败。
            "pointer Enter position mutex poisoned",
            # Enter queue owner 失败。
            "pointer Enter event queue mutex poisoned",
            # Motion surface owner 失败。
            "pointer Motion surface targets mutex poisoned",
            # Motion position owner 失败。
            "pointer Motion position mutex poisoned",
            # Motion queue owner 失败。
            "pointer Motion event queue mutex poisoned",
            # Leave activation owner 失败。
            "pointer Leave activation registry mutex poisoned",
            # Leave surface owner 失败。
            "pointer Leave surface targets mutex poisoned",
            # Axis surface owner 失败。
            "pointer Axis surface targets mutex poisoned",
            # Axis position owner 失败。
            "pointer Axis position mutex poisoned",
            # Axis queue owner 失败。
            "pointer Axis event queue mutex poisoned",
        ]:
            # 每个稳定诊断都必须存在于 Component。
            self.assertIn(message, owner)

    # 确认 Enter 在三把 guards 健康后按固定顺序提交。
    def test_enter_locks_and_commits_three_owners_transactionally(self) -> None:
        # 读取 pointer focus owner Component。
        owner = OWNER.read_text(encoding="utf-8")
        # Enter 端口起点。
        start = owner.index("pub(crate) fn handle_pointer_enter")
        # Motion 端口标记 Enter 末尾。
        end = owner.index("pub(crate) fn handle_pointer_motion", start)
        # 保存完整 Enter 实现。
        enter = owner[start:end]
        # surface targets 必须最先取得。
        target_lock = enter.index("surface_windows.lock()")
        # position owner 必须随后取得。
        position_lock = enter.index("last_pointer.lock()")
        # event queue 必须最后取得。
        event_lock = enter.index("events.lock()")
        # 三把锁严格保持 surface→position→events 顺序。
        self.assertEqual([target_lock, position_lock, event_lock], sorted([target_lock, position_lock, event_lock]))
        # 已知 surface 的 focus 提交必须晚于全部锁。
        focus_commit = enter.rindex("targets.pointer_enter(surface_id)")
        # position 提交必须晚于 focus。
        position_commit = enter.index("last_pointer.position = position")
        # event 提交必须晚于 position。
        event_commit = enter.index("events.push_back")
        # 最后一把锁先于任一共享事实修改。
        self.assertLess(event_lock, focus_commit)
        # 共享事实按 focus→position→event 提交。
        self.assertEqual([focus_commit, position_commit, event_commit], sorted([focus_commit, position_commit, event_commit]))

    # 确认 Motion 在三把 guards 健康后按固定顺序提交。
    def test_motion_locks_and_commits_three_owners_transactionally(self) -> None:
        # 读取 pointer focus owner Component。
        owner = OWNER.read_text(encoding="utf-8")
        # Motion 端口起点。
        start = owner.index("pub(crate) fn handle_pointer_motion")
        # Leave 端口标记 Motion 末尾。
        end = owner.index("pub(crate) fn handle_pointer_leave", start)
        # 保存完整 Motion 实现。
        motion = owner[start:end]
        # surface targets 必须最先取得。
        target_lock = motion.index("surface_windows.lock()")
        # position owner 必须随后取得。
        position_lock = motion.index("last_pointer.lock()")
        # event queue 必须最后取得。
        event_lock = motion.index("events.lock()")
        # 三把锁严格保持 surface→position→events 顺序。
        self.assertEqual([target_lock, position_lock, event_lock], sorted([target_lock, position_lock, event_lock]))
        # position 提交必须晚于全部锁。
        position_commit = motion.index("last_pointer.position = position")
        # event 提交必须晚于 position。
        event_commit = motion.index("events.push_back")
        # 最后一把锁先于任一共享事实修改。
        self.assertLess(event_lock, position_commit)
        # 位置先于对应 UI 事件提交。
        self.assertLess(position_commit, event_commit)

    # 确认 Axis 在三把 guards 健康后按固定顺序提交。
    def test_axis_locks_and_commits_three_owners_transactionally(self) -> None:
        # 读取 pointer focus owner Component。
        owner = OWNER.read_text(encoding="utf-8")
        # Axis 端口起点。
        start = owner.index("pub(crate) fn handle_pointer_axis")
        # Leave 端口标记 Axis 末尾。
        end = owner.index("pub(crate) fn handle_pointer_leave", start)
        # 保存完整 Axis 实现。
        axis = owner[start:end]
        # surface targets 必须最先取得。
        target_lock = axis.index("surface_windows.lock()")
        # position owner 必须随后取得。
        position_lock = axis.index("last_pointer.lock()")
        # event queue 必须最后取得。
        event_lock = axis.index("events.lock()")
        # 三把锁严格保持 surface→position→events 顺序。
        self.assertEqual([target_lock, position_lock, event_lock], sorted([target_lock, position_lock, event_lock]))
        # Wheel 提交必须晚于全部锁。
        event_commit = axis.index("events.push_back")
        # 最后一把锁先于唯一共享事实修改。
        self.assertLess(event_lock, event_commit)
        # 零增量必须在第一把锁前提前停止。
        zero_guard = axis.index("delta_x == 0.0 && delta_y == 0.0")
        # 未知 Axis 不访问任何 owner。
        self.assertLess(zero_guard, target_lock)
        # Axis 不得构造默认坐标。
        self.assertNotIn("unwrap_or_default", axis)

    # 确认 Leave 沿全局锁序精确撤销授权与焦点。
    def test_leave_uses_global_activation_then_surface_order(self) -> None:
        # 读取 pointer focus owner Component。
        owner = OWNER.read_text(encoding="utf-8")
        # 保存完整 Leave 实现。
        leave = owner[owner.index("pub(crate) fn handle_pointer_leave"):]
        # activation registry 必须最先取得。
        activation_lock = leave.index("pointer_activations.lock()")
        # surface targets 必须随后取得。
        target_lock = leave.index("surface_windows.lock()")
        # 全局锁序与 capability Release 保持一致。
        self.assertLess(activation_lock, target_lock)
        # 授权撤销必须晚于两把锁与精确身份检查。
        revoke = leave.index("activations.revoke_pointer_focus")
        # 焦点清理必须紧随授权撤销。
        clear_focus = leave.index("targets.pointer_leave(surface_id)")
        # 两把 guards 健康后才允许修改。
        self.assertLess(target_lock, revoke)
        # 安全优先撤销授权，再清除 surface focus。
        self.assertLess(revoke, clear_focus)
        # 迟到 Leave 必须显式比较 surface 身份。
        self.assertIn("focused_surface != surface_id", leave)


# 直接执行本文件时运行契约测试。
if __name__ == "__main__":
    # 交给 unittest 输出稳定测试结果。
    unittest.main()
