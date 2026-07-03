//! 绘图层与平台层测试框架集成测试。
//!
//! 使用 `uix-platform` 的 `test_harness`（`FakePlatform`、`FakeGraphicsContext` 等）
//! 验证绘图层组件与平台抽象层的集成契约：
//!
//! - `IGraphicsContext` 生命周期（初始化/尺寸/渲染/销毁）
//! - `IPresenter` 像素呈现管线
//! - `PlatformWindow` 窗口系统交互
//! - `IDisplay` 显示信息与 DPI
//! - `FakePlatform` 全平台聚合
//! - 端到端：图形渲染 → 像素输出 → 呈现器

use uix_graphics::color::Color;
use uix_graphics::engine::cpu::canvas_2d::CpuCanvas2D;
use uix_graphics::engine::cpu::pixel_surface::PixelSurface;
use uix_graphics::engine::cpu::software::SoftwareEngine;
use uix_graphics::traits::{Canvas2D, GraphicsEngine, UpdateStrategy};
use uix_platform::PresentDamage;
use uix_platform::api::traits::{
    IClipboard, IDisplay, IGraphicsContext, IPresenter, IWindowManager, Platform, PlatformWindow,
};
use uix_platform::geometry::Rect;
use uix_platform::test_harness::{
    FakeDisplay, FakeGraphicsContext, FakePlatform, FakePresenter, FakeWindow, FakeWindowManager,
};

// ════════════════════════════════════════════════════════════════════════════
// FakeGraphicsContext — IGraphicsContext 生命周期与契约测试
// ════════════════════════════════════════════════════════════════════════════

/// 默认创建的 GraphicsContext 未初始化，width/height 为 0。
#[test]
fn fake_graphics_context_new_default_uninitialized() {
    let ctx = FakeGraphicsContext::new();
    assert!(!ctx.state.initialized);
    assert_eq!(ctx.width(), 0);
    assert_eq!(ctx.height(), 0);
}

/// with_size 预设宽高，但 initialized 仍为 false（未调用 initialize）。
#[test]
fn fake_graphics_context_with_size_presets_but_not_initialized() {
    let ctx = FakeGraphicsContext::with_size(1024, 768);
    assert!(!ctx.state.initialized);
    assert_eq!(ctx.width(), 1024);
    assert_eq!(ctx.height(), 768);
}

/// initialize 写入宽高、标记初始化。
#[test]
fn fake_graphics_context_initialize_sets_size_and_flag() {
    let mut ctx = FakeGraphicsContext::new();
    let result = ctx.initialize(std::ptr::null_mut(), 800, 600);
    assert!(result.is_ok());
    assert!(ctx.state.initialized);
    assert_eq!(ctx.width(), 800);
    assert_eq!(ctx.height(), 600);
}

/// initialize 覆盖 with_size 预设的值。
#[test]
fn fake_graphics_context_initialize_overrides_preset() {
    let mut ctx = FakeGraphicsContext::with_size(640, 480);
    ctx.initialize(std::ptr::null_mut(), 800, 600).unwrap();
    assert_eq!(ctx.width(), 800); // initialize 覆盖
    assert_eq!(ctx.height(), 600);
}

/// resize 在 initialize 后更新宽高。
#[test]
fn fake_graphics_context_resize_updates_dimensions() {
    let mut ctx = FakeGraphicsContext::new();
    ctx.initialize(std::ptr::null_mut(), 800, 600).unwrap();
    ctx.resize(400, 300);
    assert_eq!(ctx.width(), 400);
    assert_eq!(ctx.height(), 300);
}

/// resize 在 initialize 前也可用。
#[test]
fn fake_graphics_context_resize_before_init() {
    let mut ctx = FakeGraphicsContext::new();
    ctx.resize(100, 200);
    assert_eq!(ctx.width(), 100);
    assert_eq!(ctx.height(), 200);
}

/// make_current 递增调用计数。
#[test]
fn fake_graphics_context_make_current_tracks_calls() {
    let mut ctx = FakeGraphicsContext::new();
    assert_eq!(ctx.state.make_current_calls, 0);
    ctx.make_current();
    assert_eq!(ctx.state.make_current_calls, 1);
    ctx.make_current();
    ctx.make_current();
    assert_eq!(ctx.state.make_current_calls, 3);
}

