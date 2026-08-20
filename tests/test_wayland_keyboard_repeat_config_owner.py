# 导入标准单元测试框架。
import unittest
# 使用路径对象定位仓库文件。
from pathlib import Path


# 计算仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 Wayland 子模块目录。
WAYLAND = ROOT / "src/native/backends/linux/windowing/wayland"
# 定位 keyboard callback adapter。
SEAT = WAYLAND / "seat.rs"
# 定位 keyboard repeat config Component。
OWNER = WAYLAND / "keyboard_repeat_config_owner.rs"
# 定位 Wayland 模块注册表。
MODULE = WAYLAND / "mod.rs"


# 验证 keyboard RepeatInfo 的双 owner 事务边界。
class WaylandKeyboardRepeatConfigOwnerTests(unittest.TestCase):
    # 确认新 Component 已注册且 seat 导入事务端口。
    def test_component_is_registered_and_imported(self) -> None:
        # 读取模块注册表。
        module = MODULE.read_text(encoding="utf-8")
        # 读取 seat callback adapter。
        seat = SEAT.read_text(encoding="utf-8")
        # Wayland System 必须注册 repeat config Component。
        self.assertIn("pub(crate) mod keyboard_repeat_config_owner;", module)
        # seat 必须导入 RepeatInfo 事务端口。
        self.assertIn("handle_keyboard_repeat_info;", seat)

    # 确认 RepeatInfo callback 只转交原始协议配置。
    def test_callback_delegates_without_direct_owner_locks(self) -> None:
        # 读取 seat callback adapter。
        seat = SEAT.read_text(encoding="utf-8")
        # RepeatInfo 分支从协议模式开始。
        start = seat.index("wl_keyboard::Event::RepeatInfo")
        # callback 默认分支标记 adapter 末尾。
        end = seat.index("_ => {}", start)
        # 保存完整 RepeatInfo adapter。
        branch = seat[start:end]
        # callback 必须委托双 owner Component。
        self.assertIn("handle_keyboard_repeat_info(", branch)
        # adapter 不得直接取得共享锁。
        self.assertNotIn(".lock()", branch)
        # adapter 不得静默跳过失败锁。
        self.assertNotIn("if let Ok", branch)
        # rate 与 delay 必须原样转交。
        self.assertIn("rate,", branch)
        # delay 必须作为独立参数转交。
        self.assertIn("delay,", branch)

    # 确认 Component 对两个 poisoned owners 显式失败。
    def test_component_fails_closed_without_poison_recovery(self) -> None:
        # 读取 repeat config Component。
        owner = OWNER.read_text(encoding="utf-8")
        # Component 不得恢复访问 poisoned owner。
        self.assertNotIn("into_inner()", owner)
        # Component 不得静默跳过失败锁。
        self.assertNotIn("if let Ok", owner)
        # 所有状态失败统一分类为 InvalidState。
        self.assertIn("Errc::InvalidState", owner)
        # rate owner 失败必须稳定可定位。
        self.assertIn("RepeatInfo repeat-rate mutex poisoned", owner)
        # delay owner 失败必须稳定可定位。
        self.assertIn("RepeatInfo repeat-delay mutex poisoned", owner)

    # 确认双 owners 沿固定顺序获取。
    def test_locks_rate_before_delay(self) -> None:
        # 读取 repeat config Component。
        owner = OWNER.read_text(encoding="utf-8")
        # 保存公开事务端口。
        handler = owner[owner.index("pub(crate) fn handle_keyboard_repeat_info"):]
        # repeat-rate 必须最先取得。
        rate_lock = handler.index("repeat_rate.lock()")
        # repeat-delay 必须随后取得。
        delay_lock = handler.index("repeat_delay.lock()")
        # 固定保持 rate→delay 顺序。
        self.assertLess(rate_lock, delay_lock)

    # 确认全部 guards 健康后一次提交原始配置。
    def test_commits_both_values_after_all_locks(self) -> None:
        # 读取 repeat config Component。
        owner = OWNER.read_text(encoding="utf-8")
        # 保存公开事务端口。
        handler = owner[owner.index("pub(crate) fn handle_keyboard_repeat_info"):]
        # 最后一把 owner 是 repeat-delay。
        delay_lock = handler.index("repeat_delay.lock()")
        # rate 是第一份提交事实。
        rate_commit = handler.index("*current_rate = rate")
        # delay 是第二份提交事实。
        delay_commit = handler.index("*current_delay = delay")
        # 两把 guards 健康后才允许修改。
        self.assertLess(delay_lock, rate_commit)
        # 双配置保持 rate→delay 提交顺序。
        self.assertLess(rate_commit, delay_commit)
        # Component 不得重写有符号协议值。
        for forbidden in ["max(", "min(", "clamp(", "abs(", "as u"]:
            # 禁止任何策略性数值改写。
            self.assertNotIn(forbidden, handler)


# 直接执行本文件时运行契约测试。
if __name__ == "__main__":
    # 交给 unittest 输出稳定测试结果。
    unittest.main()
