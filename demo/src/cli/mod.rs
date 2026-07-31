//! CLI 演示 — 按功能域展示 `core` / `native` / `draw` / `ui` / `app` / `data` 能力。
//!
//! 运行：`cargo run --bin uix-demo -- --cli`

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use uix::data::SettingsService;
use uix::diagnostics::{Diagnostics, RecoveryAction, RecoveryOutcome};
use uix::draw::target::RenderTarget;
use uix::prelude::*;

#[allow(dead_code)]
pub fn init_logging() {
    tracing::info!("演示日志初始化完成");
}

pub fn demo_core_types() {
    println!("\n╔══ core：几何类型 ═══╗");
    let pt = Point::new(10.0, 20.0);
    println!("  Point(10,20) → ({},{})", pt.x, pt.y);
    let sz = Size::new(100.0, 50.0);
    println!("  Size(100,50) → w={}, h={}", sz.w, sz.h);
    let r = Rect::new(0.0, 0.0, 100.0, 100.0);
    println!(
        "  Rect(0,0,100,100) contains(50,50): {}, contains(150,50): {}",
        r.contains(Point::new(50.0, 50.0)),
        r.contains(Point::new(150.0, 50.0))
    );
    println!(
        "  Intersection: {:?}",
        Rect::new(0.0, 0.0, 100.0, 100.0).intersect(&Rect::new(50.0, 50.0, 100.0, 100.0))
    );
    let ins = EdgeInsets::uniform(8.0);
    println!(
        "  EdgeInsets uniform(8) → L:{} T:{} R:{} B:{} h:{} v:{}",
        ins.left,
        ins.top,
        ins.right,
        ins.bottom,
        ins.horizontal(),
        ins.vertical()
    );
    let c = colors::PRIMARY;
    println!("  Primary: {} (RGBA:{:08X})", c, c.to_rgba());
}

pub fn demo_errors() {
    println!("\n╔══ core：错误处理 ═══╗");
    println!("  Ok: {:?}", Ok::<i32, Error>(42));
    let e1 = Error::invalid_arg("bad input");
    println!("  invalid_arg: {}", e1);
    let e2 = Error::not_found("config.json");
    println!("  not_found: {}", e2);
    let e3 = Error::invalid_state("not mounted");
    println!(
        "  invalid_state: {} .is(InvalidState): {}",
        e3,
        e3.is(Errc::InvalidState)
    );
    let root = Error::new(Errc::IoError, "disk full");
    let chained = Error::new(Errc::WriteFailure, "write failed").with_source(root);
    println!("  Chained: {}", chained);
    println!("  root_cause: {}", chained.root_cause());
}

pub fn demo_recovery() {
    println!("\n╔══ diagnostics：指定错误恢复 ═══╗");
    let diagnostics = Diagnostics::default();
    let recoveries = Arc::new(AtomicUsize::new(0));
    let recovery_count = Arc::clone(&recoveries);
    let _subscription = diagnostics.on_error(Errc::Timeout, move |_| {
        recovery_count.fetch_add(1, Ordering::Relaxed);
        RecoveryAction::Recovered
    });

    let outcome = diagnostics.attempt_recovery(Error::new(Errc::Timeout, "demo timeout"));
    println!(
        "  recovered={} callbacks={}",
        matches!(outcome, RecoveryOutcome::Recovered),
        recoveries.load(Ordering::Relaxed)
    );
}

pub fn demo_state() {
    println!("\n╔══ ui：响应式状态 ═══╗");
    let count = State::new(0i32);
    println!("  State(0) = {}", count.get());
    count.watch(|v| println!("    ⤷ Watcher: count = {}", v));
    count.set(1);
    count.set(2);
    count.update(|v| *v += 10);
    println!(
        "  After set(1,2) + update(+10): {} gen:{}",
        count.get(),
        count.generation()
    );
    let a = State::new(5);
    let b = State::new(3);
    let sum = Computed::new({
        let a = a.clone();
        let b = b.clone();
        move || a.get() + b.get()
    });
    println!("  Computed: {}+{}={}", a.get(), b.get(), sum.get());
    a.set(10);
    sum.invalidate();
    println!("  After a=10, invalidate: sum={}", sum.get());
}

pub fn demo_flex() {
    println!("\n╔══ ui：Flex 布局 ═══╗");
    let flex = FlexLayout::row()
        .with_gap(8.0)
        .with_justify(JustifyContent::Center)
        .with_align(AlignItems::Center);

    let children = vec![
        LayoutChild::new(ComponentId::new(0), Size::new(60.0, 200.0)).with_flex(1.0, 1.0),
        LayoutChild::new(ComponentId::new(1), Size::new(100.0, 200.0)).with_flex(2.0, 1.0),
        LayoutChild::new(ComponentId::new(2), Size::new(60.0, 200.0)).with_flex(1.0, 1.0),
    ];

    let container = Rect::new(0.0, 0.0, 400.0, 300.0);
    let padding = EdgeInsets::uniform(16.0);
    let content_rect = Rect::new(
        container.x + padding.left,
        container.y + padding.top,
        (container.w - padding.horizontal()).max(0.0),
        (container.h - padding.vertical()).max(0.0),
    );

    let out = flex.layout(content_rect, &children);
    println!("  Row flex(1,2,1) in 400x300 pad=16 gap=8:");
    for (i, r) in out.positions.iter().enumerate() {
        println!(
            "    [{}] x={:.0} y={:.0} w={:.0} h={:.0}",
            i, r.x, r.y, r.w, r.h
        );
    }
}