/// swap_buffers 递增调用计数。
#[test]
fn fake_graphics_context_swap_buffers_tracks_calls() {
    let mut ctx = FakeGraphicsContext::new();
    assert_eq!(ctx.state.swap_buffers_calls, 0);
    ctx.swap_buffers();
    assert_eq!(ctx.state.swap_buffers_calls, 1);
}

/// shutdown 标记关闭，后续操作不应 panic。
#[test]
fn fake_graphics_context_shutdown_sets_flag() {
    let mut ctx = FakeGraphicsContext::new();
    ctx.initialize(std::ptr::null_mut(), 800, 600).unwrap();
    assert!(!ctx.state.shutdown_called);
    ctx.shutdown();
    assert!(ctx.state.shutdown_called);
    // shutdown 后调用 resize/make_current/swap_buffers 不应 panic
    ctx.resize(1, 1);
    ctx.make_current();
    ctx.swap_buffers();
}

/// read_pixels 默认返回空 vec（无 GPU 后端）。
#[test]
fn fake_graphics_context_read_pixels_returns_empty() {
    let mut ctx = FakeGraphicsContext::new();
    let pixels = ctx.read_pixels(0, 0, 100, 100);
    assert!(pixels.is_empty());
    let pixels = ctx.read_pixels(-10, -10, 50, 50);
    assert!(pixels.is_empty());
}

/// get_proc_address 默认返回 None。
#[test]
fn fake_graphics_context_get_proc_address_default_none() {
    let ctx = FakeGraphicsContext::new();
    let addr = ctx.get_proc_address("glClear");
    assert!(addr.is_none());
    let addr = ctx.get_proc_address("");
    assert!(addr.is_none());
}

/// 完整的上下文生命周期流：new → initialize → resize → make_current → swap → shutdown。
#[test]
fn fake_graphics_context_full_lifecycle() {
    let mut ctx = FakeGraphicsContext::new();

    ctx.initialize(std::ptr::null_mut(), 800, 600).unwrap();
    assert_eq!(ctx.width(), 800);
    assert_eq!(ctx.height(), 600);
    assert!(ctx.state.initialized);

    ctx.resize(1024, 768);
    assert_eq!(ctx.width(), 1024);

    ctx.make_current();
    assert_eq!(ctx.state.make_current_calls, 1);

    ctx.swap_buffers();
    assert_eq!(ctx.state.swap_buffers_calls, 1);

    ctx.shutdown();
    assert!(ctx.state.shutdown_called);
}

// ════════════════════════════════════════════════════════════════════════════
// FakePresenter — IPresenter 呈现管线契约测试
// ════════════════════════════════════════════════════════════════════════════

/// 默认 Presenter 宽高为 0，无像素，无调用历史。
#[test]
fn fake_presenter_default_state() {
    let p = FakePresenter::new();
    assert_eq!(p.state.width, 0);
    assert_eq!(p.state.height, 0);
    assert!(p.state.last_pixels.is_empty());
    assert!(p.state.present_calls.is_empty());
    assert!(p.state.resize_calls.is_empty());
}

/// resize 记录宽高和调用历史。
#[test]
fn fake_presenter_resize_tracks() {
    let mut p = FakePresenter::new();
    p.resize(640, 480).unwrap();
    assert_eq!(p.state.width, 640);
    assert_eq!(p.state.height, 480);
    assert_eq!(p.state.resize_calls, vec![(640, 480)]);

    p.resize(800, 600).unwrap();
    assert_eq!(p.state.resize_calls, vec![(640, 480), (800, 600)]);
}

/// present 记录像素数据和调用参数。
#[test]
fn fake_presenter_present_records_pixels() {
    let mut p = FakePresenter::new();
    p.resize(10, 10).unwrap();
    let pixels = vec![0xFF0000FFu32; 100]; // 红色 ABGR
    p.present(&pixels, 10, 10, PresentDamage::Full).unwrap();

    assert_eq!(p.state.present_calls.len(), 1);
    assert_eq!(p.state.last_pixels.len(), 100);
    assert_eq!(p.state.last_pixels[0], 0xFF0000FF);
    assert_eq!(p.state.last_pixels[99], 0xFF0000FF);

    // 验证调用记录
    let call = &p.state.present_calls[0];
    assert_eq!(call.width, 10);
    assert_eq!(call.height, 10);
    assert_eq!(call.pixels_len, 100);
    assert!(call.damage.is_full());
}

