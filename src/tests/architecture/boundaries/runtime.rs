use super::support::*;
use std::fs;
use std::path::Path;

#[test]
fn active_work_registry_stays_internal() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let checked = ["prelude.rs", "lib.rs", "app/mod.rs"];
    let mut violations = Vec::new();

    for rel in checked {
        let text = fs::read_to_string(src.join(rel)).unwrap();
        if text.contains("ActiveWorkRegistry") {
            violations.push(format!("{rel} exposes ActiveWorkRegistry"));
        }
    }

    assert!(
        violations.is_empty(),
        "ActiveWorkRegistry is framework-managed and must not be exposed to app code: {violations:?}"
    );
}

#[test]
fn graphics_backend_public_api_exposes_enum_not_context_factory() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let prelude = fs::read_to_string(src.join("prelude.rs")).unwrap();

    assert!(
        prelude.contains("GraphicsBackend"),
        "P6.5 exposes GraphicsBackend so App::graphics_backend can be used from prelude"
    );
    assert!(
        !prelude.contains("create_gpu_context_with_backend"),
        "low-level native GPU context creation stays out of prelude"
    );
}

#[test]
fn canvas_scroll_copy_requires_an_explicit_backend_semantic() {
    let canvas =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/draw/api/canvas.rs"))
            .expect("read Canvas2D contract");

    assert!(
        canvas.contains("fn scroll_region(&mut self, viewport: Rect, dx: f32, dy: f32);"),
        "Canvas2D scroll copy must not regain a silent no-op default"
    );
}

#[test]
fn native_contexts_use_checked_shutdown() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cases = [
        (
            "D3D12",
            "src/native/graphics/d3d12/platform/context.rs",
            "fn try_shutdown(&mut self) -> Result<()> {\n        self.shutdown_result()\n    }",
        ),
        (
            "WGL",
            "src/native/graphics/opengl/platform/wgl.rs",
            "fn try_shutdown(&mut self) -> Result<(), Error> {\n        self.shutdown_result()\n    }",
        ),
        (
            "EGL",
            "src/native/graphics/opengl/platform/egl.rs",
            "fn try_shutdown(&mut self) -> Result<(), Error> {\n        self.shutdown_result()\n    }",
        ),
        (
            "Vulkan",
            "src/native/graphics/vulkan/platform/context.rs",
            "fn try_shutdown(&mut self) -> Result<()> {\n        self.shutdown_result()\n    }",
        ),
        (
            "D3D11",
            "src/native/graphics/d3d11/platform/context.rs",
            "fn try_shutdown(&mut self) -> Result<()> {\n        self.shutdown_result()\n    }",
        ),
        (
            "Metal",
            "src/native/graphics/metal/platform/context.rs",
            "fn try_shutdown(&mut self) -> Result<()> {\n        self.shutdown_result()\n    }",
        ),
    ];

    for (backend, path, checked_shutdown) in cases {
        let source = read_source(root.join(path));
        assert!(
            source.contains(checked_shutdown),
            "{backend} checked shutdown must return shutdown_result instead of a legacy void hook"
        );
    }
}

#[test]
fn graphics_contracts_expose_only_checked_shutdown() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (contract, path) in [
        ("IGraphicsContext", "src/native/traits/present.rs"),
        ("RenderTarget", "src/draw/renderer/target.rs"),
        ("RenderBackend", "src/draw/backend/contract.rs"),
    ] {
        let source = read_source(root.join(path));
        assert!(
            source.contains("fn try_shutdown(&mut self) -> Result<(), Error>;"),
            "{contract} must require checked try_shutdown"
        );
        assert!(
            !source.contains("fn shutdown(&mut self);"),
            "{contract} must not keep a legacy void shutdown hook"
        );
        assert!(
            !source.contains(
                "fn try_shutdown(&mut self) -> Result<(), Error> {\n        self.shutdown();"
            ),
            "{contract} must not adapt checked shutdown to a void hook"
        );
    }
}

