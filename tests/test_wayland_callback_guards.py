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
        # 以窗口外观分区标记限定关闭实现片段。
        close_end = window_ops.index("// ── 窗口外观", close_start)
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
        # 必须复用和 compositor close callback 相同的统一事件。
        self.assertIn("UiEvent::close().for_window(self.window_id)", request_source)
        # 平台窄端口不得在交付关闭意图时提前销毁原生资源。
        self.assertNotIn("self.surface = None", request_source)


if __name__ == "__main__":
    unittest.main()