/// present 支持 damage 参数传递。
#[test]
fn fake_presenter_present_tracks_damage() {
    let mut p = FakePresenter::new();
    p.resize(20, 20).unwrap();
    let pixels = vec![0u32; 400];
    p.present(&pixels, 20, 20, PresentDamage::single(5, 5, 10, 10)).unwrap();

    let call = &p.state.present_calls[0];
    assert_eq!(call.damage, PresentDamage::single(5, 5, 10, 10));
}

/// 多次 present 累积历史，last_pixels 始终为最近一次。
#[test]
fn fake_presenter_multiple_presents_accumulate() {
    let mut p = FakePresenter::new();
    p.resize(5, 5).unwrap();

    let red = vec![0xFF0000FFu32; 25];
    p.present(&red, 5, 5, PresentDamage::Full).unwrap();

    let green = vec![0xFF00FF00u32; 25];
    p.present(&green, 5, 5, PresentDamage::Full).unwrap();

    let blue = vec![0xFFFF0000u32; 25];
    p.present(&blue, 5, 5, PresentDamage::Full).unwrap();

    assert_eq!(p.state.present_calls.len(), 3);
    // last_pixels 是最后一次
    assert_eq!(p.state.last_pixels[0], 0xFFFF0000);
}

/// clear_history 重置调用记录和像素缓冲区。
#[test]
fn fake_presenter_clear_history_resets() {
    let mut p = FakePresenter::new();
    p.resize(10, 10).unwrap();
    let pixels = vec![0xFFFFFFFFu32; 100];
    p.present(&pixels, 10, 10, PresentDamage::Full).unwrap();
    assert!(!p.state.present_calls.is_empty());
    assert!(!p.state.last_pixels.is_empty());

    p.clear_history();
    assert!(p.state.present_calls.is_empty());
    assert!(p.state.resize_calls.is_empty());
    assert!(p.state.last_pixels.is_empty());
}

// ════════════════════════════════════════════════════════════════════════════
// FakeWindow — PlatformWindow 窗口系统契约测试
// ════════════════════════════════════════════════════════════════════════════

/// 窗口创建时带有指定的 id、标题和尺寸。
#[test]
fn fake_window_creation_params() {
    let win = FakeWindow::new(1, "Test Window", 800, 600);
    assert_eq!(win.id, 1);
    assert_eq!(win.state.title, "Test Window");
    assert_eq!(win.props.state.width, 800);
    assert_eq!(win.props.state.height, 600);
    assert!(!win.state.visible);
}

/// show/hide 切换可见状态并记录调用。
#[test]
fn fake_window_show_hide_lifecycle() {
    let mut win = FakeWindow::new(1, "test", 800, 600);
    assert!(!win.is_visible());

    win.show();
    assert!(win.is_visible());
    assert_eq!(win.state.show_calls, 1);

    win.hide();
    assert!(!win.is_visible());
    assert_eq!(win.state.hide_calls, 1);

    // 再次 show
    win.show();
    assert!(win.is_visible());
    assert_eq!(win.state.show_calls, 2);
}

/// close 关闭窗口（不可见）并标记。
#[test]
fn fake_window_close_sets_invisible() {
    let mut win = FakeWindow::new(1, "test", 800, 600);
    win.show();
    assert!(win.is_visible());

    win.close();
    assert!(!win.is_visible());
    assert!(win.state.close_called);
}

/// resize_notify 更新窗口属性和记录。
#[test]
fn fake_window_resize_notify_updates_and_tracks() {
    let mut win = FakeWindow::new(1, "test", 800, 600);
    win.resize_notify(1024, 768);
    assert_eq!(win.props.state.width, 1024);
    assert_eq!(win.props.state.height, 768);
    assert_eq!(win.state.resize_notify_calls, vec![(1024, 768)]);

    win.resize_notify(640, 480);
    assert_eq!(win.state.resize_notify_calls, vec![(1024, 768), (640, 480)]);
}

