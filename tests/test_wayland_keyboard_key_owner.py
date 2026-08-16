# 使用路径对象读取仓库内的 Rust 契约实现。
from pathlib import Path
# 使用标准库测试框架保持聚焦验证无额外依赖。
import unittest

# 解析测试文件所在仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 keyboard Key owner Component。
OWNER = ROOT / "src/native/backends/linux/wayland/keyboard_key_owner.rs"
# 定位 Wayland seat 协议 adapter。
SEAT = ROOT / "src/native/backends/linux/wayland/seat.rs"
# 定位 Wayland 模块登记表。
MODULE = ROOT / "src/native/backends/linux/wayland/mod.rs"


# 验证 keyboard Key 的 owner 边界、锁序与提交顺序。
class WaylandKeyboardKeyOwnerTests(unittest.TestCase):
    # 确认模块登记和协议 adapter 只保留显式状态分派。
    def test_adapter_delegates_only_known_key_states(self) -> None:
        # 读取模块登记表。
        module_source = MODULE.read_text(encoding="utf-8")
        # 新 Component 必须进入 Wayland 私有模块图。
        self.assertIn("pub(crate) mod keyboard_key_owner;", module_source)
        # 读取 seat adapter。
        seat_source = SEAT.read_text(encoding="utf-8")
        # 截取单个 Key callback 分支。
        key_start = seat_source.index("wl_keyboard::Event::Key {")
        # Modifiers 分支标记 Key adapter 末尾。
        key_end = seat_source.index("wl_keyboard::Event::Modifiers {", key_start)
        # 保存待审计的 Key adapter。
        key_adapter = seat_source[key_start:key_end]
        # adapter 必须显式匹配协议状态。
        self.assertIn("match state", key_adapter)
        # 已知 Pressed 委托按下端口。
        self.assertIn("handle_keyboard_key_pressed(", key_adapter)
        # 已知 Released 委托抬起端口。
        self.assertIn("handle_keyboard_key_released(", key_adapter)
        # 未知 WEnum 必须拥有明确丢弃分支。
        self.assertIn("WEnum::Unknown(_)", key_adapter)
        # 非穷尽 KeyState 的未来已解码值也必须明确丢弃。
        self.assertIn("WEnum::Value(_) =>", key_adapter)
        # adapter 不得继续直接取得任何共享 owner 锁。
        self.assertNotIn(".lock()", key_adapter)
        # adapter 不得恢复 poisoned owner。
        self.assertNotIn("into_inner()", key_adapter)
        # adapter 不得用 else 把未知状态归类为 Release。
        self.assertNotIn("if state ==", key_adapter)

    # 确认 Press helper 固定取得八类 owner guards。
    def test_press_locks_all_owners_in_global_order(self) -> None:
        # 读取 owner Component。
        source = OWNER.read_text(encoding="utf-8")
        # 限定 Press guard acquisition helper。
        press_start = source.index("fn lock_press_owners")
        # Release helper 标记 Press helper 末尾。
        press_end = source.index("fn lock_release_owners", press_start)
        # 保存 Press 锁序片段。
        press = source[press_start:press_end]
        # 第一 owner 是 surface route。
        surface_lock = press.index("let targets = surface_windows")
        # 第二 owner 是 event queue。
        event_lock = press.index("let events = events")
        # 第三 owner 是 modifiers。
        modifier_lock = press.index("let modifiers = modifiers")
        # 第四 owner 是 repeat-rate。
        rate_lock = press.index("let repeat_rate = repeat_rate")
        # 第五 owner 是 keys-down。
        keys_lock = press.index("let keys_down = keys_down")
        # 第六 owner 是 input-serial。
        serial_lock = press.index("let input_serial = input_serial")
        # 第七 owner 是 held-key。
        held_lock = press.index("let held_key = held_key_info")
        # 第八 owner 是 last-repeat。
        last_lock = press.index("let last_repeat = last_repeat_time")
        # 固定 surface→events 顺序。
        self.assertLess(surface_lock, event_lock)
        # 固定 events→modifiers 顺序。
        self.assertLess(event_lock, modifier_lock)
        # 固定 modifiers→repeat-rate 顺序。
        self.assertLess(modifier_lock, rate_lock)
        # 固定 repeat-rate→keys-down 顺序。
        self.assertLess(rate_lock, keys_lock)
        # 固定 keys-down→input-serial 顺序。
        self.assertLess(keys_lock, serial_lock)
        # 固定 input-serial→held-key 顺序。
        self.assertLess(serial_lock, held_lock)
        # 固定 held-key→last-repeat 顺序。
        self.assertLess(held_lock, last_lock)
        # Press helper 不得恢复任何 poisoned owner。
        self.assertNotIn("into_inner()", press)
        # Press helper 不得用默认修饰快照继续提交。
        self.assertNotIn("unwrap_or(KeyMod::NONE)", press)
        # 八类 owner 必须各自提供阶段诊断。
        self.assertEqual(press.count("mutex poisoned"), 8)

    # 确认 Release helper 固定取得六类 owner guards。
    def test_release_locks_all_owners_in_global_order(self) -> None:
        # 读取 owner Component。
        source = OWNER.read_text(encoding="utf-8")
        # 限定 Release guard acquisition helper。
        release_start = source.index("fn lock_release_owners")
        # Press 端口标记 Release helper 末尾。
        release_end = source.index("pub(crate) fn handle_keyboard_key_pressed", release_start)
        # 保存 Release 锁序片段。
        release = source[release_start:release_end]
        # 第一 owner 是 surface route。
        surface_lock = release.index("let targets = surface_windows")
        # 第二 owner 是 event queue。
        event_lock = release.index("let events = events")
        # 第三 owner 是 modifiers。
        modifier_lock = release.index("let modifiers = modifiers")
        # 第四 owner 是 keys-down。
        keys_lock = release.index("let keys_down = keys_down")
        # 第五 owner 是 held-key。
        held_lock = release.index("let held_key = held_key_info")
        # 第六 owner 是 last-repeat。
        last_lock = release.index("let last_repeat = last_repeat_time")
        # 固定 surface→events 顺序。
        self.assertLess(surface_lock, event_lock)
        # 固定 events→modifiers 顺序。
        self.assertLess(event_lock, modifier_lock)
        # 固定 modifiers→keys-down 顺序。
        self.assertLess(modifier_lock, keys_lock)
        # 固定 keys-down→held-key 顺序。
        self.assertLess(keys_lock, held_lock)
        # 固定 held-key→last-repeat 顺序。
        self.assertLess(held_lock, last_lock)
        # Release helper 不得恢复任何 poisoned owner。
        self.assertNotIn("into_inner()", release)
        # Release helper 不得用默认修饰快照继续提交。
        self.assertNotIn("unwrap_or(KeyMod::NONE)", release)
        # 六类 owner 必须各自提供阶段诊断。
        self.assertEqual(release.count("mutex poisoned"), 6)

    # 确认 Press 在 guard 健康和去重判定后保持既有提交语义。
    def test_press_commits_serial_events_and_repeat_state_once(self) -> None:
        # 读取 owner Component。
        source = OWNER.read_text(encoding="utf-8")
        # 限定 Press 端口。
        press_start = source.index("pub(crate) fn handle_keyboard_key_pressed")
        # Release 端口标记 Press 端口末尾。
        press_end = source.index("pub(crate) fn handle_keyboard_key_released", press_start)
        # 保存 Press 提交片段。
        press = source[press_start:press_end]
        # 先取得完整 owner 集合。
        owner_lock = press.index("match lock_press_owners")
        # 随后明确处理无窗口结果。
        target_check = press.index("let Some((window_id, mut owners))")
        # compositor repeat 去重必须在全部 guards 健康后判断。
        dedupe = press.index("if *owners.repeat_rate > 0 && owners.keys_down.contains(&code)")
        # 成功 Press 首先推进 input serial。
        serial_commit = press.index("owners.input_serial.record(serial)")
        # 随后登记物理按键。
        keys_commit = press.index("owners.keys_down.insert(code)")
        # KeyDown 是首个 UI 事件。
        key_event = press.index("UiEvent::key_down")
        # 可选 TextInput 紧随 KeyDown。
        text_event = press.index("UiEvent::text_input")
        # 随后登记客户端重复候选。
        held_commit = press.index("*owners.held_key = Some")
        # last-repeat 最后清空。
        last_commit = press.index("*owners.last_repeat = None")
        # 目标判定必须晚于完整 owner acquisition。
        self.assertLess(owner_lock, target_check)
        # 去重判定必须晚于目标判定。
        self.assertLess(target_check, dedupe)
        # 去重判定必须先于任何业务写入。
        self.assertLess(dedupe, serial_commit)
        # serial 先于物理按键集合推进。
        self.assertLess(serial_commit, keys_commit)
        # 物理按键集合先于 KeyDown 发布。
        self.assertLess(keys_commit, key_event)
        # KeyDown 必须先于 TextInput。
        self.assertLess(key_event, text_event)
        # 文本事件必须先于重复候选提交。
        self.assertLess(text_event, held_commit)
        # 重复候选必须先于节拍清空。
        self.assertLess(held_commit, last_commit)

    # 确认 Release 在 guard 健康后保持既有清理语义。
    def test_release_commits_key_up_and_repeat_cleanup_once(self) -> None:
        # 读取 owner Component。
        source = OWNER.read_text(encoding="utf-8")
        # 限定 Release 端口到文件末尾。
        release_start = source.index("pub(crate) fn handle_keyboard_key_released")
        # 保存 Release 提交片段。
        release = source[release_start:]
        # 先取得完整 owner 集合。
        owner_lock = release.index("match lock_release_owners")
        # 随后明确处理无窗口结果。
        target_check = release.index("let Some((window_id, mut owners))")
        # 成功 Release 首先移除物理按键。
        keys_commit = release.index("owners.keys_down.remove(&code)")
        # 随后发布 KeyUp。
        key_event = release.index("UiEvent::key_up")
        # 再清除客户端重复候选。
        held_commit = release.index("*owners.held_key = None")
        # last-repeat 最后清空。
        last_commit = release.index("*owners.last_repeat = None")
        # 目标判定必须晚于完整 owner acquisition。
        self.assertLess(owner_lock, target_check)
        # 目标判定必须先于任何业务写入。
        self.assertLess(target_check, keys_commit)
        # 物理按键移除先于 KeyUp 发布。
        self.assertLess(keys_commit, key_event)
        # KeyUp 必须先于候选清理。
        self.assertLess(key_event, held_commit)
        # 候选清理必须先于节拍清空。
        self.assertLess(held_commit, last_commit)

    # 确认 owner 只上报 typed failure 且文件规模满足项目门槛。
    def test_owner_failure_boundary_and_file_limits(self) -> None:
        # 读取 owner Component。
        owner_source = OWNER.read_text(encoding="utf-8")
        # 所有 owner failure 都必须稳定分类为 InvalidState。
        self.assertIn("Errc::InvalidState", owner_source)
        # failure 必须进入既有 pending source。
        self.assertIn("pending_failures.enqueue", owner_source)
        # Component 不得执行最终错误策略或应用逻辑。
        for forbidden in ["tracing::", ".report(", "attempt_recovery"]:
            # 每类越界副作用都必须缺席。
            self.assertNotIn(forbidden, owner_source)
        # 本任务涉及的三个 Rust 文件都必须低于 900 行。
        for path in [OWNER, SEAT, MODULE]:
            # 逐文件计算物理行数。
            line_count = len(path.read_text(encoding="utf-8").splitlines())
            # 报错时显示具体超限文件。
            self.assertLessEqual(line_count, 900, str(path))


# 允许直接运行本聚焦测试文件。
if __name__ == "__main__":
    # 使用标准 unittest runner 输出精确用例数。
    unittest.main()