#[test]
fn thread_bound_drop_guards_foreign_thread_teardown() {
    let source = read_source(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/native/factory/thread_bound.rs"),
    );

    assert!(
        source.contains("impl Drop for ThreadBoundGraphicsContext"),
        "thread-bound contexts must own Drop so foreign-thread teardown stays typed"
    );
    assert!(
        source.contains("std::mem::forget(inner)"),
        "wrong-thread Drop must leak the native context instead of calling its Drop"
    );
}

#[test]
fn application_defers_show_until_first_present() {
    let application = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app/shell/application.rs"),
    )
    .expect("read application");
    let application_runtime = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app/shell/application/runtime.rs"),
    )
    .expect("read application runtime");
    let event_loop = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app/event_loop/event_loop.rs"),
    )
    .expect("read event loop");
    let window_driver =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app/window_driver.rs"))
            .expect("read window driver");

    assert!(
        application.contains("show deferred")
            && application_runtime.contains("WindowDriver::new(width, height, true)")
            && !application.contains("initial window show failed")
            && !application_runtime.contains("initial window show failed"),
        "Application must not ShowWindow before fonts/session/first present"
    );
    assert!(
        event_loop.contains("!platform_window.is_visible()")
            && window_driver.contains("if frame_committed && self.deferred_show")
            && window_driver.contains("first_present_ms=")
            && window_driver.contains("deferred show after first present failed"),
        "the shared window driver must reveal each window only after a successful first present"
    );
}

#[test]
fn root_and_secondary_windows_share_one_frame_driver() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let event_loop = read_source(root.join("src/app/event_loop/event_loop.rs"));
    let application = read_source(root.join("src/app/shell/application.rs"));
    let window_driver = read_source(root.join("src/app/window_driver.rs"));

    assert!(
        window_driver.contains("pub(crate) struct WindowDriver")
            && window_driver.contains("pub(crate) fn drive_frame("),
        "the ordered per-window frame pipeline must have one owner"
    );
    assert!(
        event_loop.contains("driver.drive_frame(WindowFrameContext")
            && application.contains("driver.drive_frame(WindowFrameContext"),
        "root and secondary windows must call the same WindowDriver"
    );
    for duplicate in [
        "ScenePipeline::new()",
        "PresentDamageTracker::new()",
        "fn sync_secondary_animation_deadlines",
        "fn ensure_secondary_surface_matches_window",
    ] {
        assert!(
            !event_loop.contains(duplicate) && !application.contains(duplicate),
            "outer loops must not regain duplicated frame-driver logic: {duplicate}"
        );
    }
}

#[test]
fn window_driver_uses_one_shot_surface_scheduler() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scheduler = read_source(root.join("src/app/frame_scheduler.rs"));
    let driver = read_source(root.join("src/app/window_driver.rs"));
    let event_loop = read_source(root.join("src/app/event_loop/event_loop.rs"));

    assert!(
        scheduler.contains("pub(crate) enum SurfaceState")
            && scheduler.contains("generation: u64")
            && scheduler.contains("next_request_id: u64")
            && scheduler.contains("outstanding: Option<FrameRequest>")
            && scheduler.contains("pub(crate) fn notify_opportunity(")
            && scheduler.contains("request.token != token")
            && scheduler.contains("SurfaceState::Recovering"),
        "per-window frame scheduling must retain token-safe one-shot and recovery state"
    );
    assert!(
        driver.contains("frame_scheduler: FrameScheduler")
            && driver.contains("active_work.register_open(kind)")
            && driver.contains("self.frame_scheduler.frame_failed(failure, frame_time)")
            && driver.contains("request_native_frame(NativeFrameRequest::after_present(token))")
            && driver.contains("platform_window.native_frame_presented(token)")
            && driver.contains("if frame_committed")
            && driver.contains("cancel_outstanding_native_frame"),
        "WindowDriver must own frame opportunities, open animation registrations, and bounded recovery"
    );
    assert!(
        event_loop.contains("let window_deadline = driver.next_deadline("),
        "the root event loop must wait on the shared driver's one-shot deadline"
    );
    for forbidden in [
        "ANIMATION_FRAME_INTERVAL",
        "Duration::from_millis(16)",
        "register(kind, now +",
    ] {
        assert!(
            !driver.contains(forbidden),
            "WindowDriver must not regain fixed-interval animation polling: {forbidden}"
        );
    }
}