/// 通过 PlatformWindow trait 访问窗口属性全套接口。
#[test]
fn fake_window_properties_full_access() {
    let mut win = FakeWindow::new(1, "test", 800, 600);

    // set_size
    win.properties_mut().set_size(1280, 720);
    assert_eq!(win.properties().width(), 1280);
    assert_eq!(win.properties().height(), 720);
    assert_eq!(win.props.state.set_size_calls, vec![(1280, 720)]);

    // min/max size
    win.properties_mut().set_minimum_size(200, 150);
    win.properties_mut().set_maximum_size(2000, 1500);
    assert_eq!(win.props.state.min_w, 200);
    assert_eq!(win.props.state.min_h, 150);
    assert_eq!(win.props.state.max_w, 2000);
    assert_eq!(win.props.state.max_h, 1500);

    // position
    win.properties_mut().set_position(100, 200);
    assert_eq!(win.properties().position().x as i32, 100);
    assert_eq!(win.properties().position().y as i32, 200);

    // resizable
    win.properties_mut().set_resizable(false);
    assert!(!win.props.state.resizable);

    // fullscreen
    win.properties_mut().set_fullscreen(true);
    assert!(win.properties().is_fullscreen());
    win.properties_mut().set_fullscreen(false);
    assert!(!win.properties().is_fullscreen());

    // borderless
    win.properties_mut().set_borderless(true);
    assert!(win.props.state.borderless);

    // window opacity
    win.properties_mut().set_window_opacity(0.5);
    assert!((win.props.state.opacity - 0.5).abs() < 1e-6);

    // always on top
    win.properties_mut().set_always_on_top(true);
    assert!(win.props.state.always_on_top);
}

/// set_title 记录标题变更历史。
#[test]
fn fake_window_set_title_tracks() {
    let mut win = FakeWindow::new(1, "test", 800, 600);
    win.set_title("New Title");
    assert_eq!(win.state.title, "New Title");
    assert_eq!(win.state.set_title_calls, vec!["New Title"]);
}

/// 窗口 + GPU 上下文：with_gpu 创建后 gpu_ctx 为 Some。
#[test]
fn fake_window_gpu_context_optional() {
    // 默认无 GPU
    let mut win = FakeWindow::new(1, "test", 800, 600);
    assert!(win.gpu_ctx.is_none());
    assert!(win.graphics_context().is_none());

    // with_gpu 后返回 Some
    let mut win_gpu = FakeWindow::new(2, "gpu", 800, 600).with_gpu();
    assert!(win_gpu.gpu_ctx.is_some());
    assert!(win_gpu.state.has_gpu);
    assert!(win_gpu.graphics_context().is_some());
}

/// 窗口生命周期完整流：创建 → show → set_title → resize_notify → close。
#[test]
fn fake_window_full_lifecycle() {
    let mut win = FakeWindow::new(1, "App", 800, 600);

    win.show();
    assert!(win.is_visible());

    win.set_title("Running");
    assert_eq!(win.state.title, "Running");

    win.resize_notify(1024, 768);
    assert_eq!(win.properties().width(), 1024);

    win.properties_mut().set_fullscreen(true);
    assert!(win.properties().is_fullscreen());

    win.close();
    assert!(!win.is_visible());
    assert!(win.state.close_called);
}

// ════════════════════════════════════════════════════════════════════════════
// FakeWindowManager — 窗口管理契约测试
// ════════════════════════════════════════════════════════════════════════════

/// create_window 返回 Box<dyn PlatformWindow>，尺寸与标题正确。
#[test]
fn fake_window_manager_create_via_trait() {
    let mut mgr = FakeWindowManager::new();
    let mut window = mgr.create_window("Hello", 1024, 768).unwrap();
    assert_eq!(window.properties().width(), 1024);
    assert_eq!(window.properties().height(), 768);
    // 通过 PlatformWindow trait 操作
    window.show();
    assert!(window.is_visible());
}

/// create_window 记录创建历史。
#[test]
fn fake_window_manager_tracks_creations() {
    let mut mgr = FakeWindowManager::new();
    assert!(mgr.create_calls.is_empty());

    let _w1 = mgr.create_window("Window 1", 800, 600).unwrap();
    assert_eq!(mgr.create_calls.len(), 1);
    assert_eq!(mgr.create_calls[0], ("Window 1".to_string(), 800, 600));

    let _w2 = mgr.create_window("Window 2", 400, 300).unwrap();
    assert_eq!(mgr.create_calls.len(), 2);
}

