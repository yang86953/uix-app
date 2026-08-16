# -*- coding: utf-8 -*-
"""Keep Wayland dispatch panics inside the owner-thread failure boundary."""
from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
EVENT_LOOP = ROOT / "src/native/backends/linux/wayland/event_loop.rs"
COMPAT = ROOT / "src/native/backends/linux/wayland/compat.rs"
# 读取 wl_output callback 与显示状态 owner 的接线。
WAYLAND_BACKEND = ROOT / "src/native/backends/linux/wayland/mod.rs"
# 读取单窗口 callback adapter 的 backend source 注入点。
WAYLAND_WINDOW_FACTORY = ROOT / "src/native/backends/linux/wayland/window.rs"
# 读取 Wayland clipboard data_source callback。
WAYLAND_CLIPBOARD = ROOT / "src/native/backends/linux/wayland/clipboard.rs"
# 读取 Wayland frame callback 私有 Component。
WAYLAND_FRAME_CALLBACK = ROOT / "src/native/backends/linux/wayland/frame_callback.rs"
# 读取 Wayland surface 跨注册表注销 Component。
WAYLAND_SURFACE_REGISTRATION = ROOT / "src/native/backends/linux/wayland/surface_registration.rs"
# 读取 Wayland 指针激活授权 Component。
WAYLAND_POINTER_ACTIVATION = ROOT / "src/native/backends/linux/wayland/pointer_activation.rs"
# 读取 Linux 平台的 owner-thread 失败提取边界。
PLATFORM = ROOT / "src/native/backends/linux/platform.rs"
# 定位 Wayland 窗口操作与装饰模式实现。
WINDOW_OPS = ROOT / "src/native/backends/linux/wayland/window_ops.rs"
# 定位实际启用自定义标题栏的主演示入口（uix-lang-demo）。
GUI_DEMO = ROOT / "demo/uix-lang-demo/src/main.rs"