#[test]
fn wayland_frame_pacing_and_shm_backpressure_are_protocol_driven() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let window_ops = read_source(root.join("src/native/backends/linux/wayland/window_ops.rs"));
    let presenter = read_source(root.join("src/native/backends/linux/wayland/presenter.rs"));
    let buffer_lease = read_source(root.join("src/native/shared/buffer_lease.rs"));

    assert!(
        window_ops.contains("let callback = surface.frame()")
            && window_ops.contains("UiEvent::frame_opportunity(")
            && window_ops.contains("current.as_ref() == Some(&request)")
            && window_ops.contains("request.token == token"),
        "Wayland must deliver only the exact outstanding wl_surface.frame request"
    );
    assert!(
        !presenter.contains("surface.frame()")
            && presenter.contains("ShmBuffer::try_acquire")
            && presenter.contains("Errc::WouldBlock")
            && presenter.contains("wl_buffer::Event::Release")
            && buffer_lease.contains("compare_exchange(false, true"),
        "Wayland SHM presentation must reuse only compositor-released buffers and back off when both are busy"
    );
}

#[test]
fn windows_frame_pacing_waits_off_thread_and_delivers_exactly_once() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let pacer = read_source(root.join("src/native/backends/windows/frame_pacer.rs"));
    let window_ops = read_source(root.join("src/native/backends/windows/window_ops.rs"));
    let wnd_proc = read_source(root.join("src/native/backends/windows/wnd_proc.rs"));

    assert!(
        pacer.contains("thread::Builder::new()")
            && pacer.contains("receiver.recv()")
            && pacer.contains("DwmFlush()")
            && pacer.contains(".matches_submitted(ticket)")
            && pacer.contains("WM_UIX_FRAME_OPPORTUNITY")
            && pacer.contains("PostMessageW("),
        "Windows must wait for DWM on a blocking worker and post one exact ticket back to Win32"
    );
    assert!(
        window_ops.contains("fn os_request_native_frame(")
            && window_ops.contains("self.frame_pacer.request(request)")
            && window_ops.contains("fn os_native_frame_presented(")
            && window_ops.contains("self.frame_pacer.presented(token)")
            && window_ops.contains("fn os_cancel_native_frame(")
            && window_ops.contains("self.frame_pacer.cancel(token)"),
        "each Windows window must own native frame request and cancellation"
    );
    assert!(
        pacer.contains("mark_submitted(token)")
            && pacer.contains("matches_submitted(ticket)")
            && pacer.contains("mpsc::sync_channel(1)"),
        "DWM waiting must start only after exact present confirmation and retain bounded backpressure"
    );
    assert!(
        wnd_proc.contains("complete_posted_frame(&binding.frame_pacer")
            && wnd_proc.contains("UiEvent::frame_opportunity(")
            && wnd_proc.contains("platform.push_event("),
        "the private Win32 message must become one WindowId-routed frame opportunity"
    );
    assert!(
        !window_ops.contains("DwmFlush") && !wnd_proc.contains("DwmFlush"),
        "DwmFlush must never block the UI thread"
    );
}