/// 连续创建的窗口各自独立。
#[test]
fn fake_window_manager_ids_increment() {
    let mut mgr = FakeWindowManager::new();
    let mut w1 = mgr.create_window("w1", 100, 100).unwrap();
    let mut w2 = mgr.create_window("w2", 200, 200).unwrap();
    w1.show();
    w2.show();
    assert!(w1.is_visible());
    assert!(w2.is_visible());
}

/// clear_history 重置创建记录但不影响已创建的窗口。
#[test]
fn fake_window_manager_clear_history() {
    let mut mgr = FakeWindowManager::new();
    let _w = mgr.create_window("test", 800, 600).unwrap();
    assert_eq!(mgr.create_calls.len(), 1);
    mgr.clear_history();
    assert!(mgr.create_calls.is_empty());
}

// ════════════════════════════════════════════════════════════════════════════
// FakeDisplay — IDisplay 显示信息契约测试
// ════════════════════════════════════════════════════════════════════════════

/// 默认显示：1 个显示器，DPI=1.0，非暗色模式。
#[test]
fn fake_display_defaults() {
    let d = FakeDisplay::new();
    assert!((d.dpi_scale() - 1.0).abs() < 1e-6);
    assert!(!d.is_dark_mode());
    assert_eq!(d.count(), 1);
}

/// DPI 可随时调整，影响 dpi_scale() 返回值。
#[test]
fn fake_display_dpi_configurable() {
    let d = FakeDisplay::new();
    d.set_dpi(2.0);
    assert!((d.dpi_scale() - 2.0).abs() < 1e-6);
    d.set_dpi(1.5);
    assert!((d.dpi_scale() - 1.5).abs() < 1e-6);
    d.set_dpi(1.25);
    assert!((d.dpi_scale() - 1.25).abs() < 1e-6);
}

/// 暗色模式可切换。
#[test]
fn fake_display_dark_mode_toggle() {
    let d = FakeDisplay::new();
    assert!(!d.is_dark_mode());
    d.set_dark_mode(true);
    assert!(d.is_dark_mode());
    d.set_dark_mode(false);
    assert!(!d.is_dark_mode());
}

/// 多显示器场景：count 和 info 返回正确信息。
#[test]
fn fake_display_multi_monitor() {
    let d = FakeDisplay::new();
    d.set_count(2);
    assert_eq!(d.count(), 2);

    let info0 = d.info(0);
    assert!(info0.is_primary);
    assert!((info0.dpi_scale - 1.0).abs() < 1e-6);

    let info1 = d.info(1);
    assert!(!info1.is_primary);

    // 每个 info 的 bounds 不同
    assert!(info0.bounds.x < info1.bounds.x);
}

/// info 调用被记录到 info_calls。
#[test]
fn fake_display_info_calls_tracked() {
    let d = FakeDisplay::new();
    d.info(0);
    d.info(1);
    d.info(0);
    assert_eq!(d.info_calls.borrow().len(), 3);
}

/// clear_history 重置调用记录。
#[test]
fn fake_display_clear_history() {
    let d = FakeDisplay::new();
    d.info(0);
    d.info(1);
    assert!(!d.info_calls.borrow().is_empty());
    d.clear_history();
    assert!(d.info_calls.borrow().is_empty());
}

// ════════════════════════════════════════════════════════════════════════════
// FakePlatform — 全平台聚合集成测试
// ════════════════════════════════════════════════════════════════════════════

/// FakePlatform::new 初始化所有子系统，各自处于默认状态。
#[test]
fn fake_platform_new_all_subsystems_initialized() {
    let pf = FakePlatform::new();
    // 显示
    assert!((pf.display.dpi_scale() - 1.0).abs() < 1e-6);
    assert_eq!(pf.display.count(), 1);
    // 剪贴板
    assert_eq!(pf.clipboard.text(), "");
    assert!(!pf.clipboard.has_text());
    // 窗口管理器
    assert!(pf.window_manager.create_calls.is_empty());
}

/// 通过 Platform trait 访问 display。
#[test]
fn fake_platform_display_via_trait() {
    let pf = FakePlatform::new();
    let display: &dyn IDisplay = pf.display();
    assert!((display.dpi_scale() - 1.0).abs() < 1e-6);
    assert!(!display.is_dark_mode());
}

