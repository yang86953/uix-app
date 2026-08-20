# -*- coding: utf-8 -*-
"""Keep Wayland dispatch panics inside the owner-thread failure boundary."""
from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
EVENT_LOOP = ROOT / "src/native/backends/linux/windowing/wayland/event_loop.rs"
COMPAT = ROOT / "src/native/backends/linux/windowing/wayland/compat.rs"
# 读取 wl_output callback 与显示状态 owner 的接线。
WAYLAND_BACKEND = ROOT / "src/native/backends/linux/windowing/wayland/mod.rs"
# 读取单窗口 callback adapter 的 backend source 注入点。
WAYLAND_WINDOW_FACTORY = ROOT / "src/native/backends/linux/windowing/wayland/window.rs"
# 读取 Wayland clipboard data_source callback。
WAYLAND_CLIPBOARD = ROOT / "src/native/backends/linux/windowing/wayland/clipboard.rs"
# 读取 Wayland seat 的 data-device callback 委托边界。
WAYLAND_SEAT = ROOT / "src/native/backends/linux/windowing/wayland/seat.rs"
# 读取 Wayland frame callback 私有 Component。
WAYLAND_FRAME_CALLBACK = ROOT / "src/native/backends/linux/windowing/wayland/frame_callback.rs"
# 读取 Wayland surface 跨注册表注销 Component。
WAYLAND_SURFACE_REGISTRATION = ROOT / "src/native/backends/linux/windowing/wayland/surface_registration.rs"
# 读取 Wayland 指针激活授权 Component。
WAYLAND_POINTER_ACTIVATION = ROOT / "src/native/backends/linux/windowing/wayland/pointer_activation.rs"
# 读取逐窗移动与缩放授权消费及协议提交 Component。
WAYLAND_WINDOW_INTERACTION = ROOT / "src/native/backends/linux/windowing/wayland/window_interaction.rs"
# 读取 Linux 平台的 owner-thread 失败提取边界。
PLATFORM = ROOT / "src/native/backends/linux/platform.rs"
# 定位 Wayland 窗口操作与装饰模式实现。
WINDOW_OPS = ROOT / "src/native/backends/linux/windowing/wayland/window_ops.rs"
# 定位逐窗 surface descriptor、装饰映射与注销身份 Component。
WAYLAND_WINDOW_SURFACE = ROOT / "src/native/backends/linux/windowing/wayland/window_surface.rs"
# 定位逐窗 xdg-shell callback teardown Component。
WINDOW_CALLBACK_SHUTDOWN = ROOT / "src/native/backends/linux/windowing/wayland/window_callback_shutdown.rs"
# 定位实际启用自定义标题栏的主演示入口（uix-lang-demo）。
GUI_DEMO = ROOT / "demo/uix-lang-demo/src/main.rs"


