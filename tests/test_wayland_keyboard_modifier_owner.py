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
# 定位 keyboard modifier owner Component。
OWNER = WAYLAND / "keyboard_modifier_owner.rs"
# 定位 Wayland 模块注册表。
MODULE = WAYLAND / "mod.rs"


# 验证 keyboard Modifiers 的三 owner 事务边界。
class WaylandKeyboardModifierOwnerTests(unittest.TestCase):
    # 确认新 Component 已注册且 seat 导入事务端口。
    def test_component_is_registered_and_imported(self) -> None:
        # 读取模块注册表。
        module = MODULE.read_text(encoding="utf-8")
        # 读取 seat callback adapter。
        seat = SEAT.read_text(encoding="utf-8")
        # Wayland System 必须注册 keyboard modifier Component。
        self.assertIn("pub(crate) mod keyboard_modifier_owner;", module)
        # seat 必须导入 Modifiers 事务端口。
        self.assertIn("handle_keyboard_modifiers;", seat)

    # 确认 Modifiers callback 只转交协议位图并委托 Component。
    def test_callback_delegates_without_direct_owner_locks(self) -> None:
        # 读取 seat callback adapter。
        seat = SEAT.read_text(encoding="utf-8")
        # Modifiers 分支从协议模式开始。
        start = seat.index("wl_keyboard::Event::Modifiers")
        # RepeatInfo 分支标记 adapter 末尾。
        end = seat.index("wl_keyboard::Event::RepeatInfo", start)
        # 保存完整 Modifiers adapter。
        branch = seat[start:end]
        # callback 必须委托三 owner Component。
        self.assertIn("handle_keyboard_modifiers(", branch)
        # adapter 不得直接取得共享锁。
        self.assertNotIn(".lock()", branch)
        # adapter 不得静默跳过失败锁。
        self.assertNotIn("if let Ok", branch)
        # adapter 不得直接修改 KeyMod。
        self.assertNotIn("KeyMod::", branch)
        # 三份协议位图都必须转交。
        for field in ["mods_depressed", "mods_latched", "mods_locked"]:
            # 每个协议字段必须出现在调用参数中。
            self.assertIn(field, branch)

    # 确认 Component 对所有 poisoned owners 显式失败。
    def test_component_fails_closed_without_poison_recovery(self) -> None:
        # 读取 keyboard modifier owner Component。
        owner = OWNER.read_text(encoding="utf-8")
        # Component 不得恢复访问 poisoned owner。
        self.assertNotIn("into_inner()", owner)
        # Component 不得静默跳过失败锁。
        self.assertNotIn("if let Ok", owner)
        # 所有状态失败统一分类为 InvalidState。
        self.assertIn("Errc::InvalidState", owner)
        # 三个 owner 失败必须稳定可定位。
        for stage in ["modifier state", "held-key", "last-repeat"]:
            # 每个阶段都保留 Modifiers 身份。
            self.assertIn(f"keyboard Modifiers {stage} mutex poisoned", owner)

    # 确认协议位图按既有平台中立语义映射。
    def test_modifier_bit_mapping_preserves_existing_semantics(self) -> None:
        # 读取 keyboard modifier owner Component。
        owner = OWNER.read_text(encoding="utf-8")
        # 定位纯位图映射函数。
        start = owner.index("fn key_mod_from_wayland_bits")
        # 公开事务端口标记纯函数末尾。
        end = owner.index("pub(crate) fn handle_keyboard_modifiers", start)
        # 保存完整位图映射。
        mapping = owner[start:end]
        # 四个既有位与 KeyMod 必须一一对应。
        expected = [("1", "SHIFT"), ("4", "CTRL"), ("8", "ALT"), ("16", "SUPER")]
        # 逐项验证稳定映射。
        for bit, modifier in expected:
            # 每个条件必须检查对应位。
            self.assertIn(f"combined & {bit} != 0", mapping)
            # 每个命中必须合并对应 KeyMod。
            self.assertIn(f"modifiers |= KeyMod::{modifier}", mapping)
        # 空位图必须从 NONE 开始。
        self.assertIn("let mut modifiers = KeyMod::NONE", mapping)

    # 确认三个协议位图在任何共享状态修改前合并。
    def test_protocol_bitmaps_are_combined_before_commit(self) -> None:
        # 读取 keyboard modifier owner Component。
        owner = OWNER.read_text(encoding="utf-8")
        # 保存公开事务端口。
        handler = owner[owner.index("pub(crate) fn handle_keyboard_modifiers"):]
        # 三个位图必须通过 OR 合并。
        combined = handler.index("mods_depressed | mods_latched | mods_locked")
        # 修饰快照提交发生在合并后。
        modifier_commit = handler.index("*modifiers = key_mod_from_wayland_bits(combined)")
        # 合并必须先于共享状态修改。
        self.assertLess(combined, modifier_commit)

    # 确认三个 owners 沿固定顺序获取并一次提交。
    def test_locks_and_commits_three_owners_transactionally(self) -> None:
        # 读取 keyboard modifier owner Component。
        owner = OWNER.read_text(encoding="utf-8")
        # 保存公开事务端口。
        handler = owner[owner.index("pub(crate) fn handle_keyboard_modifiers"):]
        # modifier state 必须最先取得。
        modifier_lock = handler.index("modifier_state.lock()")
        # held-key 必须随后取得。
        held_lock = handler.index("held_key_info.lock()")
        # last-repeat 必须最后取得。
        last_lock = handler.index("last_repeat_time.lock()")
        # 三把锁严格保持 mods→held→last 顺序。
        locks = [modifier_lock, held_lock, last_lock]
        # 顺序必须单调递增。
        self.assertEqual(locks, sorted(locks))
        # 修饰快照是第一份提交事实。
        modifier_commit = handler.index("*modifiers = key_mod_from_wayland_bits(combined)")
        # held-key 清理随后提交。
        held_commit = handler.index("*held_key = None")
        # last-repeat 清理最后提交。
        last_commit = handler.index("*last_repeat = None")
        # 最后一把锁必须先于任一修改。
        self.assertLess(last_lock, modifier_commit)
        # 三份状态保持修饰→held→last 提交顺序。
        commits = [modifier_commit, held_commit, last_commit]
        # 顺序必须单调递增。
        self.assertEqual(commits, sorted(commits))


# 直接执行本文件时运行契约测试。
if __name__ == "__main__":
    # 交给 unittest 输出稳定测试结果。
    unittest.main()