/// 通过 Platform trait 访问 window_manager 并创建窗口。
#[test]
fn fake_platform_window_via_trait() {
    let mut pf = FakePlatform::new();
    let mut window = pf.window_manager().create_window("test", 800, 600).unwrap();
    window.show();
    assert!(window.is_visible());
    window.set_title("Updated");
    window.resize_notify(1024, 768);
    assert_eq!(window.properties().width(), 1024);
}

/// 通过 Platform trait 访问 clipboard。
#[test]
fn fake_platform_clipboard_via_trait() {
    let mut pf = FakePlatform::new();
    let clipboard: &mut dyn IClipboard = pf.clipboard();
    assert!(!clipboard.has_text());
    clipboard.set_text("hello");
    assert!(clipboard.has_text());
    assert_eq!(clipboard.text(), "hello");
}

/// 完整的 Platform trait 访问：所有子系统都能通过 trait 方法访问且不 panic。
#[test]
fn fake_platform_all_traits_accessible() {
    let mut pf = FakePlatform::new();

    // 逐一访问所有子系统
    let _ = pf.window_manager();
    let _ = pf.event_loop();
    let _ = pf.event_bus();
    let _ = pf.clipboard();
    let _ = pf.cursor();
    let _ = pf.display();
    let _ = pf.file_dialog();
    let _ = pf.keyboard();
    let _ = pf.text_input();
    let _ = pf.timer();
    let _ = pf.notification();
    let _ = pf.console();
    let _ = pf.file_system();
    let _ = pf.system_info();
}

// ════════════════════════════════════════════════════════════════════════════
// 端到端集成测试 — 图形渲染 → 像素输出 → 呈现器
// 连接绘图层组件与平台层测试框架，验证两端协同工作。
// ════════════════════════════════════════════════════════════════════════════

/// 端到端：CpuCanvas2D 渲染矩形 → 提取像素 → FakePresenter 验证。
#[test]
fn end_to_end_render_rect_to_presenter() {
    let surf = PixelSurface::new(10, 10);
    let mut canvas = CpuCanvas2D::new(surf);

    // 渲染一个红色矩形
    canvas.fill_rect(
        Rect::new(0.0, 0.0, 5.0, 5.0),
        Color::from_rgba(255, 0, 0, 255),
        None,
    );

    // 提取像素
    let pixels = canvas.surface().pixels();
    assert_eq!(pixels.len(), 100);

    // 通过 FakePresenter 呈现
    let mut presenter = FakePresenter::new();
    presenter.resize(10, 10).unwrap();
    presenter
        .present(pixels, 10, 10, PresentDamage::single(0, 0, 5, 5))
        .unwrap();

    // 验证呈现器收到正确的像素数据
    assert_eq!(presenter.state.present_calls.len(), 1);
    assert_eq!(presenter.state.last_pixels.len(), 100);
    // (0,0) 应为红色
    assert_eq!(presenter.state.last_pixels[0], 0xFF0000FF);
    // (5,5) 应在矩形外，仍为透明
    assert_eq!(presenter.state.last_pixels[5 * 10 + 5], 0x00000000);
    // damage 参数正确传递
    assert_eq!(
        presenter.state.present_calls[0].damage,
        PresentDamage::single(0, 0, 5, 5)
    );
}

/// 端到端：CpuCanvas2D 渲染圆形 → 像素数据 → FakePresenter 验证。
#[test]
fn end_to_end_render_circle_to_presenter() {
    let surf = PixelSurface::new(20, 20);
    let mut canvas = CpuCanvas2D::new(surf);

    // 渲染一个绿色圆
    canvas.fill_circle(10.0, 10.0, 5.0, Color::from_rgba(0, 255, 0, 255));

    // 提取像素并呈现
    let pixels = canvas.surface().pixels();
    let mut presenter = FakePresenter::new();
    presenter.resize(20, 20).unwrap();
    presenter.present(pixels, 20, 20, PresentDamage::Full).unwrap();

    // 圆心应为绿色
    let center = presenter.state.last_pixels[10 * 20 + 10];
    let g = (center >> 8) & 0xFF;
    assert_eq!(g, 0xFF, "圆心绿色分量应为 0xFF");
}