#[test]
fn macos_frame_pacing_is_lazy_exact_and_post_present() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let pacer = read_source(root.join("src/native/backends/macos/display_link.rs"));
    let platform = read_source(root.join("src/native/backends/macos/platform.rs"));
    let mailbox = read_source(root.join("src/native/shared/native_frame_mailbox.rs"));

    assert!(
        pacer.contains("displayLinkWithTarget:selector:")
            && pacer.contains("respondsToSelector:")
            && pacer.contains("NSRunLoopCommonModes")
            && !pacer.contains("CVDisplayLink"),
        "macOS must use the modern window-bound display link with an availability fallback"
    );
    assert!(
        pacer.contains("NativeFrameRequestPhase::AfterPresent")
            && pacer.contains(".mark_submitted(token)")
            && pacer.contains("set_paused(self.display_link, false)")
            && pacer.contains("set_paused(display_link, true)")
            && pacer.contains("msg_void(display_link, \"invalidate\")"),
        "macOS display pacing must arm lazily, start only after exact present, and invalidate submitted cancellation"
    );
    assert!(
        pacer.contains(".take_submitted()")
            && pacer.contains("UiEvent::frame_opportunity(")
            && pacer.contains("CACurrentMediaTime()")
            && pacer.contains("CFRunLoopWakeUp(run_loop)"),
        "one native callback must deliver one exact WindowId-routed opportunity and wake the main run loop"
    );
    assert!(
        mailbox.contains("self.pending.is_some_and(|request| request.token == token)")
            && mailbox.contains("let replaced_submitted = self.submitted")
            && mailbox.contains("pub(crate) fn take_submitted("),
        "the shared native mailbox must preserve exact token and stale-source isolation"
    );
    assert!(
        platform.contains("self.frame_pacer.request(request)")
            && platform.contains("self.frame_pacer.presented(token)")
            && platform.contains("self.frame_pacer.cancel(token)"),
        "each macOS window must own native frame request, present confirmation, and cancellation"
    );
    let shutdown = platform
        .find("self.frame_pacer.shutdown()")
        .expect("MacosWindowOps must shut down its display link");
    let release = platform
        .find("cocoa::release_object(window)")
        .expect("MacosWindowOps must release its NSWindow");
    assert!(
        shutdown < release,
        "the display link must stop before its NSWindow owner is released"
    );
}

#[test]
fn hidden_windows_suspend_visual_work_until_their_exact_show_signal() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let events = read_source(root.join("src/native/traits/event/types.rs"));
    let scheduler = read_source(root.join("src/app/frame_scheduler.rs"));
    let driver = read_source(root.join("src/app/window_driver.rs"));
    let wnd_proc = read_source(root.join("src/native/backends/windows/wnd_proc.rs"));

    assert!(
        events.contains("WindowShow")
            && events.contains("WindowHide")
            && events.contains("pub fn window_show()")
            && events.contains("pub fn window_hide()"),
        "visibility lifecycle events must be platform-independent"
    );
    assert!(
        scheduler.contains("pub(crate) enum SurfaceSuspendReason")
            && scheduler.contains("    Hidden,")
            && scheduler.contains("pub(crate) fn suspended_reason("),
        "the scheduler must retain the exact suspension reason"
    );
    assert!(
        driver.contains("UiEventType::WindowHide")
            && driver.contains("UiEventType::WindowShow")
            && driver.contains("self.suspend_if_surface_unavailable(tree, platform_window)")
            && driver.contains("== Some(SurfaceSuspendReason::Hidden)"),
        "WindowDriver must block hidden visual work and resume only Hidden"
    );
    assert!(
        wnd_proc.contains("WM_SHOWWINDOW")
            && wnd_proc.contains("UiEvent::window_show()")
            && wnd_proc.contains("UiEvent::window_hide()")
            && wnd_proc.contains("self.push_event("),
        "Win32 visibility changes must update state and route to one WindowId"
    );
}

