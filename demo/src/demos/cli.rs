//! CLI 演示 — 从命令行运行所有主要 UIX 子系统的功能演示。

use std::cell::Cell;
use uix::prelude::*;
use std::result::Result;
use uix::core::diagnostic::{
    LogMiddleware as SvcLogMiddleware, MiddlewareContext, MiddlewarePipeline, RetryMiddleware,
};
use uix::native::services::file_service::FileService;
use uix::core::log::{info_fn, Level, Logger};
use uix::data::settings::SettingsService;

// ── Logger ────────────────────────────────────────────────────────────────

#[allow(dead_code)]
pub fn init_logger() {
    Logger::instance().set_level(Level::Info);
    info_fn("演示日志初始化完成");
}

// ── 核心类型 ──────────────────────────────────────────────────────────────

pub fn demo_core_types() {
    println!("\n╔══ 核心类型 ═══╗");
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

// ── 错误处理 ──────────────────────────────────────────────────────────────

pub fn demo_errors() {
    println!("\n╔══ 错误处理 ═══╗");
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

// ── 响应式状态 ────────────────────────────────────────────────────────────

pub fn demo_state() {
    println!("\n╔══ 响应式状态 ═══╗");
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

// ── Flex 布局 ────────────────────────────────────────────────────────────

pub fn demo_flex() {
    println!("\n╔══ Flex 布局 ═══╗");
    // 使用新的公共 LayoutEngine API
    let flex = FlexLayout::row()
        .with_gap(8.0)
        .with_justify(JustifyContent::Center)
        .with_align(AlignItems::Center);

    let children = vec![
        LayoutChild::new(0, Size::new(60.0, 200.0)).with_flex(1.0, 1.0),
        LayoutChild::new(1, Size::new(100.0, 200.0)).with_flex(2.0, 1.0),
        LayoutChild::new(2, Size::new(60.0, 200.0)).with_flex(1.0, 1.0),
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

// ── 设置服务 ──────────────────────────────────────────────────────────────

pub fn demo_settings() -> Result<(), Error> {
    println!("\n╔══ 设置服务 ═══╗");
    let mut s = SettingsService::new();
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
    let mut s2 = SettingsService::new();
    s2.load(&ps)?;
    println!("  reloaded: {} keys", s2.count());
    std::fs::remove_file(&ps)?;
    Ok(())
}

// ── 中间件 ────────────────────────────────────────────────────────────────

pub fn demo_middleware() {
    println!("\n╔══ 中间件 ═══╗");
    let mut p = MiddlewarePipeline::new();
    p.add(SvcLogMiddleware);
    p.add(RetryMiddleware::new(2));
    let mut ctx = MiddlewareContext {
        service_name: "Demo".into(),
        operation: "GET /ok".into(),
        ..Default::default()
    };
    p.execute(&mut ctx, |c| {
        c.succeeded = true;
        c.status_code = 200;
    });
    println!("  Success: ok={} status={}", ctx.succeeded, ctx.status_code);
    let mut fctx = MiddlewareContext {
        operation: "GET /fail".into(),
        ..Default::default()
    };
    let att = Cell::new(0u32);
    p.execute(&mut fctx, move |c| {
        let n = att.get() + 1;
        att.set(n);
        if n >= 3 {
            c.succeeded = true;
            c.status_code = 200;
        } else {
            c.succeeded = false;
            c.error_message = format!("fail #{}", n);
        }
        println!(
            "    attempt {}: {}",
            n,
            if c.succeeded { "OK" } else { "fail" }
        );
    });
    println!(
        "  After retry: ok={} retries={}",
        fctx.succeeded, fctx.retry_count
    );
}

// ── 文件服务 ──────────────────────────────────────────────────────────────

pub fn demo_file_service() -> Result<(), Error> {
    println!("\n╔══ 文件服务 ═══╗");
    let fs = FileService::new();
    let tmp = std::env::temp_dir().join("uix_demo_fs");
    let p = tmp.join("hello.txt");
    let ps = p.to_string_lossy().to_string();
    fs.write_string(&ps, "Hello UIX!\nLine 2.\n")?;
    println!(
        "  written, exists:{} size:{}",
        fs.exists(&ps),
        fs.file_size(&ps)?
    );
    println!("  content: {}", fs.read_to_string(&ps)?.trim());
    fs.append_string(&ps, "Line 3.\n")?;
    println!("  after append: {} lines", fs.read_lines(&ps)?.len());
    fs.remove(&ps)?;
    std::fs::remove_dir(&tmp).ok();
    Ok(())
}

// ── 主题 ───────────────────────────────────────────────────────────────────

pub fn demo_theme() {
    println!("\n╔══ 主题 (Ant Design 5) ═══╗");
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

// ── 图形引擎 ──────────────────────────────────────────────────────────────

pub fn demo_graphics_engine() -> Result<(), Error> {
    println!("\n╔══ 图形引擎 ═══╗");
    let mut e = NullEngine::new();
    e.initialize(800, 600)?;
    e.canvas_2d()
        .fill_rect(Rect::new(10.0, 10.0, 100.0, 50.0), colors::PRIMARY, None);
    e.shutdown();
    let mut boxed: Box<dyn GraphicsEngine> = Box::new(NullEngine::new());
    boxed.initialize(640, 480)?;
    println!(
        "  Box<dyn GraphicsEngine> width={}",
        boxed.canvas_2d().width()
    );
    boxed.shutdown();
    Ok(())
}

// ── 依赖注入容器 ──────────────────────────────────────────────────────────

pub fn demo_di_container() {
    println!("\n╔══ 依赖注入容器 ═══╗");
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

// ── 运行全部 ──────────────────────────────────────────────────────────────

pub fn run_all_cli() -> Result<(), Error> {
    println!("\n  ╔══════════════════════════════════╗");
    println!("  ║   UIX 框架 — CLI 演示         ║");
    println!("  ╚══════════════════════════════════╝");
    demo_core_types();
    demo_errors();
    demo_state();
    demo_flex();
    demo_settings()?;
    demo_middleware();
    demo_file_service()?;
    demo_theme();
    demo_graphics_engine()?;
    demo_di_container();
    println!("\n  ╔══════════════════════════════════╗");
    println!("  ║   全部演示完成！                  ║");
    println!("  ╚══════════════════════════════════╝\n");
    Ok(())
}