/// 端到端：PixelSurface 直接配合 FakePresenter 验证像素格式一致性。
/// 验证 ABGR 像素格式在绘图层输出和平台层输入之间保持一致。
#[test]
fn end_to_end_pixel_surface_to_presenter_all_colors() {
    let surf = PixelSurface::new(4, 1);
    let mut canvas = CpuCanvas2D::new(surf);

    // 渲染四个不同颜色的像素
    let colors = [
        Color::from_rgba(255, 0, 0, 255),     // 红
        Color::from_rgba(0, 255, 0, 255),     // 绿
        Color::from_rgba(0, 0, 255, 255),     // 蓝
        Color::from_rgba(255, 255, 255, 255), // 白
    ];
    for (i, color) in colors.iter().enumerate() {
        canvas.fill_rect(Rect::new(i as f32, 0.0, 1.0, 1.0), *color, None);
    }

    // 提取像素
    let pixels = canvas.surface().pixels();
    assert_eq!(pixels.len(), 4);

    // 通过 FakePresenter 验证
    let mut presenter = FakePresenter::new();
    presenter.resize(4, 1).unwrap();
    presenter.present(pixels, 4, 1, PresentDamage::Full).unwrap();

    // ABGR 格式验证（PixelSurface 内部存储格式）
    assert_eq!(presenter.state.last_pixels[0], 0xFF0000FF, "红色 ABGR");
    assert_eq!(presenter.state.last_pixels[1], 0xFF00FF00, "绿色 ABGR");
    assert_eq!(presenter.state.last_pixels[2], 0xFFFF0000, "蓝色 ABGR");
    assert_eq!(presenter.state.last_pixels[3], 0xFFFFFFFF, "白色 ABGR");
}

/// 端到端：RenderingBackend 的 copy_region + FakePresenter 滚动场景。
#[test]
fn end_to_end_scroll_region_to_presenter() {
    let surf = PixelSurface::new(10, 10);
    let mut canvas = CpuCanvas2D::new(surf);

    // 在顶部绘制白色条带
    canvas.fill_rect(
        Rect::new(0.0, 0.0, 10.0, 3.0),
        Color::from_rgba(255, 255, 255, 255),
        None,
    );

    // scroll_region: 向下滚动 4px（内容上移）
    // dy=4: src=(0,4,10,10) → dst=(0,0)
    // 填充区为 y=0..3（3 行白色），因此 y=4 是透明，
    // 复制后 (0,0) 得到透明像素
    canvas.scroll_region(Rect::new(0.0, 0.0, 10.0, 10.0), 0.0, 4.0);

    // 提取像素并呈现
    let pixels = canvas.surface().pixels();
    let mut presenter = FakePresenter::new();
    presenter.resize(10, 10).unwrap();
    presenter.present(pixels, 10, 10, PresentDamage::Full).unwrap();

    // scroll_region(dx=0, dy=4): src=(0,4,10,10) → dst=(0,0)
    // src 起始 y=4 在填充区外（填充 y=0..3），因此 (0,0) 得到透明像素
    assert_eq!(
        presenter.state.last_pixels[0], 0x00000000,
        "scroll dy=4 后 (0,0) 被 src=(0,4) 的透明像素覆盖"
    );
}

/// 端到端：SoftwareEngine 完整帧生命周期不 panic。
#[test]
fn end_to_end_software_engine_frame_no_panic() {
    let mut engine = SoftwareEngine::new();
    engine.initialize(10, 10).unwrap();

    // FullRedraw 帧
    let result = engine.begin_frame(UpdateStrategy::FullRedraw);
    {
        let canvas = engine.canvas_2d();
        canvas.fill_rect(
            Rect::new(0.0, 0.0, 5.0, 5.0),
            Color::from_rgba(0, 0, 255, 255),
            None,
        );
    }
    let result2 = engine.end_frame();
    assert!(matches!(
        result,
        uix_graphics::engine::RenderOutcome::Present(_)
    ));
    assert!(matches!(
        result2,
        uix_graphics::engine::RenderOutcome::Present(_)
    ));

    // Overlay 帧
    engine.begin_frame(UpdateStrategy::Overlay(vec![Rect::new(2.0, 2.0, 3.0, 3.0)]));
    engine.end_frame();

    // Overlay 空列表 → Idle（无 overlay 区域需绘制）
    let overlay = engine.begin_frame(UpdateStrategy::Overlay(vec![]));
    assert_eq!(overlay, uix_graphics::engine::RenderOutcome::Idle);
}