class WaylandCallbackGuardTests(unittest.TestCase):
    def test_dispatch_pending_catches_callback_panics(self) -> None:
        source = EVENT_LOOP.read_text(encoding="utf-8")
        self.assertIn("catch_unwind", source)
        self.assertIn("event_queue.dispatch_pending", source)
        self.assertIn("callback panicked during dispatch", source)
        self.assertIn("Errc::PlatformError", source)

    def test_unknown_created_child_remains_an_explicit_failure_case(self) -> None:
        source = COMPAT.read_text(encoding="utf-8")
        self.assertIn("fn event_created_child", source)
        self.assertIn("missing event-created child dispatch", source)

    # 确认 callback registry 状态损坏通过检查式 Component 进入 owner source。
    def test_callback_registry_state_failures_are_checked(self) -> None:
        # 读取 Wayland 0.31 callback compatibility adapter。
        source = COMPAT.read_text(encoding="utf-8")
        # registry Component 必须成为 callback map 的唯一 owner。
        self.assertIn("struct CallbackRegistry", source)
        # callback map 锁中毒不得继续通过 PoisonError 恢复访问。
        self.assertNotIn("unwrap_or_else(|error| error.into_inner())", source)
        # register/unregister/take/reinsert 必须共享稳定的锁中毒诊断。
        self.assertIn("Wayland callback registry mutex poisoned during {operation}", source)
        # 擦除 callback 类型错配必须形成稳定 typed failure。
        self.assertIn("callback registry type mismatch", source)
        # dispatch 必须使用检查式 downcast，而不是静默 if-let 丢弃。
        self.assertIn("downcast_callback::<I>", source)
        # 限定 WaylandDispatchState 的 callback 执行片段。
        dispatch_start = source.index("fn dispatch<I>")
        # Dispatch trait 实现标记 dispatch helper 的末尾。
        dispatch_end = source.index("impl<I> Dispatch", dispatch_start)
        # 保存 callback 执行与回插代码。
        dispatch = source[dispatch_start:dispatch_end]
        # callback panic 必须仍由 catch_unwind 隔离。
        catch_index = dispatch.index("catch_unwind")
        # panic 必须转换为 owner-thread failure。
        panic_index = dispatch.index("report_callback_panic::<I>")
        # 健康 registry 必须在 panic 转换之后仍回插 callback owner。
        reinsert_index = dispatch.index('"callback reinsert"')
        # panic 转换只能发生在 catch_unwind 之后。
        self.assertLess(catch_index, panic_index)
        # 回插必须覆盖 callback 成功与 panic 两种结果。
        self.assertLess(panic_index, reinsert_index)

    # 确认 wl_output callback 不会恢复并继续写入中毒显示状态。
    def test_output_callback_reports_poisoned_display_state(self) -> None:
        # 读取 Wayland backend 构造期的 output callback。
        source = WAYLAND_BACKEND.read_text(encoding="utf-8")
        # output 枚举循环标记 callback 片段起点。
        output_start = source.index("for (index, global)")
        # wm_base ping callback 标记 output callback 片段终点。
        output_end = source.index("_wm_base.quick_assign", output_start)
        # 保存单一显示状态 callback 片段。
        output_callback = source[output_start:output_end]
        # callback 必须复用 backend 已有 pending failure source。
        self.assertIn("pending_failures.clone()", output_callback)
        # 锁中毒不得继续通过 into_inner 访问显示状态。
        self.assertNotIn("into_inner()", output_callback)
        # 锁失败必须稳定分类为 InvalidState。
        self.assertIn("Errc::InvalidState", output_callback)
        # 诊断必须保留 wl_output 与 outputs mutex 阶段。
        self.assertIn("Wayland wl_output callback outputs mutex poisoned", output_callback)
        # 健康路径必须继续保留四类协议事实。
        for event_name in ["Geometry", "Mode", "Scale", "Done"]:
            # 每类 wl_output 事件都必须仍由同一 callback 处理。
            self.assertIn(f"Event::{event_name}", output_callback)

    # 确认 xdg_toplevel Configure 在全部 owner 可用后才提交同源状态。
    def test_toplevel_callback_commits_window_state_transactionally(self) -> None:
        # 读取单窗口协议 adapter。
        window_ops = WINDOW_OPS.read_text(encoding="utf-8")
        # 读取 Wayland 窗口工厂。
        window_factory = WAYLAND_WINDOW_FACTORY.read_text(encoding="utf-8")
        # 窗口 callback 必须持有 backend 注入的 source。
        self.assertIn("pending_failures: PendingFailureSource", window_ops)
        # 工厂必须复用 backend source，禁止创建空 failure queue。
        self.assertIn("self.pending_failures.clone()", window_factory)
        # toplevel callback 接线起点。
        callback_start = window_ops.index("let tl_events = events.clone()")
        # 装饰初始化标记 callback 片段终点。
        callback_end = window_ops.index("// 窗口装饰", callback_start)
        # 保存 Close/Configure callback 片段。
        callback = window_ops[callback_start:callback_end]
        # callback 内不得通过 into_inner 恢复损坏 owner。
        self.assertNotIn("into_inner()", callback)
        # Close 队列失败必须有稳定诊断。
        self.assertIn("Wayland xdg_toplevel Close event queue mutex poisoned", callback)
        # Configure 三个 owner 失败必须各自可定位。
        self.assertIn("Configure mode state mutex poisoned", callback)
        # WindowState 借用必须使用非 panic 路径。
        self.assertIn("window_state.try_borrow_mut()", callback)
        # Configure 队列失败必须有稳定诊断。
        self.assertIn("Configure event queue mutex poisoned", callback)
        # Configure 分支标记事务代码起点。
        configure_start = callback.index("xdg_toplevel::Event::Configure")
        # 保存 Configure 事务片段。
        configure = callback[configure_start:]
        # 先取得模式 owner。
        mode_lock = configure.index("configured_modes.lock()")
        # 再检查 WindowState 写借用。
        state_borrow = configure.index("window_state.try_borrow_mut()")
        # 最后取得事件队列 owner。
        queue_lock = configure.index("tl_events.lock()")
        # 全部 owner 可用后才允许改写模式。
        apply_modes = configure.index("modes.apply_configure")
        # 锁与借用检查必须保持事务前置顺序。
        self.assertLess(mode_lock, state_borrow)
        # WindowState 检查必须先于事件队列检查。
        self.assertLess(state_borrow, queue_lock)
        # 任何模式改写必须发生在全部 owner 检查之后。
        self.assertLess(queue_lock, apply_modes)
        # 健康路径必须继续生成 resize/maximize/restore 三类事件。
        for event_name in ["UiEvent::resize", "WindowMaximize", "WindowRestore"]:
            # 每类既有窗口事实都必须保留。
            self.assertIn(event_name, configure)

    # 确认 clipboard Send callback 不会把 FD owner 交给中毒写队列。
    def test_clipboard_send_callback_reports_poisoned_write_queue(self) -> None:
        # 读取 Wayland clipboard callback adapter。
        source = WAYLAND_CLIPBOARD.read_text(encoding="utf-8")
        # data source callback 标记 Send 处理片段起点。
        callback_start = source.index("source.quick_assign")
        # selection 请求标记 callback 片段终点。
        callback_end = source.index("dd.set_selection", callback_start)
        # 保存 Send callback 片段。
        callback = source[callback_start:callback_end]
        # callback 必须先建立独占协议 FD 的 ClipboardWrite。
        create_write = callback.index("ClipboardWrite::from_event_fd")
        # 健康队列才允许接管 write owner。
        lock_queue = callback.index("writes.lock()")
        # write owner 只能在检查队列之后入队。
        push_write = callback.index("queued.push(write)")
        # FD owner 必须先建立，再检查唯一队列 owner。
        self.assertLess(create_write, lock_queue)
        # 队列检查必须先于所有权转移。
        self.assertLess(lock_queue, push_write)
        # 锁中毒不得继续通过 into_inner 访问发送队列。
        self.assertNotIn("into_inner()", callback)
        # 锁失败必须稳定分类为 InvalidState。
        self.assertIn("Errc::InvalidState", callback)
        # 诊断必须保留 clipboard Send 写队列阶段。
        self.assertIn("Wayland clipboard Send write queue mutex poisoned", callback)
        # 既有 FD 初始化失败必须继续分类为 IoError。
        self.assertIn("Errc::IoError", callback)

    # 确认 compositor Done 检查式消费 one-shot request 并投递逐窗事件。
    def test_frame_callback_consumes_request_without_ghost_state(self) -> None:
        # 读取 frame callback Component。
        frame_callback = WAYLAND_FRAME_CALLBACK.read_text(encoding="utf-8")
        # 读取 WindowOps 的请求登记、callback 接线与取消路径。
        window_ops = WINDOW_OPS.read_text(encoding="utf-8")
        # Component 必须检查 active request owner。
        self.assertIn("Wayland frame callback request mutex poisoned", frame_callback)
        # Component 必须检查窗口事件队列 owner。
        self.assertIn("Wayland frame callback event queue mutex poisoned", frame_callback)
        # 已完成 request 必须在尝试事件投递前清除。
        clear_request = frame_callback.index("*active = None")
        # 事件队列锁标记可能失败的投递边界。
        lock_events = frame_callback.index("events.lock()")
        # callback 已到达后不得因队列失败留下幽灵 active request。
        self.assertLess(clear_request, lock_events)
        # 健康路径必须保留 token 与 callback 单调时间。
        self.assertIn("UiEvent::frame_opportunity(request.token, callback_at, None)", frame_callback)
        # 健康路径必须附加稳定 WindowId。
        self.assertIn(".for_window(window_id)", frame_callback)
        # WindowOps 必须委托私有 Component。
        self.assertIn("deliver_frame_opportunity(", window_ops)
        # callback typed failure 必须进入 backend source。
        self.assertIn("frame_failures.enqueue(error)", window_ops)
        # request 登记锁失败必须同步返回 typed error。
        self.assertIn("frame request mutex poisoned during registration", window_ops)
        # request 取消锁失败必须同步返回 typed error。
        self.assertIn("frame request mutex poisoned during cancellation", window_ops)
        # 限定原生 frame request/cancel 实现片段。
        frame_start = window_ops.index("fn os_request_native_frame")
        # 下一个窗口状态分区标记 frame 片段终点。
        frame_end = window_ops.index("// ── 窗口状态", frame_start)
        # 保存 frame pacing 实现。
        frame_ops = window_ops[frame_start:frame_end]
        # frame request、callback 与 cancel 均不得恢复 poisoned mutex。
        self.assertNotIn("into_inner()", frame_ops)

    # 确认窗口关闭只在两份健康注册表上提交 surface 注销事务。
    def test_surface_unregistration_preserves_retryable_identity(self) -> None:
        # 读取跨注册表注销 Component。
        registration = WAYLAND_SURFACE_REGISTRATION.read_text(encoding="utf-8")
        # 读取 WindowOps 的显式关闭与 Drop 编排。
        window_ops = WINDOW_OPS.read_text(encoding="utf-8")
        # Component 不得恢复任一 poisoned registry。
        self.assertNotIn("into_inner()", registration)
        # 两份 owner 损坏必须分别产生稳定诊断。
        self.assertIn("pointer activation registry mutex poisoned", registration)
        # surface 路由 owner 损坏也必须可定位。
        self.assertIn("surface window registry mutex poisoned", registration)
        # 记录第一份注册表 guard 的取得位置。
        pointer_lock = registration.index("pointer_activations.lock()")
        # 记录第二份注册表 guard 的取得位置。
        surface_lock = registration.index("surface_windows.lock()")
        # 记录授权注销提交位置。
        unregister_pointer = registration.index("pointer_registry.unregister_surface")
        # 记录路由注销提交位置。
        unregister_surface = registration.index("surface_registry.unregister_surface")
        # 第二份 guard 必须在第一项注销前取得。
        self.assertLess(pointer_lock, surface_lock)
        # 任一共享事实都不得在两份 guard 齐备前修改。
        self.assertLess(surface_lock, unregister_pointer)
        # 两项注销保持既定的授权后路由顺序。
        self.assertLess(unregister_pointer, unregister_surface)
        # 限定窗口私有注销方法。
        method_start = window_ops.index("fn unregister_surface")
        # 以窗口初始化标记方法末尾。
        method_end = window_ops.index("pub(crate) fn init", method_start)
        # 保存身份消费编排片段。
        method = window_ops[method_start:method_end]
        # Component 必须先完成跨注册表事务。
        component_call = method.index("unregister_window_surface(")
        # surface identity 只能在成功返回后清除。
        clear_identity = method.index("self.surface_id = None")
        # 失败路径必须保留 identity 供显式重试。
        self.assertLess(component_call, clear_identity)
        # 限定注册事务 Component 旁的 Drop adapter。
        drop_start = registration.index("impl Drop for WaylandWindowOps")
        # 保存 Drop 失败转交通道。
        drop_source = registration[drop_start:]
        # Drop 必须观察注销失败。
        self.assertIn("if let Err(error) = self.unregister_surface()", drop_source)
        # 无同步接收方时必须复用 backend pending source。
        self.assertIn("self.pending_failures.enqueue(error)", drop_source)
        # 限定显式关闭实现。
        close_start = window_ops.index("fn os_close")
        # 主动关闭方法标记显式 teardown 片段末尾。
        close_end = window_ops.index("fn os_request_close", close_start)
        # 保存显式关闭事务。
        close_source = window_ops[close_start:close_end]
        # frame request 锁中毒必须同步传播 typed failure。
        self.assertIn("frame request mutex poisoned during window close", close_source)
        # 显式关闭不得恢复损坏的 request owner。
        self.assertNotIn("into_inner()", close_source)
        # 注册表失败必须通过问号传播。
        self.assertIn("self.unregister_surface()?", close_source)
        # 注册表注销必须先于任何协议对象释放。
        self.assertLess(
            # 获取 checked 注销位置。
            close_source.index("self.unregister_surface()?"),
            # 获取第一个协议对象释放位置。
            close_source.index("self.xdg_decoration = None"),
        )

    # 确认窗口初始化只在两份健康注册表上发布 surface owner。
    def test_surface_registration_precedes_protocol_owner_publication(self) -> None:
        # 读取跨注册表登记 Component。
        registration = WAYLAND_SURFACE_REGISTRATION.read_text(encoding="utf-8")
        # 读取 WindowOps 初始化编排。
        window_ops = WINDOW_OPS.read_text(encoding="utf-8")
        # 登记函数必须复用与注销相同的双 guard helper。
        register_start = registration.index("fn register_window_surface")
        # 注销函数标记登记片段末尾。
        register_end = registration.index("fn unregister_window_surface", register_start)
        # 保存登记事务实现。
        register_source = registration[register_start:register_end]
        # 登记必须先取得两份健康 guard。
        lock_registries = register_source.index("lock_surface_registries(")
        # 授权代次是第一份提交事实。
        register_pointer = register_source.index("pointer_registry.register_surface")
        # 窗口路由是第二份提交事实。
        register_surface = register_source.index("surface_registry.register_surface")
        # 两份 guard 必须先于任何注册表改写。
        self.assertLess(lock_registries, register_pointer)
        # 两项登记保持统一的授权后路由顺序。
        self.assertLess(register_pointer, register_surface)
        # 共享 helper 不得恢复 poisoned mutex。
        self.assertNotIn("into_inner()", registration)
        # helper 必须提供稳定登记阶段。
        self.assertIn('"surface registration"', register_source)
        # 限定窗口初始化实现。
        init_start = window_ops.index("pub(crate) fn init")
        # WindowOps trait 实现标记初始化片段末尾。
        init_end = window_ops.index("impl WindowOps for WaylandWindowOps", init_start)
        # 保存初始化协议 owner 编排。
        init_source = window_ops[init_start:init_end]
        # 初始化必须委托事务化登记 Component。
        register_call = init_source.index("register_window_surface(")
        # surface identity 只能在登记成功后发布。
        publish_identity = init_source.index("self.surface_id = Some(surface_id)")
        # 装饰 owner 只能在登记成功后发布。
        publish_decoration = init_source.index("self.xdg_decoration = xdg_decoration")
        # 输入区域 owner 只能在登记成功后发布。
        publish_region = init_source.index("self.input_region = Some(input_region)")
        # 协议 commit 只能在注册事实与本地 owner 就绪后执行。
        commit_surface = init_source.index("surface.commit()")
        # 双注册表事务必须先于 surface identity。
        self.assertLess(register_call, publish_identity)
        # identity 必须先于装饰 owner 发布。
        self.assertLess(publish_identity, publish_decoration)
        # 装饰与输入区域都在 commit 前归属窗口 owner。
        self.assertLess(publish_decoration, publish_region)
        # 输入区域发布必须先于 surface commit。
        self.assertLess(publish_region, commit_surface)
        # 登记前的装饰对象必须由局部 RAII owner 持有。
        self.assertIn("let xdg_decoration = if let Ok(dm)", init_source)
        # 登记前的输入区域也必须保持局部 owner。
        self.assertIn("let input_region = {", init_source)
        # 初始化注册表路径不得继续恢复 poisoned mutex。
        pre_publish = init_source[:publish_identity]
        # Component 调用前后都不允许直接恢复损坏注册表。
        self.assertNotIn("into_inner()", pre_publish)

    # 确认 Wayland dispatch 与关闭失败最终都进入 owner-thread 队列。
    def test_dispatch_failures_reach_owner_pending_source(self) -> None:
        # 读取兼容层的 callback source 持有与入队路径。
        compat = COMPAT.read_text(encoding="utf-8")
        # 读取事件循环的 panic/IO 失败收口路径。
        event_loop = EVENT_LOOP.read_text(encoding="utf-8")
        # 读取 Linux 平台向应用暴露的失败提取路径。
        platform = PLATFORM.read_text(encoding="utf-8")
        # 兼容层必须持有独立的 pending source。
        self.assertIn("pending_failures: PendingFailureSource", compat)
        # callback panic 必须把 typed failure 放入该 source。
        self.assertIn("self.pending_failures.enqueue", compat)
        # 事件循环必须用 checked dispatch 捕获异常并走关闭收口。
        self.assertIn("fn dispatch_pending_checked", event_loop)
        # 所有关闭失败必须通过 close_after_failure 入队。
        self.assertIn("close_after_failure", event_loop)
        # owner-thread 必须从同一 backend 提取 pending failure。
        self.assertIn("fn take_pending_failure", platform)
        # Linux 平台不得绕过 Wayland backend 的 source。
        self.assertIn("self.backend.take_pending_failure()", platform)

    # 确认 Wayland 自定义标题栏由客户端装饰模式承接且演示不再按平台禁用。
    def test_custom_title_bar_uses_wayland_client_side_decoration(self) -> None:
        # 读取 Wayland 窗口操作实现。
        window_ops = WINDOW_OPS.read_text(encoding="utf-8")
        # 读取主演示的窗口配置。
        gui_demo = GUI_DEMO.read_text(encoding="utf-8")
        # Wayland 后端必须实现统一的系统标题栏可见性能力。
        self.assertIn("fn os_set_system_title_bar_visible", window_ops)
        # 隐藏系统标题栏必须请求客户端装饰。
        self.assertIn("XdgDecoMode::ClientSide", window_ops)
        # 恢复系统标题栏必须请求服务端装饰。
        self.assertIn("XdgDecoMode::ServerSide", window_ops)
        # 关闭窗口时必须先释放依赖顶层窗口的装饰对象。
        close_start = window_ops.index("fn os_close")
        # 以紧邻的主动关闭方法限定显式 teardown 片段。
        close_end = window_ops.index("fn os_request_close", close_start)
        # 保存关闭实现，避免其他生命周期赋值影响断言。
        close_source = window_ops[close_start:close_end]
        # 装饰对象必须在顶层窗口之前释放。
        self.assertLess(
            # 获取装饰释放语句位置。
            close_source.index("self.xdg_decoration = None"),
            # 获取顶层窗口释放语句位置。
            close_source.index("self.toplevel = None"),
        )
        # 主演示必须实际启用自定义标题栏。
        self.assertIn(".custom_title_bar(true)", gui_demo)

    # 确认应用主动关闭复用 Wayland 原生关闭事实，不直接销毁 surface。
    def test_request_close_enters_the_per_window_event_queue(self) -> None:
        # 读取 Wayland 窗口操作实现。
        window_ops = WINDOW_OPS.read_text(encoding="utf-8")
        # 定位主动关闭窄端口。
        request_start = window_ops.index("fn os_request_close")
        # 以交互移动注释限定关闭实现片段。
        request_end = window_ops.index("// 将当前 PointerDown", request_start)
        # 保存关闭实现，避免其他回调队列代码造成假阳性。
        request_source = window_ops[request_start:request_end]
        # 关闭意图必须进入当前 Wayland owner 的事件队列。
        self.assertIn("self.events", request_source)
        # 同步端口必须把队列损坏分类为稳定 InvalidState。
        self.assertIn("Errc::InvalidState", request_source)
        # 诊断必须保留主动关闭与逐窗队列阶段。
        self.assertIn("Wayland request-close event queue mutex poisoned", request_source)
        # 主动关闭不得恢复 poisoned queue 后伪报成功。
        self.assertNotIn("into_inner()", request_source)
        # 同步调用方是唯一 failure receiver，不另行写 pending source。
        self.assertNotIn("pending_failures", request_source)
        # 必须复用和 compositor close callback 相同的统一事件。
        self.assertIn("UiEvent::close().for_window(self.window_id)", request_source)
        # 每次健康调用只允许投递一个关闭事实。
        self.assertEqual(request_source.count("UiEvent::close().for_window(self.window_id)"), 1)
        # 平台窄端口不得在交付关闭意图时提前销毁原生资源。
        self.assertNotIn("self.surface = None", request_source)

    # 确认交互移动不会从 poisoned registry 消费一次性 serial。
    def test_move_drag_checks_pointer_activation_registry_owner(self) -> None:
        # 读取指针激活注册表 Component。
        activation = WAYLAND_POINTER_ACTIVATION.read_text(encoding="utf-8")
        # 读取 WindowOps 的交互移动编排。
        window_ops = WINDOW_OPS.read_text(encoding="utf-8")
        # 限定 checked consume 实现。
        checked_start = activation.index("pub(crate) fn consume_checked")
        # 既有纯消费方法标记 checked 入口末尾。
        checked_end = activation.index("pub(crate) fn consume(", checked_start)
        # 保存共享 owner 检查片段。
        checked = activation[checked_start:checked_end]
        # mutex poison 必须稳定分类为 InvalidState。
        self.assertIn("Errc::InvalidState", checked)
        # 诊断必须保留移动请求与授权注册表阶段。
        self.assertIn("pointer activation registry mutex poisoned during move request", checked)
        # checked 入口不得恢复 poisoned registry。
        self.assertNotIn("into_inner()", checked)
        # 共享 owner lock 必须发生在任何授权消费之前。
        self.assertLess(checked.index("registry.lock()"), checked.index("registry.consume("))
        # 限定 WindowOps 交互移动实现。
        move_start = window_ops.index("fn os_begin_move_drag")
        # 窗口标题操作标记移动实现末尾。
        move_end = window_ops.index("fn os_set_title", move_start)
        # 保存移动协议编排片段。
        move_source = window_ops[move_start:move_end]
        # WindowOps 必须委托授权 owner 的 checked 入口。
        checked_call = move_source.index("WaylandPointerActivationRegistry::consume_checked(")
        # 授权结果分派只能发生在 checked 调用成功后。
        match_outcome = move_source.index("match outcome")
        # typed failure 必须在 seat/toplevel 检查和协议提交前返回。
        self.assertLess(checked_call, match_outcome)
        # WindowOps 不得自行恢复 poisoned registry。
        self.assertNotIn("into_inner()", move_source)
        # 同步调用方是唯一 failure receiver，不另行写 pending source。
        self.assertNotIn("pending_failures", move_source)
        # 健康授权仍只提交一次 Wayland move 请求。
        self.assertEqual(move_source.count("toplevel._move(seat.as_ref(), serial)"), 1)


if __name__ == "__main__":
    unittest.main()