#[test]
fn d3d11_occlusion_is_a_per_window_idle_probe_not_graphics_recovery() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let present_traits = read_source(root.join("src/native/traits/present.rs"));
    let d3d11 = read_source(root.join("src/native/graphics/d3d11/platform/context.rs"));
    let d3d12_context = read_source(root.join("src/native/graphics/d3d12/platform/context.rs"));
    let d3d12_swap_chain =
        read_source(root.join("src/native/graphics/d3d12/platform/swap_chain.rs"));
    let scheduler = read_source(root.join("src/app/frame_scheduler.rs"));
    let driver = read_source(root.join("src/app/window_driver.rs"));
    let recovery_driver = read_source(root.join("src/draw/renderer/recovery_driver.rs"));

    assert!(
        present_traits.contains("pub enum PresentTestResult")
            && present_traits.contains("fn test_present(&mut self)")
            && present_traits.contains("Errc::NotImplemented"),
        "idle present probing must be an optional typed capability with zero caller maintenance"
    );
    assert!(
        d3d11.contains("DXGI_STATUS_OCCLUDED")
            && d3d11.contains("Errc::GraphicsOccluded")
            && d3d11.contains("Present(0, DXGI_PRESENT_TEST)")
            && d3d11.contains("DXGI_SWAP_EFFECT_DISCARD"),
        "D3D11 bitblt presentation must classify occlusion and use only DXGI's no-data exit probe"
    );
    assert!(
        d3d12_swap_chain.contains("DXGI_SWAP_EFFECT_FLIP_DISCARD")
            && !d3d12_swap_chain.contains("fn test_present(")
            && !d3d12_context.contains("fn test_present("),
        "the flip-model D3D12 path must not falsely advertise DXGI occlusion status support"
    );
    assert!(
        scheduler.contains("    Occluded,")
            && scheduler.contains("OCCLUSION_PROBE_BASE_DELAY")
            && scheduler.contains("OCCLUSION_PROBE_MAX_DELAY")
            && scheduler.contains("pub(crate) fn take_due_occlusion_probe(")
            && scheduler.contains("SurfaceState::Suspended(_) => false"),
        "occlusion probes must be bounded registered work, never visual frame opportunities"
    );
    assert!(
        driver.contains("if self.frame_scheduler.take_due_occlusion_probe(now)")
            && driver.contains("engine.test_present()")
            && driver.contains("Ok(PresentTestResult::Occluded)")
            && driver.contains("Ok(PresentTestResult::Presentable)"),
        "WindowDriver must test occlusion before entering animation/layout/paint/present"
    );
    assert!(
        recovery_driver.contains("matches!(failure, GraphicsFailure::Occluded(_))")
            && recovery_driver.contains("self.engine.test_present()"),
        "a healthy occluded swapchain must be probed in place rather than rebuilt"
    );
}

#[test]
fn macos_occlusion_is_an_exact_per_window_signal_without_polling() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let window_traits = read_source(root.join("src/native/traits/window.rs"));
    let events = read_source(root.join("src/native/traits/event/types.rs"));
    let shared_window = read_source(root.join("src/native/shared/window.rs"));
    let delegate = read_source(root.join("src/native/backends/macos/window_delegate.rs"));
    let platform = read_source(root.join("src/native/backends/macos/platform.rs"));
    let driver = read_source(root.join("src/app/window_driver.rs"));

    assert!(
        window_traits.contains("pub enum WindowOcclusionState")
            && window_traits.contains("fn occlusion_state(&self) -> WindowOcclusionState")
            && window_traits.contains("WindowOcclusionState::Unknown")
            && shared_window.contains("fn os_occlusion_state(&self) -> WindowOcclusionState"),
        "exact compositor visibility must be optional so unsupported backends cannot claim exposure"
    );
    assert!(
        events.contains("WindowOcclusionChanged")
            && events.contains("pub fn window_occlusion_changed()"),
        "native occlusion changes must wake the shared per-window driver"
    );
    assert!(
        delegate.contains("windowDidChangeOcclusionState:")
            && delegate.contains("window_did_change_occlusion_state")
            && delegate.contains("UiEvent::window_occlusion_changed()")
            && delegate.contains("event.for_window((*context).window_id)"),
        "the AppKit delegate must route one occlusion notification to its exact WindowId"
    );
    assert!(
        platform.contains("NS_WINDOW_OCCLUSION_STATE_VISIBLE")
            && platform.contains("msg_usize(window, \"occlusionState\")")
            && platform.contains("WindowOcclusionState::Visible")
            && platform.contains("WindowOcclusionState::Occluded")
            && platform.contains("fn os_occlusion_state(&self) -> WindowOcclusionState"),
        "macOS must classify the current NSWindow occlusionState rather than trust notification order"
    );
    let availability_check = driver
        .find("self.suspend_if_surface_unavailable(tree, platform_window)")
        .expect("driver must recheck native surface availability");
    let drive_frame_tail = &driver[availability_check..];
    let present_probe = drive_frame_tail
        .find("self.frame_scheduler.take_due_occlusion_probe(now)")
        .expect("driver must retain DXGI's optional probe path");
    let visual_request = drive_frame_tail
        .find("self.arm_visual_request(now, tree, pending_root, *reconcile_pending)")
        .expect("driver must arm visual work only after availability checks");
    assert!(
        present_probe < visual_request,
        "native occlusion must suspend before animation, layout, paint, or present"
    );
    assert!(
        driver.contains("WindowOcclusionState::Occluded")
            && driver.contains("WindowOcclusionState::Visible")
            && driver.contains("WindowOcclusionState::Unknown")
            && driver.contains("Some(SurfaceSuspendReason::Hidden | SurfaceSuspendReason::Occluded)")
            && driver.contains("this path deliberately registers no")
            && driver.contains("availability probe deadline"),
        "event-driven macOS occlusion must retain dirty state, resume only exact suspension, and register no polling deadline"
    );
}