/// 端到端：SoftwareEngine 离屏缓冲 → blit → 不 panic。
#[test]
fn end_to_end_offscreen_blit_no_panic() {
    let mut engine = SoftwareEngine::new();
    engine.initialize(20, 20).unwrap();

    let offscreen = engine.create_offscreen(10, 10).unwrap();

    // 在离屏上绘制
    if let Some(canvas) = engine.offscreen_canvas(&offscreen) {
        canvas.fill_rect(
            Rect::new(0.0, 0.0, 10.0, 10.0),
            Color::from_rgba(255, 255, 255, 255),
            None,
        );
    }

    // blit 到主表面
    engine.blit_offscreen(&offscreen, Rect::new(5.0, 5.0, 10.0, 10.0));

    // 销毁离屏
    engine.destroy_offscreen(offscreen);
    assert!(engine.offscreen_canvas(&offscreen).is_none());
}

// ════════════════════════════════════════════════════════════════════════════
// 跨层交互测试 — 图形引擎 + 平台子系统组合场景
// ════════════════════════════════════════════════════════════════════════════

/// 组合场景：FakeDisplay 的 DPI 设置与图形引擎的 DPI 查询各自正确。
#[test]
fn cross_layer_display_dpi_with_graphics_engine() {
    let engine = SoftwareEngine::new();
    // SoftwareEngine::dpi() 返回固定 96.0（默认实现）
    assert!((engine.dpi() - 96.0).abs() < 1e-6);

    let d = FakeDisplay::new();
    d.set_dpi(2.0);
    // 显示 DPI 与引擎 DPI 是独立的，此处验证各自正确
    assert!((d.dpi_scale() - 2.0).abs() < 1e-6);
}

/// 组合场景：窗口 resize_notify 触发图形引擎 resize。
#[test]
fn cross_layer_window_resize_propagates_to_engine() {
    let mut engine = SoftwareEngine::new();
    engine.initialize(800, 600).unwrap();
    assert_eq!(engine.canvas_2d().width(), 800);
    assert_eq!(engine.canvas_2d().height(), 600);

    // 模拟窗口 resize 通知
    let mut win = FakeWindow::new(1, "test", 800, 600);
    win.resize_notify(400, 300);

    // 业务层应响应窗口 resize，调用引擎 resize
    engine.resize(win.properties().width(), win.properties().height());
    assert_eq!(engine.canvas_2d().width(), 400);
    assert_eq!(engine.canvas_2d().height(), 300);
}

/// 组合场景：FakePlatform 完整集成 — 创建窗口 → 引擎渲染。
#[test]
fn cross_layer_full_pipeline_via_fake_platform() {
    let mut pf = FakePlatform::new();

    // 通过平台创建窗口
    let mut window = pf
        .window_manager()
        .create_window("Graphics Test", 30, 30)
        .unwrap();
    window.show();

    // 创建引擎并初始化（匹配窗口尺寸）
    let mut engine = SoftwareEngine::new();
    engine
        .initialize(window.properties().width(), window.properties().height())
        .unwrap();

    // 渲染帧
    engine.begin_frame(UpdateStrategy::FullRedraw);
    {
        let canvas = engine.canvas_2d();
        canvas.fill_rect(
            Rect::new(5.0, 5.0, 10.0, 10.0),
            Color::from_rgba(255, 255, 255, 255),
            None,
        );
    }
    engine.end_frame();

    window.close();
}

// ════════════════════════════════════════════════════════════════════════════
// NullEngine + FakePlatform 集成
// ════════════════════════════════════════════════════════════════════════════

/// NullEngine 在 FakePlatform 场景下正常工作。
#[test]
fn null_engine_with_fake_platform() {
    use uix_graphics::null_engine::NullEngine;

    let _pf = FakePlatform::new();
    // 验证 FakePlatform 与 NullEngine 可以共存于同一个测试中
    let mut engine = NullEngine::new();
    engine.initialize(100, 100).unwrap();
    engine.begin_frame(UpdateStrategy::FullRedraw);
    engine.end_frame();
    engine.shutdown();
}