pub fn demo_theme() {
    println!("\n╔══ ui：主题 (Ant Design 5) ═══╗");
    let l = Theme::antd_light();
    println!(
        "  Light: primary:{} bg:{} surface:{} text:{} dark:{}",
        l.tokens().color_primary(),
        l.tokens().color_bg_elevated(),
        l.tokens().color_bg_container(),
        l.tokens().color_text(),
        l.tokens().is_dark()
    );
    let d = Theme::antd_dark();
    println!(
        "  Dark:  primary:{} bg:{} surface:{} text:{} dark:{}",
        d.tokens().color_primary(),
        d.tokens().color_bg_elevated(),
        d.tokens().color_bg_container(),
        d.tokens().color_text(),
        d.tokens().is_dark()
    );
}

pub fn demo_settings() -> Result<(), Error> {
    println!("\n╔══ data：设置服务 ═══╗");
    let s = SettingsService::new();
    s.set("theme", "dark");
    s.set("font_size", "14");
    s.set("language", "zh-CN");
    println!("  count:{} dirty:{}", s.count(), s.dirty());
    println!(
        "  theme={:?} font_size={:?} missing={:?}",
        s.get("theme"),
        s.get("font_size"),
        s.get("nope")
    );
    let p = std::env::temp_dir().join("uix_demo_settings.json");
    let ps = p.to_string_lossy().to_string();
    s.load(&ps)?;
    s.set("demo_ok", "true");
    s.save()?;
    let s2 = SettingsService::new();
    s2.load(&ps)?;
    println!("  reloaded: {} keys", s2.count());
    std::fs::remove_file(&ps)?;
    Ok(())
}

pub fn demo_file_service() -> Result<(), Error> {
    println!("\n╔══ std：文件服务 ═══╗");
    let tmp = std::env::temp_dir().join("uix_demo_fs");
    let p = tmp.join("hello.txt");
    std::fs::create_dir_all(&tmp)?;
    std::fs::write(&p, "Hello UIX!\nLine 2.\n")?;
    let metadata = std::fs::metadata(&p)?;
    println!(
        "  written, exists:{} size:{}",
        p.exists(),
        metadata.len()
    );
    println!("  content: {}", std::fs::read_to_string(&p)?.trim());
    use std::io::Write as _;
    let mut file = std::fs::OpenOptions::new().append(true).open(&p)?;
    file.write_all(b"Line 3.\n")?;
    println!(
        "  after append: {} lines",
        std::fs::read_to_string(&p)?.lines().count()
    );
    std::fs::remove_file(&p)?;
    std::fs::remove_dir(&tmp)?;
    Ok(())
}

pub fn demo_renderer() -> Result<(), Error> {
    println!("\n╔══ draw：图形引擎 ═══╗");
    let mut e = Renderer::cpu();
    e.initialize(800, 600)?;
    let (width, height) = e.logical_extent();
    println!("  Renderer(cpu) logical extent={width}x{height}");
    e.try_shutdown()?;
    let mut boxed: Box<dyn RenderTarget> = Box::new(Renderer::cpu());
    boxed.initialize(640, 480)?;
    let (width, height) = boxed.logical_extent();
    println!("  Box<dyn RenderTarget> logical extent={width}x{height}");
    boxed.try_shutdown()?;
    Ok(())
}

pub fn demo_di_container() {
    println!("\n╔══ app：依赖注入容器 ═══╗");
    let mut c = DiContainer::new();
    c.singleton("config_value".to_string());
    c.singleton(42i32);
    match c.resolve::<String>() {
        Some(v) => println!("  resolved String: {:?}", v),
        None => eprintln!("  resolved String: not found"),
    }
    match c.resolve::<i32>() {
        Some(v) => println!("  resolved i32: {:?}", v),
        None => eprintln!("  resolved i32: not found"),
    }
    println!("  has::<f64>: {}", c.has::<f64>());
}

pub fn run() -> Result<(), Error> {
    println!("\n  ╔══════════════════════════════════╗");
    println!("  ║   UIX 框架 — CLI 功能域演示       ║");
    println!("  ╚══════════════════════════════════╝");
    demo_core_types();
    demo_errors();
    demo_recovery();
    demo_state();
    demo_flex();
    demo_settings()?;
    demo_file_service()?;
    demo_theme();
    demo_renderer()?;
    demo_di_container();
    println!("\n  ╔══════════════════════════════════╗");
    println!("  ║   全部演示完成！                  ║");
    println!("  ╚══════════════════════════════════╝\n");
    Ok(())
}