#[test]
fn wayland_window_modes_are_owned_per_window() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let backend = read_source(root.join("src/native/backends/linux/wayland/mod.rs"));
    let window = read_source(root.join("src/native/backends/linux/wayland/window.rs"));
    let window_ops = read_source(root.join("src/native/backends/linux/wayland/window_ops.rs"));

    assert!(
        !backend.contains("pub(crate) maximized: Arc<Mutex<bool>>")
            && !backend.contains("pub(crate) fullscreen: Arc<Mutex<bool>>")
            && window_ops.contains("configured_modes: Arc<Mutex<NativeWindowModeState>>")
            && window_ops.contains("NativeWindowModeState::default()")
            && window.contains("Rc::clone(&state)"),
        "Wayland configure state must be created per window and share only that window's WindowState"
    );
}

#[test]
fn wayland_input_events_follow_surface_focus_and_keep_their_window_target() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let window = read_source(root.join("src/native/backends/linux/wayland/window.rs"));
    let window_ops = read_source(root.join("src/native/backends/linux/wayland/window_ops.rs"));
    let seat = read_source(root.join("src/native/backends/linux/wayland/seat.rs"));
    let event_loop = read_source(root.join("src/native/backends/linux/wayland/event_loop.rs"));
    let text_input = read_source(root.join("src/native/backends/linux/wayland/text_input.rs"));
    let ime_events = read_source(root.join("src/native/shared/ime_events.rs"));

    assert!(
        window.contains("self.surface_windows.clone()")
            && window_ops.contains(".register_surface(surface_id, window_id)")
            && window_ops.contains(".unregister_surface(surface_id)"),
        "each live Wayland surface must be registered to exactly one application window"
    );
    assert!(
        seat.contains(".pointer_enter(surface.as_ref().id())")
            && seat.contains(".pointer_target()")
            && seat.contains(".keyboard_enter(surface.as_ref().id())")
            && seat.contains(".keyboard_target()")
            && seat.contains("event.for_window(window_id)")
            && seat.contains("UiEvent::key_down(code, current_mods).for_window(window_id)")
            && seat.contains("UiEventType::WindowFocus")
            && seat.contains("UiEventType::WindowBlur"),
        "Wayland pointer and keyboard callbacks must inherit the matching surface focus"
    );
    assert!(
        event_loop.contains("current_target != Some(window_id)")
            && event_loop.contains("UiEvent::key_down(code, mods).for_window(window_id)")
            && event_loop.contains("UiEvent::text_input(text).for_window(window_id)"),
        "Wayland key repeats must retain and revalidate their original window target"
    );
    assert!(
        text_input.contains("window_for_surface(surface.as_ref().id())")
            && text_input.contains("zwp_text_input_v3::Event::PreeditString")
            && text_input.contains("zwp_text_input_v3::Event::Done")
            && text_input.contains("active_generation.load(Ordering::SeqCst) != generation")
            && text_input.contains("pending.apply_for_window")
            && ime_events.contains("on_marked_text_for_window")
            && ime_events.contains("on_committed_text_for_window")
            && ime_events.contains("on_unmark_text_for_window"),
        "Wayland IME batches must be accepted only for the selected surface and target one window"
    );
}

