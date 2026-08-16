# 导入标准单元测试框架。
import unittest
# 使用路径对象定位仓库文件。
from pathlib import Path


# 计算仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 Wayland 子模块目录。
WAYLAND = ROOT / "src/native/backends/linux/wayland"
# 定位 pointer callback adapter。
SEAT = WAYLAND / "seat.rs"
# 定位 pointer Button owner Component。
OWNER = WAYLAND / "pointer_button_owner.rs"
# 定位 Wayland 模块注册表。
MODULE = WAYLAND / "mod.rs"


# 验证 pointer Button 的多 owner 事务边界。
class WaylandPointerButtonOwnerTests(unittest.TestCase):
    # 确认新 Component 已注册且 seat 导入两个事务端口。
    def test_component_is_registered_and_imported(self) -> None:
        # 读取模块注册表。
        module = MODULE.read_text(encoding="utf-8")
        # 读取 seat callback adapter。
        seat = SEAT.read_text(encoding="utf-8")
        # Wayland System 必须注册 pointer Button Component。
        self.assertIn("pub(crate) mod pointer_button_owner;", module)
        # seat 必须导入 Press 事务端口。
        self.assertIn("handle_pointer_button_pressed,", seat)
        # seat 必须导入 Release 事务端口。
        self.assertIn("handle_pointer_button_released,", seat)

    # 确认 Button callback 只映射协议值并委托 Component。
    def test_button_callback_delegates_without_direct_owner_locks(self) -> None:
        # 读取 seat callback adapter。
        seat = SEAT.read_text(encoding="utf-8")
        # Button 分支从协议模式开始。
        start = seat.index("wl_pointer::Event::Button")
        # Axis 分支标记 Button adapter 末尾。
        end = seat.index("wl_pointer::Event::Axis", start)
        # 保存完整 Button adapter。
        branch = seat[start:end]
        # Press 必须委托独立 Component。
        self.assertIn("handle_pointer_button_pressed(", branch)
        # Release 必须委托独立 Component。
        self.assertIn("handle_pointer_button_released(", branch)
        # adapter 不得直接取得共享锁。
        self.assertNotIn(".lock()", branch)
        # adapter 不得恢复 poisoned owner。
        self.assertNotIn("into_inner()", branch)
        # adapter 不得伪造默认坐标。
        self.assertNotIn("unwrap_or_default", branch)
        # adapter 不得绕过 Component 直接入队。
        self.assertNotIn("enqueue_for_window", branch)
        # adapter 不得直接签发授权。
        self.assertNotIn("issue_primary_press", branch)
        # adapter 不得直接撤销授权。
        self.assertNotIn("revoke_primary_press", branch)
        # 未知 Linux 按钮保持既有 None 语义。
        self.assertIn("_ => MouseButton::None", branch)

    # 确认 Component 对所有 poisoned owners 显式失败。
    def test_component_fails_closed_without_poison_recovery(self) -> None:
        # 读取 pointer Button owner Component。
        owner = OWNER.read_text(encoding="utf-8")
        # Component 不得恢复访问 poisoned owner。
        self.assertNotIn("into_inner()", owner)
        # Component 不得静默跳过失败锁。
        self.assertNotIn("if let Ok", owner)
        # Component 不得伪造默认坐标。
        self.assertNotIn("unwrap_or_default", owner)
        # 所有状态失败统一分类为 InvalidState。
        self.assertIn("Errc::InvalidState", owner)
        # Press 的五个 owner 失败必须稳定可定位。
        for stage in ["activation registry", "surface targets", "position", "event queue", "input serial"]:
            # 每个 Press owner 都保留阶段诊断。
            self.assertIn(f"Button Press {stage} mutex poisoned", owner)
        # Release 的四个 owner 失败必须稳定可定位。
        for stage in ["activation registry", "surface targets", "position", "event queue"]:
            # 每个 Release owner 都保留阶段诊断。
            self.assertIn(f"Button Release {stage} mutex poisoned", owner)

    # 确认 Press 沿五 owner 全局顺序获取 guards。
    def test_press_uses_activation_surface_position_events_serial_order(self) -> None:
        # 读取 pointer Button owner Component。
        owner = OWNER.read_text(encoding="utf-8")
        # Press 端口起点。
        start = owner.index("pub(crate) fn handle_pointer_button_pressed")
        # Release 端口标记 Press 末尾。
        end = owner.index("pub(crate) fn handle_pointer_button_released", start)
        # 保存完整 Press 实现。
        press = owner[start:end]
        # activation registry 是主键第一 owner。
        activation_lock = press.index("pointer_activations.lock()")
        # surface targets 必须随后取得。
        surface_lock = press.index("surface_windows.lock()")
        # pointer position 必须随后取得。
        position_lock = press.index("last_pointer.lock()")
        # event queue 必须随后取得。
        event_lock = press.index("events.lock()")
        # input serial 必须最后取得。
        serial_lock = press.index("input_serial.lock()")
        # 五把锁严格保持全局顺序。
        locks = [activation_lock, surface_lock, position_lock, event_lock, serial_lock]
        # 顺序必须单调递增。
        self.assertEqual(locks, sorted(locks))
        # 非主键必须显式跳过 activation owner。
        self.assertIn("button == MouseButton::Left", press)
        # 无焦点必须在 position owner 前停止。
        focus_guard = press.index("targets.pointer_target_identity()")
        # 焦点检查必须早于 position lock。
        self.assertLess(focus_guard, position_lock)

    # 确认 Press 在最后一把 guard 健康后一次提交三份共享事实。
    def test_press_commits_activation_serial_and_event_after_all_locks(self) -> None:
        # 读取 pointer Button owner Component。
        owner = OWNER.read_text(encoding="utf-8")
        # Press 端口起点。
        start = owner.index("pub(crate) fn handle_pointer_button_pressed")
        # Release 端口标记 Press 末尾。
        end = owner.index("pub(crate) fn handle_pointer_button_released", start)
        # 保存完整 Press 实现。
        press = owner[start:end]
        # 最后一把 owner 是 input serial。
        serial_lock = press.index("input_serial.lock()")
        # 主键授权签发是第一份可变事实。
        activation_commit = press.index("activations.issue_primary_press")
        # serial 记录是第二份可变事实。
        serial_commit = press.index("input_serial.record(serial)")
        # PointerDown 入队是最后一份可变事实。
        event_commit = press.index("events.push_back")
        # 全部 guards 健康后才开始修改。
        self.assertLess(serial_lock, activation_commit)
        # 授权、serial、event 保持安全提交顺序。
        self.assertEqual([activation_commit, serial_commit, event_commit], sorted([activation_commit, serial_commit, event_commit]))
        # 事件必须绑定同一焦点快照窗口。
        self.assertIn("pointer_event.for_window(window_id)", press)

    # 确认 Release 沿四 owner 全局顺序获取 guards。
    def test_release_uses_activation_surface_position_events_order(self) -> None:
        # 读取 pointer Button owner Component。
        owner = OWNER.read_text(encoding="utf-8")
        # 保存完整 Release 实现。
        release = owner[owner.index("pub(crate) fn handle_pointer_button_released"):]
        # activation registry 是主键第一 owner。
        activation_lock = release.index("pointer_activations.lock()")
        # surface targets 必须随后取得。
        surface_lock = release.index("surface_windows.lock()")
        # pointer position 必须随后取得。
        position_lock = release.index("last_pointer.lock()")
        # event queue 必须最后取得。
        event_lock = release.index("events.lock()")
        # 四把锁严格保持全局顺序。
        locks = [activation_lock, surface_lock, position_lock, event_lock]
        # 顺序必须单调递增。
        self.assertEqual(locks, sorted(locks))
        # 非主键必须显式跳过 activation owner。
        self.assertIn("button == MouseButton::Left", release)

    # 确认 Release 在全部 guards 健康后撤销授权并投递事件。
    def test_release_commits_revocation_before_pointer_up(self) -> None:
        # 读取 pointer Button owner Component。
        owner = OWNER.read_text(encoding="utf-8")
        # 保存完整 Release 实现。
        release = owner[owner.index("pub(crate) fn handle_pointer_button_released"):]
        # 最后一把 owner 是 event queue。
        event_lock = release.index("events.lock()")
        # 主键授权撤销必须晚于所有 locks。
        revoke = release.index("activations.revoke_primary_press")
        # PointerUp 入队必须晚于授权撤销。
        event_commit = release.index("events.push_back")
        # 全部 guards 健康后才开始修改。
        self.assertLess(event_lock, revoke)
        # 授权先失效，再交付抬起事件。
        self.assertLess(revoke, event_commit)
        # 事件必须绑定抬起时的焦点窗口。
        self.assertIn("for_window(window_id)", release)


# 直接执行本文件时运行契约测试。
if __name__ == "__main__":
    # 交给 unittest 输出稳定测试结果。
    unittest.main()
