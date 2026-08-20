# 使用路径对象读取 Wayland cursor 与 App 交接契约。
from pathlib import Path
# 使用标准库测试框架保持聚焦验证无额外依赖。
import unittest

# 解析测试文件所在仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 Wayland cursor Adapter。
WAYLAND_CURSOR = ROOT / "src/native/backends/linux/windowing/wayland/cursor.rs"
# 定位 cursor intent Component。
CURSOR_STATE = ROOT / "src/native/backends/linux/windowing/wayland/cursor_state.rs"
# 定位 seat callback 与 pointer capability 编排。
WAYLAND_SEAT = ROOT / "src/native/backends/linux/windowing/wayland/seat.rs"
# 定位可选 cursor-shape global 的 composition root。
WAYLAND_BACKEND = ROOT / "src/native/backends/linux/windowing/wayland/mod.rs"
# 定位 App pointer-cursor 交接 Component。
APP_CURSOR = ROOT / "src/app/event_loop/pointer_cursor.rs"


# 验证 Wayland cursor 能力只在真实协议或持久 intent 已建立后成功。
class WaylandCursorCapabilityTruthTests(unittest.TestCase):
    # 确认可选 cursor-shape global 由 backend 真实绑定和持有。
    def test_backend_binds_optional_cursor_shape_manager(self) -> None:
        # 读取 Wayland composition root。
        source = WAYLAND_BACKEND.read_text(encoding="utf-8")
        # backend 必须持有唯一可选 manager owner。
        self.assertEqual(
            # 统计字段声明，避免第二份能力事实。
            source.count("cursor_shape_manager: Option<Main<WpCursorShapeManagerV1>>"),
            # 只允许一个 composition-root owner。
            1,
        )
        # 构造期必须按协议支持范围绑定 v1 到 v2。
        self.assertIn(".bind::<WpCursorShapeManagerV1, _, _>(&queue_handle, 1..=2, ())", source)
        # bind 失败保留 None，不得创建伪 manager。
        self.assertIn("let cursor_shape_manager = globals", source)
        # 构造结果必须发布同一可选 owner。
        self.assertIn("cursor_shape_manager,", source)

    # 确认 set/show 先验证真实能力，再提交协议或待 Enter intent。
    def test_set_and_show_use_protocol_before_committing_intent(self) -> None:
        # 读取 Wayland cursor Adapter。
        source = WAYLAND_CURSOR.read_text(encoding="utf-8")
        # 定位 set_cursor 入口。
        set_start = source.index("fn set_cursor")
        # show_cursor 标记 set 片段终点。
        show_start = source.index("fn show_cursor", set_start)
        # cursor_position 标记 show 片段终点。
        show_end = source.index("fn cursor_position", show_start)
        # 保存形状入口。
        set_cursor = source[set_start:show_start]
        # 保存可见性入口。
        show_cursor = source[show_start:show_end]
        # 形状入口必须先验证 optional global。
        self.assertIn('self.cursor_shape_manager("set_cursor")?', set_cursor)
        # 有焦点时必须提交真实 cursor-shape 请求。
        self.assertIn("submit_shape(manager, pointer, serial, shape)", set_cursor)
        # intent 只能在协议路径成功之后发布。
        self.assertLess(set_cursor.index("submit_shape"), set_cursor.index("commit_cursor(cursor)"))
        # 可见性入口必须对称验证 manager，防止只能隐藏不能恢复。
        self.assertIn('self.cursor_shape_manager("show_cursor")?', show_cursor)
        # 隐藏必须提交 wl_pointer 空 surface。
        self.assertIn("pointer.set_cursor(serial, None, 0, 0)", show_cursor)
        # 恢复必须重放当前 shape。
        self.assertIn("submit_shape(manager, pointer, serial, shape)", show_cursor)
        # 可见性 intent 只能在协议路径成功之后发布。
        self.assertLess(show_cursor.index("pointer.set_cursor"), show_cursor.index("commit_visibility"))
        # 两个入口都不得用日志替代协议动作。
        self.assertNotIn("tracing::", set_cursor + show_cursor)

    # 确认 Enter 重放和所有失焦边沿撤销同一 serial。
    def test_seat_routes_cursor_enter_leave_and_pointer_release(self) -> None:
        # 读取 seat callback 编排。
        source = WAYLAND_SEAT.read_text(encoding="utf-8")
        # Enter 必须显式捕获协议 serial。
        enter_start = source.index("wl_pointer::Event::Enter")
        # Motion 标记 Enter 分支终点。
        enter_end = source.index("wl_pointer::Event::Motion", enter_start)
        # 保存 Enter 编排片段。
        enter = source[enter_start:enter_end]
        # callback 必须把 serial 交给 cursor Component。
        self.assertIn("apply_cursor_on_pointer_enter", enter)
        # cursor 重放使用事件自身 pointer 代理。
        self.assertIn("pointer,", enter)
        # Leave 必须撤销 cursor focus serial。
        leave_start = source.index("wl_pointer::Event::Leave")
        # Button 标记 Leave 分支终点。
        leave_end = source.index("wl_pointer::Event::Button", leave_start)
        # 保存 Leave 编排片段。
        leave = source[leave_start:leave_end]
        # 协议 Leave 后必须调用统一失效端口。
        self.assertIn("clear_cursor_pointer_focus", leave)
        # capability Release 成功后也必须失效旧代理 serial。
        release_start = source.index("pointer_transition == InputProxyTransition::Release")
        # Keyboard Bind 标记 pointer Release 片段终点。
        release_end = source.index("keyboard_transition == InputProxyTransition::Bind", release_start)
        # 保存 capability teardown 片段。
        release = source[release_start:release_end]
        # serial 清理必须位于 checked 代理释放之后。
        self.assertLess(
            # 定位 checked 释放调用。
            release.index("release_pointer_proxy_checked"),
            # 定位 cursor serial 失效。
            release.index("clear_cursor_pointer_focus"),
        )
        # backend shutdown 同样必须清理原子 serial。
        shutdown_start = source.index("pub(crate) fn shutdown_seat_and_input")
        # ensure_seat 标记 shutdown 片段终点。
        shutdown_end = source.index("pub(crate) fn ensure_seat_and_input", shutdown_start)
        # 保存无返回 teardown 片段。
        shutdown = source[shutdown_start:shutdown_end]
        # shutdown 不得跨 backend 生命周期保留 Enter 授权。
        self.assertIn("self.cursor_state.clear_enter()", shutdown)

    # 确认 cursor Component 不用可中毒锁保存协议授权。
    def test_cursor_state_uses_atomic_intent_and_reserved_serial_encoding(self) -> None:
        # 读取 cursor-state Component。
        source = CURSOR_STATE.read_text(encoding="utf-8")
        # 三份事实必须由明确原子类型持有。
        self.assertIn("enter_serial: AtomicU64", source)
        # 形状 intent 使用私有整数编码。
        self.assertIn("cursor: AtomicU32", source)
        # 可见性使用原子布尔值。
        self.assertIn("visible: AtomicBool", source)
        # 零值必须只表示无 Enter serial。
        self.assertIn("const NO_ENTER_SERIAL: u64 = 0", source)
        # 真实 u32 serial 以加一方式无损编码。
        self.assertIn(".store(u64::from(serial) + 1", source)
        # Component 不得引入 poisoned Mutex 恢复路径。
        self.assertNotIn("Mutex", source)
        # Leave/capability/shutdown 共用幂等清零端口。
        self.assertIn("pub(crate) fn clear_enter", source)

    # 确认 App 只对稳定能力缺失去重。
    def test_app_deduplicates_not_implemented_but_retries_other_errors(self) -> None:
        # 读取 App cursor 交接 Component。
        source = APP_CURSOR.read_text(encoding="utf-8")
        # 定位窄端口交接函数。
        start = source.index("fn apply_cursor_port")
        # 测试模块标记实现片段终点。
        end = source.index("#[cfg(test)]", start)
        # 保存交接实现。
        apply = source[start:end]
        # NotImplemented 必须有独立 guard。
        self.assertIn("error.code() == Errc::NotImplemented", apply)
        # capability absence 分支必须更新 active 以去重。
        capability_start = apply.index("Err(error) if error.code()")
        # 普通失败分支标记 capability 片段终点。
        retry_start = apply.index("Err(error) =>", capability_start)
        # 保存 capability absence 分支。
        capability = apply[capability_start:retry_start]
        # 稳定缺失必须缓存请求值。
        self.assertIn("active.set(Some(requested))", capability)
        # 稳定缺失不得产生 warn。
        self.assertNotIn("tracing::warn!", capability)
        # 普通失败仍必须产生可观察 warning。
        self.assertIn("tracing::warn!", apply[retry_start:])
        # 普通失败分支不得更新 active。
        self.assertNotIn("active.set", apply[retry_start:])

    # 确认本任务涉及文件满足项目规模门槛。
    def test_touched_files_stay_within_limit(self) -> None:
        # 逐一检查本任务修改的源码与测试文件。
        for path in (
            # Wayland 协议 Adapter。
            WAYLAND_CURSOR,
            # cursor intent Component。
            CURSOR_STATE,
            # seat 生命周期编排。
            WAYLAND_SEAT,
            # backend composition root。
            WAYLAND_BACKEND,
            # App 能力交接。
            APP_CURSOR,
            # 本聚焦契约测试。
            Path(__file__),
        ):
            # 计算当前文件物理行数。
            line_count = len(path.read_text(encoding="utf-8").splitlines())
            # 文件必须保持不超过 900 行。
            self.assertLessEqual(line_count, 900, path)


# 允许直接运行本聚焦测试文件。
if __name__ == "__main__":
    # 使用标准 unittest runner 输出精确用例数。
    unittest.main()