class WaylandCallbackGuardTests(unittest.TestCase):
    def test_dispatch_pending_catches_callback_panics(self) -> None:
        source = EVENT_LOOP.read_text(encoding="utf-8")
        self.assertIn("catch_unwind", source)
        self.assertIn("event_queue.dispatch_pending", source)
        self.assertIn("callback panicked during dispatch", source)
        self.assertIn("Errc::PlatformError", source)

    # 确认 owner-thread 不会从 poisoned Wayland 事件队列继续取事件。
    def test_next_event_reports_poisoned_queue_to_pending_source(self) -> None:
        # 读取 Wayland 事件循环 owner-thread adapter。
        source = EVENT_LOOP.read_text(encoding="utf-8")
        # 限定 next_event 实现。
        next_start = source.index("pub(crate) fn next_event")
        # 内部 poll helper 分区标记 next_event 末尾。
        next_end = source.index("// ── 内部辅助", next_start)
        # 保存事件提取窄端口。
        next_event = source[next_start:next_end]
        # 队列 owner 必须先使用 checked lock。
        lock_queue = next_event.index("self.events.lock()")
        # 锁失败必须进入 backend 既有 failure source。
        enqueue_failure = next_event.index("self.enqueue_failure")
        # poison 分支必须返回普通无事件形状。
        return_none = next_event.index("return None")
        # 健康路径才允许弹出队首事件。
        pop_event = next_event.index("events.pop_front()")
        # owner 检查必须先于 failure 转交。
        self.assertLess(lock_queue, enqueue_failure)
        # failure 必须先入队再返回 None。
        self.assertLess(enqueue_failure, return_none)
        # pop 只能位于 poison 早退之后的健康路径。
        self.assertLess(return_none, pop_event)
        # next_event 不得恢复 poisoned queue。
        self.assertNotIn("into_inner()", next_event)
        # 锁中毒必须稳定分类为 InvalidState。
        self.assertIn("Errc::InvalidState", next_event)
        # 诊断必须保留 next_event 队列阶段。
        self.assertIn("Wayland next_event queue mutex poisoned", next_event)
        # 窄端口不得执行 owner-thread 之外的错误处理策略。
        for forbidden in ["tracing::", ".report(", "attempt_recovery"]:
            # 任何一项都不得进入事件提取路径。
            self.assertNotIn(forbidden, next_event)
        # 每次健康调用只允许弹出一个事件。
        self.assertEqual(next_event.count("events.pop_front()"), 1)

    # 确认阻塞 dispatch 不会依据 poisoned 按键重复状态选择 poll 策略。
    def test_dispatch_blocking_checks_key_repeat_state_owners(self) -> None:
        # 读取 Wayland owner-thread 事件循环。
        source = EVENT_LOOP.read_text(encoding="utf-8")
        # 限定阻塞 dispatch 实现。
        dispatch_start = source.index("pub(crate) fn dispatch_blocking")
        # 超时入口标记阻塞实现末尾。
        dispatch_end = source.index("pub(crate) fn dispatch_timeout", dispatch_start)
        # 保存按键重复调度决策片段。
        dispatch = source[dispatch_start:dispatch_end]
        # 先读取 held-key owner。
        held_lock = dispatch.index("self.held_key_info.lock()")
        # 再读取 repeat-rate owner。
        rate_lock = dispatch.index("self.repeat_rate.lock()")
        # 健康状态全部取得后才允许选择重复超时分支。
        repeat_branch = dispatch.index("if has_held && repeat_rate > 0")
        # 两份状态必须保持固定非嵌套读取顺序。
        self.assertLess(held_lock, rate_lock)
        # 调度选择必须晚于两次 owner 检查。
        self.assertLess(rate_lock, repeat_branch)
        # 阻塞入口不得恢复任一 poisoned mutex。
        self.assertNotIn("into_inner()", dispatch)
        # 两个 failure 分支都必须稳定分类为 InvalidState。
        self.assertEqual(dispatch.count("Errc::InvalidState"), 2)
        # held-key 诊断必须可独立定位。
        self.assertIn("dispatch_blocking held-key mutex poisoned", dispatch)
        # repeat-rate 诊断也必须可独立定位。
        self.assertIn("dispatch_blocking repeat-rate mutex poisoned", dispatch)
        # 两种 owner failure 都复用 backend pending source。
        self.assertEqual(dispatch.count("self.enqueue_failure"), 2)
        # 健康重复路径必须继续委托 timeout dispatch。
        self.assertIn("self.dispatch_timeout(Duration::from_millis", dispatch)
        # 无重复输入时仍保持无限期阻塞 poll。
        self.assertIn('self.dispatch_polled(-1, "dispatch_blocking")', dispatch)
        # 窄端口不得执行错误策略或用户代码。
        for forbidden in ["tracing::", ".report(", "attempt_recovery"]:
            # 任何一项都不得进入阻塞调度决策。
            self.assertNotIn(forbidden, dispatch)

    # 确认 poll 快照不会包含 poisoned clipboard I/O owner 的 FD。
    def test_dispatch_polled_checks_clipboard_io_owners_before_poll(self) -> None:
        # 读取 Wayland owner-thread poll 编排。
        source = EVENT_LOOP.read_text(encoding="utf-8")
        # 限定 dispatch_polled 实现。
        dispatch_start = source.index("fn dispatch_polled")
        # 按键重复 helper 标记 poll 实现末尾。
        dispatch_end = source.index("fn generate_key_repeats", dispatch_start)
        # 保存完整 poll 编排。
        dispatch = source[dispatch_start:dispatch_end]
        # 系统 poll 调用标记快照完成边界。
        poll_call = dispatch.index("let ret = unsafe")
        # 只审计系统调用前的 FD owner 快照。
        snapshot = dispatch[:poll_call]
        # read owner 必须先检查。
        read_lock = snapshot.index("self.clipboard_read.lock()")
        # write owners 随后检查。
        write_lock = snapshot.index("self.clipboard_writes.lock()")
        # 快照固定保持 read 后 write 的检查顺序。
        self.assertLess(read_lock, write_lock)
        # 系统 poll 必须晚于两份 owner 检查。
        self.assertLess(write_lock, poll_call)
        # poll 前不得恢复任一 poisoned clipboard owner。
        self.assertNotIn("into_inner()", snapshot)
        # read owner failure 必须有稳定阶段诊断。
        self.assertIn("dispatch_polled clipboard-read mutex poisoned", snapshot)
        # write owner failure 也必须独立可定位。
        self.assertIn("dispatch_polled clipboard-write mutex poisoned", snapshot)
        # 两个 failure 分支均稳定分类为 InvalidState。
        self.assertEqual(snapshot.count("Errc::InvalidState"), 2)
        # 两个 failure 分支都复用 backend pending source。
        self.assertEqual(snapshot.count("self.enqueue_failure"), 2)
        # 健康 read FD 仍监听 POLLIN。
        self.assertIn("events: POLLIN", snapshot)
        # 健康 write FD 仍监听 POLLOUT。
        self.assertIn("events: POLLOUT", snapshot)
        # 快照阶段不得执行错误策略或用户代码。
        for forbidden in ["tracing::", ".report(", "attempt_recovery"]:
            # 任何一项都不得进入 poll FD 构造。
            self.assertNotIn(forbidden, snapshot)

    # 确认 poll completion 不会读取、写入或移除 poisoned clipboard owner。
    def test_clipboard_completion_propagates_owner_state_failures(self) -> None:
        # 读取 Wayland owner-thread event loop。
        source = EVENT_LOOP.read_text(encoding="utf-8")
        # 限定 dispatch_polled 调用方传播片段。
        dispatch_start = source.index("fn dispatch_polled")
        # 按键重复 helper 标记 poll 编排末尾。
        dispatch_end = source.index("fn generate_key_repeats", dispatch_start)
        # 保存 poll completion 调用方。
        dispatch = source[dispatch_start:dispatch_end]
        # read completion 必须通过布尔通道失败即早退。
        self.assertIn("if !self.read_clipboard_pipe(polled_fd)", dispatch)
        # read FD error 也必须委托 checked owner helper。
        self.assertIn("if !self.discard_clipboard_read_after_poll_error(polled_fd)", dispatch)
        # write completion 必须通过 Option 通道区分状态失败。
        self.assertIn("let Some(written) = self.write_clipboard_pipe", dispatch)
        # 任一 helper 状态失败都终止本轮 dispatch。
        self.assertGreaterEqual(dispatch.count("return false"), 4)
        # 限定 read FD error owner helper。
        read_error_start = source.index("fn discard_clipboard_read_after_poll_error")
        # read completion helper 标记 error helper 末尾。
        read_error_end = source.index("fn read_clipboard_pipe", read_error_start)
        # 保存 read error 处理片段。
        read_error = source[read_error_start:read_error_end]
        # read error helper 必须 checked lock owner。
        self.assertIn("match self.clipboard_read.lock()", read_error)
        # read error helper 不得恢复 poisoned owner。
        self.assertNotIn("into_inner()", read_error)
        # read owner failure 必须稳定分类。
        self.assertIn("Errc::InvalidState", read_error)
        # read owner failure 诊断必须保留 completion 阶段。
        self.assertIn("clipboard read owner mutex poisoned during poll completion", read_error)
        # 限定 read completion helper。
        read_start = source.index("fn read_clipboard_pipe")
        # write completion helper标记 read helper 末尾。
        read_end = source.index("fn write_clipboard_pipe", read_start)
        # 保存 read completion 事务。
        read_completion = source[read_start:read_end]
        # read owner 必须先检查。
        read_lock = read_completion.index("match self.clipboard_read.lock()")
        # text owner 必须随后检查。
        text_lock = read_completion.index("match self.clipboard_text.lock()")
        # 非阻塞 I/O 只能在两个 owner 健康后开始。
        read_io = read_completion.index("read.read_available()")
        # 文本提交必须发生在 read owner 移除之前。
        text_commit = read_completion.index("*clipboard_text =")
        # 完成分支的首次 read owner 移除。
        release_read = read_completion.index("*active = None")
        # 固定保持 read 后 text 的锁顺序。
        self.assertLess(read_lock, text_lock)
        # 两次 owner 检查都先于 I/O。
        self.assertLess(text_lock, read_io)
        # 文本事实先提交，随后释放 read FD owner。
        self.assertLess(text_commit, release_read)
        # read completion 不得恢复任一 poisoned owner。
        self.assertNotIn("into_inner()", read_completion)
        # read 与 text 状态失败必须各自分类。
        self.assertEqual(read_completion.count("Errc::InvalidState"), 2)
        # text owner failure 诊断必须独立可定位。
        self.assertIn("clipboard text owner mutex poisoned during poll completion", read_completion)
        # 限定 write completion helper。
        write_start = source.index("fn write_clipboard_pipe")
        # failure 收口 helper 标记 write helper 末尾。
        write_end = source.index("fn close_after_failure", write_start)
        # 保存 write completion 片段。
        write_completion = source[write_start:write_end]
        # write helper 必须 checked lock owner。
        self.assertIn("match self.clipboard_writes.lock()", write_completion)
        # write helper 不得恢复 poisoned owner。
        self.assertNotIn("into_inner()", write_completion)
        # write 状态失败必须稳定分类。
        self.assertIn("Errc::InvalidState", write_completion)
        # write owner failure 诊断必须独立可定位。
        self.assertIn("clipboard write owner mutex poisoned during poll completion", write_completion)
        # owner failure 必须使用 None 且不伪装零进度。
        self.assertIn("return None", write_completion)
        # 健康路径仍保留协议错误 readiness。
        self.assertIn("POLLERR | POLLHUP | POLLNVAL", write_completion)
        # 健康路径仍只在 POLLOUT 时写入。
        self.assertIn("(revents & POLLOUT) == 0", write_completion)
        # completion helper 不得执行错误策略或用户代码。
        for forbidden in ["tracing::", ".report(", "attempt_recovery"]:
            # 所有 completion 片段都不得包含这些副作用。
            self.assertNotIn(forbidden, read_error + read_completion + write_completion)

    # 确认按键重复只在全部 owner 健康时提交状态与事件。
    def test_key_repeat_generation_commits_owner_state_transactionally(self) -> None:
        # 读取 Wayland owner-thread event loop。
        source = EVENT_LOOP.read_text(encoding="utf-8")
        # 限定 dispatch_polled 调用方传播片段。
        dispatch_start = source.index("fn dispatch_polled")
        # 按键重复 helper 标记 poll 编排末尾。
        dispatch_end = source.index("fn generate_key_repeats", dispatch_start)
        # 保存 poll completion 调用方。
        dispatch = source[dispatch_start:dispatch_end]
        # 生成失败必须显式终止本轮 dispatch。
        self.assertIn("if !self.generate_key_repeats()", dispatch)
        # 限定重复生成决策端口。
        generate_start = source.index("fn generate_key_repeats")
        # 焦点失配清理 helper 标记生成端口末尾。
        generate_end = source.index("fn clear_stale_key_repeat_state", generate_start)
        # 保存生成决策片段。
        generate = source[generate_start:generate_end]
        # held-key owner 必须 checked lock。
        held_lock = generate.index("match self.held_key_info.lock()")
        # surface target owner 随后 checked lock。
        target_lock = generate.index("match self.surface_windows.lock()")
        # repeat rate owner 在 target 验证后读取。
        rate_lock = generate.index("match self.repeat_rate.lock()")
        # repeat delay owner 最后参与时间计算。
        delay_lock = generate.index("match self.repeat_delay.lock()")
        # 四份读取保持固定非嵌套顺序。
        self.assertLess(held_lock, target_lock)
        # target 校验先于协议 rate。
        self.assertLess(target_lock, rate_lock)
        # rate 健康且启用后才读取 delay。
        self.assertLess(rate_lock, delay_lock)
        # 生成端口不得恢复 poisoned owner。
        self.assertNotIn("into_inner()", generate)
        # 四类读取 failure 均稳定分类为 InvalidState。
        self.assertEqual(generate.count("Errc::InvalidState"), 4)
        # 各 owner 诊断必须独立可定位。
        for stage in ["held-key", "surface target", "rate", "delay"]:
            # 每个状态阶段都必须进入生成诊断。
            self.assertIn(f"key repeat {stage} mutex poisoned during generation", generate)
        # 焦点失配必须委托双 owner 清理事务。
        self.assertIn("return self.clear_stale_key_repeat_state()", generate)
        # 健康路径保留 10ms 最小 interval。
        self.assertIn("Duration::from_millis(10)", generate)
        # 限定焦点失配清理事务。
        cleanup_start = source.index("fn clear_stale_key_repeat_state")
        # 事件提交 helper 标记清理事务末尾。
        cleanup_end = source.index("fn commit_key_repeat_if_due", cleanup_start)
        # 保存双 owner 清理片段。
        cleanup = source[cleanup_start:cleanup_end]
        # 清理先取得 held-key guard。
        cleanup_held_lock = cleanup.index("match self.held_key_info.lock()")
        # 清理再取得 last-time guard。
        cleanup_last_lock = cleanup.index("match self.last_repeat_time.lock()")
        # held-key 清除必须晚于两把健康 guard。
        clear_held = cleanup.index("*held_key = None")
        # last-time 清除紧随 held-key 清除。
        clear_last = cleanup.index("*last_repeat = None")
        # 固定保持 held-key 后 last-time 的锁顺序。
        self.assertLess(cleanup_held_lock, cleanup_last_lock)
        # 两把 guard 均健康后才开始清理。
        self.assertLess(cleanup_last_lock, clear_held)
        # 两份状态保持同一事务提交顺序。
        self.assertLess(clear_held, clear_last)
        # 清理事务不得恢复 poisoned owner。
        self.assertNotIn("into_inner()", cleanup)
        # 两类清理 failure 都稳定分类。
        self.assertEqual(cleanup.count("Errc::InvalidState"), 2)
        # 限定重复事件提交事务。
        commit_start = source.index("fn commit_key_repeat_if_due")
        # clipboard read error helper 标记提交事务末尾。
        commit_end = source.index("fn discard_clipboard_read_after_poll_error", commit_start)
        # 保存重复事件提交片段。
        commit = source[commit_start:commit_end]
        # 先取得 event queue guard。
        event_lock = commit.index("match self.events.lock()")
        # 随后取得 last-time guard。
        commit_last_lock = commit.index("match self.last_repeat_time.lock()")
        # 到期判断必须在两把 guards 健康后重新执行。
        due_check = commit.index("let should_fire = match *last_repeat")
        # 时间戳只能在两把 guard 健康后推进。
        timestamp_commit = commit.index("*last_repeat = Some(now)")
        # key-down 是首个窗口事件。
        key_event = commit.index("events.push_back(UiEvent::key_down")
        # 可选 text-input 紧随 key-down。
        text_event = commit.index("events.push_back(UiEvent::text_input")
        # 固定保持 event queue 后 last-time 的全局锁顺序。
        self.assertLess(event_lock, commit_last_lock)
        # 两把 guards 健康后才重新判断到期。
        self.assertLess(commit_last_lock, due_check)
        # 到期判断后才推进时间。
        self.assertLess(due_check, timestamp_commit)
        # 时间戳与事件保持确定性提交顺序。
        self.assertLess(timestamp_commit, key_event)
        # key-down 必须先于可选文本事件。
        self.assertLess(key_event, text_event)
        # 提交事务不得恢复 poisoned owner。
        self.assertNotIn("into_inner()", commit)
        # last-time 与 event queue failure 均稳定分类。
        self.assertEqual(commit.count("Errc::InvalidState"), 2)
        # 两类提交 owner 诊断必须独立可定位。
        self.assertIn("last-time mutex poisoned during event commit", commit)
        # 事件队列诊断必须保留提交阶段。
        self.assertIn("event queue mutex poisoned during event commit", commit)
        # 重复生成路径不得执行错误策略或用户代码。
        for forbidden in ["tracing::", ".report(", "attempt_recovery"]:
            # 三个重复端口都不得包含这些副作用。
            self.assertNotIn(forbidden, generate + cleanup + commit)

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

    # 确认 set_text 不会从 poisoned 输入状态读取 selection serial。
    def test_clipboard_set_text_checks_input_serial_owner(self) -> None:
        # 读取 Wayland clipboard 同步 adapter 与 data source callback。
        source = WAYLAND_CLIPBOARD.read_text(encoding="utf-8")
        # 限定 set_text 实现。
        set_start = source.index("fn set_text")
        # has_text 方法标记 set_text 片段末尾。
        set_end = source.index("fn has_text", set_start)
        # 保存完整剪贴板写入编排。
        set_text = source[set_start:set_end]
        # 输入 serial mutex 必须使用 checked lock。
        serial_lock = set_text.index("last_input_serial.lock()")
        # data source 创建标记首个 Wayland selection 协议动作。
        create_source = set_text.index("dm.create_data_source()")
        # MIME offer 必须保持在健康 serial 之后。
        offer = set_text.index('source.offer("text/plain;charset=utf-8"')
        # selection 提交必须使用同一健康 serial。
        set_selection = set_text.index("dd.set_selection(Some(&source), serial)")
        # serial owner 检查必须先于任何 selection 协议对象创建。
        self.assertLess(serial_lock, create_source)
        # data source 必须先声明既有 MIME。
        self.assertLess(create_source, offer)
        # MIME 声明必须先于 selection 提交。
        self.assertLess(offer, set_selection)
        # 同步 adapter 不得恢复 poisoned input state。
        self.assertNotIn("into_inner()", set_text)
        # 锁中毒必须稳定分类为 InvalidState。
        self.assertIn("Errc::InvalidState", set_text)
        # 诊断必须保留 clipboard set_text 与 serial 阶段。
        self.assertIn("clipboard input serial mutex poisoned during set_text", set_text)
        # 既有无 serial 失败仍必须保留。
        self.assertIn("no pointer or keyboard serial for selection", set_text)

    # 确认 data-device Selection 只在全部 clipboard owners 健康后提交。
    def test_clipboard_selection_commits_owner_state_transactionally(self) -> None:
        # 读取 clipboard callback Component。
        clipboard = WAYLAND_CLIPBOARD.read_text(encoding="utf-8")
        # 读取 seat 的协议 callback 委托。
        seat = WAYLAND_SEAT.read_text(encoding="utf-8")
        # 限定 seat 内 data-device callback 片段。
        seat_start = seat.index("// ── 剪贴板数据设备")
        # pointer/keyboard 分区标记 data-device 片段末尾。
        seat_end = seat.index("// ── 指针 + 键盘", seat_start)
        # 保存 seat adapter 片段。
        seat_callback = seat[seat_start:seat_end]
        # seat 只允许委托 clipboard Component。
        self.assertIn("super::clipboard::handle_selection_event", seat_callback)
        # seat 不得直接锁 selection owners。
        self.assertNotIn(".lock()", seat_callback)
        # seat 不得恢复 poisoned owner。
        self.assertNotIn("into_inner()", seat_callback)
        # 限定 clipboard Selection Component。
        component_start = clipboard.index("fn enqueue_selection_state_failure")
        # write cursor owner 标记 Selection Component 末尾。
        component_end = clipboard.index("pub(crate) struct ClipboardWrite", component_start)
        # 保存完整 Selection Component。
        component = clipboard[component_start:component_end]
        # Component 不得恢复任一 poisoned owner。
        self.assertNotIn("into_inner()", component)
        # 状态失败统一分类为 InvalidState。
        self.assertIn("Errc::InvalidState", component)
        # 诊断必须保留精确 owner 名称。
        for owner in ["owns flag", "read owner", "text owner"]:
            # 三个 owner 都必须有稳定 failure 参数。
            self.assertIn(f'"{owner}"', component)
        # 限定 Some(offer) 接管事务。
        offer_start = component.index("fn apply_selection_offer")
        # None 清理 helper 标记接管事务末尾。
        offer_end = component.index("fn clear_selection", offer_start)
        # 保存 offer 接管片段。
        offer = component[offer_start:offer_end]
        # ownership owner 必须先取得。
        owns_lock = offer.index("owns_clipboard.lock()")
        # read owner 随后取得。
        read_lock = offer.index("clipboard_read.lock()")
        # pipe 只能在两个 owner 健康后创建。
        create_pipe = offer.index("ClipboardRead::create_pipe()")
        # receive 只能在 pipe 创建成功后提交。
        receive = offer.index("offer.receive(")
        # 固定保持 owns→read 锁序。
        self.assertLess(owns_lock, read_lock)
        # owner 检查全部先于 I/O。
        self.assertLess(read_lock, create_pipe)
        # pipe 创建先于协议 receive。
        self.assertLess(create_pipe, receive)
        # 健康路径保留既有 MIME。
        self.assertIn('"text/plain;charset=utf-8"', offer)
        # pipe error 必须清除陈旧 read owner。
        self.assertIn("*active_read = None", offer)
        # 限定 None selection 清理事务。
        clear_start = component.index("fn clear_selection")
        # 公开 handler 标记清理事务末尾。
        clear_end = component.index("pub(crate) fn handle_selection_event", clear_start)
        # 保存三 owner 清理片段。
        clear = component[clear_start:clear_end]
        # 三把锁保持 owns→read→text 顺序。
        self.assertLess(clear.index("owns_clipboard.lock()"), clear.index("clipboard_read.lock()"))
        # text owner 必须最后验证。
        self.assertLess(clear.index("clipboard_read.lock()"), clear.index("clipboard_text.lock()"))
        # 三份状态提交只能发生在最后一把锁之后。
        self.assertLess(clear.index("clipboard_text.lock()"), clear.index("*owns = false"))
        # callback Component 不得执行错误策略或用户代码。
        for forbidden in ["tracing::", ".report(", "attempt_recovery"]:
            # 每项副作用都禁止进入 Selection callback。
            self.assertNotIn(forbidden, component)

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
        self.assertIn("frame request mutex poisoned during registration", frame_callback)
        # request 取消锁失败必须同步返回 typed error。
        self.assertIn("frame request mutex poisoned during cancellation", frame_callback)
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
        # 读取实际拥有可重试 surface identity 的私有 Component。
        window_surface = WAYLAND_WINDOW_SURFACE.read_text(encoding="utf-8")
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
        method_start = window_surface.index("fn unregister_surface")
        # 保存位于文件末部的身份消费编排片段。
        method = window_surface[method_start:]
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
        # frame request 与 callback owner 必须通过 checked Component 同步清理。
        self.assertIn("self.frame_callback.close_checked()?", close_source)
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
        # 读取将标题栏可见性映射到协议模式的私有 Component。
        window_surface = WAYLAND_WINDOW_SURFACE.read_text(encoding="utf-8")
        # 读取逐窗 callback teardown Component。
        callback_shutdown = WINDOW_CALLBACK_SHUTDOWN.read_text(encoding="utf-8")
        # 读取主演示的窗口配置。
        gui_demo = GUI_DEMO.read_text(encoding="utf-8")
        # Wayland 后端必须实现统一的系统标题栏可见性能力。
        self.assertIn("fn os_set_system_title_bar_visible", window_ops)
        # WindowOps 只通过窄 Component 映射公开布尔契约。
        self.assertIn("Self::title_bar_decoration_mode(visible)", window_ops)
        # 隐藏系统标题栏必须请求客户端装饰。
        self.assertIn("XdgDecoMode::ClientSide", window_surface)
        # 恢复系统标题栏必须请求服务端装饰。
        self.assertIn("XdgDecoMode::ServerSide", window_surface)
        # 关闭窗口时必须先释放依赖顶层窗口的装饰对象。
        close_start = window_ops.index("fn os_close")
        # 以紧邻的主动关闭方法限定显式 teardown 片段。
        close_end = window_ops.index("fn os_request_close", close_start)
        # 保存关闭实现，避免其他生命周期赋值影响断言。
        close_source = window_ops[close_start:close_end]
        # 装饰对象必须在逐窗 callback owner 消费前释放。
        self.assertLess(
            # 获取装饰释放语句位置。
            close_source.index("self.xdg_decoration = None"),
            # 获取 callback teardown Component 调用位置。
            close_source.index("self.shutdown_window_callbacks()"),
        )
        # Component 必须先消费并注销顶层窗口 callback owner。
        toplevel_take = callback_shutdown.index("self.toplevel.take()")
        # 随后才消费更底层的 xdg_surface callback owner。
        surface_take = callback_shutdown.index("self.xdg_surface.take()")
        # 依赖层级决定顶层窗口先于 surface role 释放。
        self.assertLess(toplevel_take, surface_take)
        # 顶层窗口 owner 必须显式注销 callback。
        self.assertIn("toplevel.clear_callback()", callback_shutdown[toplevel_take:surface_take])
        # surface role owner 也必须显式注销 callback。
        self.assertIn("xdg_surface.clear_callback()", callback_shutdown[surface_take:])
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

    # 确认移动与缩放共用检查式授权 owner 和独立协议请求。
    def test_window_interactions_check_activation_and_submit_xdg_requests(self) -> None:
        # 读取指针激活注册表 Component。
        activation = WAYLAND_POINTER_ACTIVATION.read_text(encoding="utf-8")
        # 读取 WindowOps 的窄委托。
        window_ops = WINDOW_OPS.read_text(encoding="utf-8")
        # 读取移动与缩放协议交互 Component。
        interaction = WAYLAND_WINDOW_INTERACTION.read_text(encoding="utf-8")
        # 限定 checked consume 实现。
        checked_start = activation.index("pub(crate) fn consume_checked")
        # 既有纯消费方法标记 checked 入口末尾。
        checked_end = activation.index("pub(crate) fn consume(", checked_start)
        # 保存共享 owner 检查片段。
        checked = activation[checked_start:checked_end]
        # mutex poison 必须稳定分类为 InvalidState。
        self.assertIn("Errc::InvalidState", checked)
        # 诊断必须覆盖移动和缩放共用的交互窗口请求阶段。
        self.assertIn("during interactive window request", checked)
        # checked 入口不得恢复 poisoned registry。
        self.assertNotIn("into_inner()", checked)
        # 共享 owner lock 必须发生在任何授权消费之前。
        self.assertLess(checked.index("registry.lock()"), checked.index("registry.consume("))
        # WindowOps 必须分别委托移动与缩放窄入口。
        self.assertIn("window_interaction::begin_move_drag", window_ops)
        self.assertIn("window_interaction::begin_resize_drag", window_ops)
        # 交互 Component 必须委托授权 owner 的 checked 入口。
        checked_call = interaction.index("WaylandPointerActivationRegistry::consume_checked(")
        # 授权结果分派只能发生在 checked 调用成功后。
        match_outcome = interaction.index("match outcome")
        # typed failure 必须在 seat/toplevel 检查和协议提交前返回。
        self.assertLess(checked_call, match_outcome)
        # WindowOps 不得自行恢复 poisoned registry。
        self.assertNotIn("into_inner()", interaction)
        # 同步调用方是唯一 failure receiver，不另行写 pending source。
        self.assertNotIn("pending_failures", interaction)
        # 健康授权必须各有一个 Wayland move 与 resize 请求。
        self.assertEqual(interaction.count("toplevel._move(seat.as_ref(), serial)"), 1)
        self.assertEqual(interaction.count("toplevel.resize(seat.as_ref(), serial"), 1)


if __name__ == "__main__":
    unittest.main()