#[test]
fn macos_native_contexts_have_real_owners_and_input_events_keep_their_window_target() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let runtime = read_source(root.join("src/native/backends/macos/objc_runtime.rs"));
    let delegate = read_source(root.join("src/native/backends/macos/window_delegate.rs"));
    let text_input = read_source(root.join("src/native/backends/macos/text_input_view.rs"));
    let platform = read_source(root.join("src/native/backends/macos/platform.rs"));
    let ime_owner = read_source(root.join("src/native/shared/ime_owner.rs"));

    assert!(
        runtime.contains("class_addIvar")
            && runtime.contains("ivar_getOffset")
            && runtime.contains("install_box")
            && runtime.contains("take_box")
            && runtime.contains("call_super_dealloc")
            && delegate.contains("_uixDelegateContext")
            && delegate.contains("delegate_dealloc")
            && text_input.contains("_uixTextInputContext")
            && text_input.contains("view_dealloc")
            && !delegate.contains("context as Id")
            && !text_input.contains("objc_setAssociatedObject"),
        "macOS Rust contexts must live in raw ivars and be reclaimed exactly once from Objective-C dealloc"
    );
    assert!(
        delegate.contains("retain_delegate_for_window(window, delegate)")
            && delegate.contains("objc_setAssociatedObject(")
            && delegate.contains("delegate,")
            && platform.contains("msg_void_bool(window, \"setReleasedWhenClosed:\", NO)")
            && platform.contains("impl Drop for MacosWindowOps")
            && platform.contains("cocoa::release_object(window)"),
        "a real Objective-C delegate and Rust-owned NSWindow must bound native callback lifetimes"
    );
    assert!(
        platform.contains("fn objc_msgSend_stret()")
            && platform.contains("unsafe extern \"C\" fn(*mut CGRect, Id, Sel)")
            && delegate.contains("fn objc_msgSend_stret()")
            && delegate.contains("unsafe extern \"C\" fn(*mut CGRect, Id, Sel)"),
        "x86_64 macOS CGRect message returns must use the Objective-C stret ABI"
    );
    assert!(
        platform.contains("window_id: Option<WindowId>")
            && platform.contains("window_delegate::window_id(event_window)")
            && platform.contains("event.for_window(window_id)")
            && platform.contains("suppress_keydown_text(event.window_id)")
            && delegate.contains("UiEventType::WindowFocus")
            && delegate.contains("UiEventType::WindowBlur"),
        "macOS pointer, keyboard, focus and blur events must resolve their originating NSWindow"
    );
    assert!(
        text_input.contains("view_window_id(view) != Some(window_id)")
            && text_input.contains("on_marked_text_for_window")
            && text_input.contains("on_committed_text_for_window")
            && text_input.contains("on_unmark_text_for_window")
            && text_input.contains("session_generation")
            && ime_owner.contains("deactivate_selected")
            && ime_owner.contains("self.active.map(|session| session.target) != Some(selected)"),
        "macOS IME callbacks must carry the selected window and stale blur must not stop a new owner"
    );
}

#[test]
fn wayland_clipboard_io_is_polled_bounded_and_uses_an_input_serial() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let clipboard = read_source(root.join("src/native/backends/linux/wayland/clipboard.rs"));
    let event_loop = read_source(root.join("src/native/backends/linux/wayland/event_loop.rs"));
    let seat = read_source(root.join("src/native/backends/linux/wayland/seat.rs"));

    assert!(
        clipboard.contains("libc::O_NONBLOCK")
            && clipboard.contains("CLIPBOARD_READ_BUDGET")
            && clipboard.contains("CLIPBOARD_WRITE_BUDGET")
            && event_loop.contains("clipboard_fd")
            && event_loop.contains("NonBlockingReadStatus::Pending")
            && event_loop.contains("POLLOUT")
            && event_loop.contains("write_clipboard_pipe")
            && !event_loop.contains("read_to_end")
            && !clipboard.contains("write_all"),
        "Wayland clipboard reads and writes must survive backpressure and do bounded work from the main poll"
    );
    assert!(
        seat.contains(".record(serial)")
            && clipboard.contains(".latest()")
            && clipboard.contains("dd.set_selection(Some(&source), serial)")
            && !clipboard.contains("dd.set_selection(Some(&source), 0)"),
        "Wayland set_selection must use a serial captured from pointer or keyboard input"
    );
}

