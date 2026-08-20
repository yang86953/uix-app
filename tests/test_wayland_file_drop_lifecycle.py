# 使用路径对象读取 Wayland 文件拖放的协议与窗口契约。
from pathlib import Path
# 标准库测试框架保持聚焦验证无额外依赖。
import unittest

# 解析测试文件所在仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 文件拖放 Component 拥有 offer 与 URI read 生命周期。
FILE_DROP = ROOT / "src/native/backends/linux/windowing/wayland/file_drop.rs"
# URI Component 隔离本地 file URI 解析。
FILE_DROP_URI = ROOT / "src/native/backends/linux/windowing/wayland/file_drop_uri.rs"
# 窗口 Adapter 校验能力与逐窗开关。
FILE_DROP_WINDOW = ROOT / "src/native/backends/linux/windowing/wayland/file_drop_window.rs"
# composition root 持有唯一共享 Component。
WAYLAND_BACKEND = ROOT / "src/native/backends/linux/windowing/wayland/mod.rs"
# seat Adapter 分类 DataOffer、Selection 与 DnD 事件。
WAYLAND_SEAT = ROOT / "src/native/backends/linux/windowing/wayland/seat.rs"
# owner-thread event loop 轮询 URI pipe。
WAYLAND_EVENT_LOOP = ROOT / "src/native/backends/linux/windowing/wayland/event_loop.rs"
# 窗口操作入口暴露真实 enable_file_drop 能力。
WAYLAND_WINDOW_OPS = ROOT / "src/native/backends/linux/windowing/wayland/window_ops.rs"
# 窗口工厂注入共享状态与协议可用性。
WAYLAND_WINDOW = ROOT / "src/native/backends/linux/windowing/wayland/window.rs"
# Drop Adapter 负责逐窗最终清理。
SURFACE_REGISTRATION = ROOT / "src/native/backends/linux/windowing/wayland/surface_registration.rs"
# 验证 Wayland FileDrop 只有在真实协议、窗口资格与完成传输成立后产生事件。
class WaylandFileDropLifecycleTests(unittest.TestCase):
    # composition root 必须只有一个共享状态 owner，并传给每个窗口。
    def test_backend_owns_and_injects_one_file_drop_component(self) -> None:
        # 读取 backend composition root。
        backend = WAYLAND_BACKEND.read_text(encoding="utf-8")
        # 读取窗口工厂接线。
        window = WAYLAND_WINDOW.read_text(encoding="utf-8")
        # 字段声明只允许一份 backend owner。
        self.assertEqual(
            # 统计明确类型字段。
            backend.count("file_drop_state: Arc<Mutex<file_drop::WaylandFileDropState>>"),
            # 单一事实 owner。
            1,
        )
        # 构造期必须初始化空 Component。
        self.assertIn("WaylandFileDropState::default()", backend)
        # 窗口接收同一 Arc，不得创建局部状态副本。
        self.assertIn("self.file_drop_state.clone()", window)
        # 协议能力事实来自 seat 已建立的真实 data-device owner。
        self.assertIn("self.data_device.is_some()", window)

    # data-device callback 必须先登记 offer，再按 Selection 与 DnD 分类。
    def test_seat_routes_every_data_device_lifecycle_edge(self) -> None:
        # 读取 seat callback 编排。
        seat = WAYLAND_SEAT.read_text(encoding="utf-8")
        # data-device 分区限定本测试观察范围。
        start = seat.index("// ── 剪贴板数据设备")
        # pointer 分区标记 callback 末尾。
        end = seat.index("// ── 指针 + 键盘", start)
        # 保存完整数据设备 Adapter。
        callback = seat[start:end]
        # DataOffer 必须建立 MIME/Action callback owner。
        self.assertIn("register_data_offer", callback)
        # Selection 必须先从 DnD owner 脱离。
        detach = callback.index("detach_selection_offer")
        # clipboard receive 只能在 detach 之后执行。
        selection = callback.index("handle_selection_event", detach)
        # owner 分类顺序必须确定。
        self.assertLess(detach, selection)
        # 其余 Enter/Motion/Leave/Drop 统一进入 DnD Component。
        self.assertIn("handle_data_device_event", callback)
        # seat Adapter 本身不得锁业务状态。
        self.assertNotIn(".lock()", callback)

    # Enter 只有对已启用本地窗口与 URI MIME 才接受 Copy。
    def test_enter_negotiates_uri_copy_only_for_enabled_window(self) -> None:
        # 读取协议/状态 Component。
        source = FILE_DROP.read_text(encoding="utf-8")
        # 限定 Enter 状态方法。
        start = source.index("fn enter(")
        # Motion 标记 Enter 末尾。
        end = source.index("fn motion", start)
        # 保存协商实现。
        enter = source[start:end]
        # 逐窗开关必须参与接受条件。
        self.assertIn("self.enabled_windows.contains", enter)
        # MIME 必须精确匹配 text/uri-list。
        self.assertIn("record.mime_types.contains(URI_LIST_MIME)", enter)
        # v3+ 只能声明 Copy 动作。
        self.assertIn("DndAction::Copy", enter)
        # 拒绝必须使用空动作集合。
        self.assertIn("DndAction::empty()", enter)
        # accept 必须使用本次 Enter serial。
        self.assertIn("record.offer.accept(", enter)
        self.assertIn("serial,", enter)

    # Drop 必须非阻塞接收、定向投递并在成功读取后 finish。
    def test_drop_reads_uri_pipe_before_finishing_and_routes_window(self) -> None:
        # 读取文件拖放 Component。
        source = FILE_DROP.read_text(encoding="utf-8")
        # Drop 建立 receive/read owner。
        drop_start = source.index("fn drop_performed")
        # Selection helper 标记 Drop 末尾。
        drop_end = source.index("fn detach_selection_offer", drop_start)
        # 保存 Drop 协议阶段。
        drop = source[drop_start:drop_end]
        # receive 必须请求已接受的 URI MIME。
        self.assertIn(".receive(URI_LIST_MIME.to_string()", drop)
        # read owner 必须进入 event-loop 队列。
        self.assertIn("self.reads.push(read)", drop)
        # Drop 后立即断开 offer callback 强引用环。
        self.assertIn("record.offer.clear_callback()", drop)
        # completion 必须构造定向 FileDrop 事件。
        completion_start = source.index("pub(crate) fn complete_polled_read")
        # outcome 类型标记 completion 末尾。
        completion_end = source.index("enum ReadOutcome", completion_start)
        # 保存 completion 实现。
        completion = source[completion_start:completion_end]
        # URI 解析发生在事件构造前。
        parse = completion.index("parse_uri_list")
        # 构造业务事件。
        event = completion.index("UiEvent::file_drop", parse)
        # 定向稳定 WindowId。
        route = completion.index(".for_window(read.window_id)", event)
        # 成功 finish 必须晚于事件可交付检查。
        finish = completion.index("finish_read(read)", route)
        # 验证严格阶段顺序。
        self.assertLess(parse, event)
        self.assertLess(event, route)
        self.assertLess(route, finish)

    # event loop 必须真实 poll 所有 URI read FD 并处理错误位。
    def test_event_loop_polls_file_drop_read_owners(self) -> None:
        # 读取 owner-thread poll 编排。
        event_loop = WAYLAND_EVENT_LOOP.read_text(encoding="utf-8")
        # 限定 poll 方法。
        start = event_loop.index("fn dispatch_polled")
        # 按键重复 helper 标记方法末尾。
        end = event_loop.index("fn generate_key_repeats", start)
        # 保存 poll 全流程。
        dispatch = event_loop[start:end]
        # poll 前必须取得健康 FD 快照。
        snapshot = dispatch.index("super::file_drop::read_fds")
        # URI FD 必须加入 poll 数组。
        append = dispatch.index("file_drop_fds.iter()", snapshot)
        # 系统 poll 必须晚于 FD 附加。
        poll_call = dispatch.index("let ret = unsafe", append)
        # completion 必须晚于 poll 返回。
        completion = dispatch.index("complete_polled_read", poll_call)
        # 验证快照、poll、完成的单向链。
        self.assertLess(snapshot, append)
        self.assertLess(append, poll_call)
        self.assertLess(poll_call, completion)

    # 禁用、关闭与 backend teardown 必须共同释放逐窗/全局 owners。
    def test_window_and_backend_teardown_clear_file_drop_owners(self) -> None:
        # 读取窗口能力 Adapter。
        window_ops = WAYLAND_WINDOW_OPS.read_text(encoding="utf-8")
        # 读取窗口最终 Drop Adapter。
        registration = SURFACE_REGISTRATION.read_text(encoding="utf-8")
        # 读取 backend teardown。
        backend = WAYLAND_BACKEND.read_text(encoding="utf-8")
        # 公共入口必须委托检查式能力 Adapter。
        self.assertIn("file_drop_window::set_window_capability", window_ops)
        # 显式关闭必须检查式禁用当前窗口。
        self.assertIn("file_drop_window::disable_window", window_ops)
        # Rust Drop 必须委托独立 teardown Adapter 完成最终释放。
        self.assertIn("file_drop_window::force_disable_window", registration)
        # backend 在 data-device callback 注销前关闭所有 offer/read owners。
        file_drop_shutdown = backend.index("self.shutdown_file_drop()")
        # seat shutdown 随后注销 data-device callback。
        seat_shutdown = backend.index("self.shutdown_seat_and_input()", file_drop_shutdown)
        # teardown 顺序必须先状态后 callback。
        self.assertLess(file_drop_shutdown, seat_shutdown)

    # 本任务涉及文件必须保持项目 900 行门槛。
    def test_touched_files_stay_within_limit(self) -> None:
        # 逐一检查协议、Adapter、composition 与聚焦测试文件。
        for path in (
            # 协议/FD 状态 Component。
            FILE_DROP,
            # URI 解析 Component。
            FILE_DROP_URI,
            # 逐窗能力 Adapter。
            FILE_DROP_WINDOW,
            # backend composition root。
            WAYLAND_BACKEND,
            # data-device Adapter。
            WAYLAND_SEAT,
            # owner-thread poll 编排。
            WAYLAND_EVENT_LOOP,
            # 窗口操作入口。
            WAYLAND_WINDOW_OPS,
            # 窗口工厂。
            WAYLAND_WINDOW,
            # surface Drop Adapter。
            SURFACE_REGISTRATION,
            # 本聚焦契约测试。
            Path(__file__),
        ):
            # 计算当前物理行数。
            line_count = len(path.read_text(encoding="utf-8").splitlines())
            # 所有文件都不得超过 900 行。
            self.assertLessEqual(line_count, 900, path)


# 允许直接运行本聚焦测试文件。
if __name__ == "__main__":
    # 使用标准 unittest runner 输出精确用例数。
    unittest.main()