#[test]
fn vulkan_readback_cannot_report_an_empty_success() {
    let source = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/native/graphics/vulkan/platform/context.rs"),
    )
    .expect("read Vulkan context");

    assert!(
        source.contains("cpu_shadow")
            && source.contains("no uploaded frame to read back (cpu_shadow empty)"),
        "Vulkan must fail typed when no staged frame exists, not return an empty success buffer"
    );
    assert!(
        source.contains("hydrate_cpu_shadow_from_staging"),
        "Vulkan must lazy-hydrate CPU shadow on readback instead of copying every present"
    );
    assert!(
        !source.contains("VulkanContext: native readback is not supported"),
        "Vulkan PixelUpload now provides CPU-shadow readback for destination-dependent IR"
    );
}

#[test]
fn metal_pixel_upload_readback_cannot_report_an_empty_success() {
    let source = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/native/graphics/metal/platform/context.rs"),
    )
    .expect("read Metal PixelUpload context");

    assert!(
        source.contains("MetalPixelUploadContext: native readback is not supported"),
        "Metal PixelUpload must return a typed readback failure instead of an empty pixel buffer"
    );
}

#[test]
fn d3d11_checked_offscreen_destroy_cannot_ignore_swapchain_restore_failure() {
    let source = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/native/graphics/d3d11/platform/context.rs"),
    )
    .expect("read D3D11 context");

    assert!(
        source.contains("fn try_destroy_offscreen_target(&mut self, id: OffscreenTargetId) -> Result<(), Error>"),
        "D3D11 checked offscreen destroy must not fall back to the void hook"
    );
    assert!(
        source.contains("self.bind_swapchain_target()?;"),
        "D3D11 checked offscreen destroy must propagate the swapchain restore failure"
    );
}

#[test]
fn replace_upload_is_declared_on_destination_dependent_backends() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/native/graphics");
    let d3d12 = fs::read_to_string(src.join("d3d12/platform/context.rs")).expect("D3D12");
    let wgl = fs::read_to_string(src.join("opengl/platform/wgl.rs")).expect("WGL");
    let egl = fs::read_to_string(src.join("opengl/platform/egl.rs")).expect("EGL");
    let vulkan = fs::read_to_string(src.join("vulkan/platform/context.rs")).expect("Vulkan");

    for (name, source) in [
        ("D3D12", d3d12.as_str()),
        ("WGL", wgl.as_str()),
        ("EGL", egl.as_str()),
        ("Vulkan", vulkan.as_str()),
    ] {
        assert!(
            source.contains("fn upload_surface_pixels("),
            "{name} must declare upload_surface_pixels for Additive/Scroll replace upload"
        );
    }
}

#[test]
fn pixel_upload_contexts_cannot_report_swapchain_present_success() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/native/graphics");
    let vulkan =
        fs::read_to_string(src.join("vulkan/platform/context.rs")).expect("read Vulkan context");
    let metal = fs::read_to_string(src.join("metal/platform/context.rs"))
        .expect("read Metal PixelUpload context");

    assert!(
        vulkan.contains(
            "VulkanContext: swapchain present is not supported for the PixelUpload recipe"
        ),
        "Vulkan PixelUpload must reject the legacy swapchain present hook"
    );
    assert!(
        metal.contains("MetalPixelUploadContext: swapchain present requires native Metal raster"),
        "Metal PixelUpload must reject the legacy swapchain present hook"
    );
}
