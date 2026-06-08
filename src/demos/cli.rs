//! CLI demos — exercises all major UIX subsystems from the command line.

#![allow(dead_code)]

use std::cell::Cell;
use uix::diag::{Errc, Error};
use uix::graphics::{
    self, colors, compute_flex_layout, AlignItems as GAlign, EdgeInsets, FlexChild,
    FlexDirection as GDir, FlexInput, GraphicsEngine, JustifyContent as GJustify, Point, Rect,
    Size,
};
use uix::platform::log::{info_fn, Level, Logger};
use uix::services::{
    FileService, LogMiddleware as SvcLogMiddleware, MiddlewareContext, MiddlewarePipeline,
    RetryMiddleware, SettingsService,
};
use uix::ui::state::{Computed, State};
use uix::ui::Theme;

// ── Logger ────────────────────────────────────────────────────────────────

pub fn init_logger() {
    Logger::instance().set_level(Level::Info);
    info_fn("Demo Logger initialized");
}

// ── Core Types ────────────────────────────────────────────────────────────

pub fn demo_core_types() -> Result<(), Error> {
    println!("\n╔══ Core Types ═══╗");
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
    Ok(())
}

// ── Error Handling ────────────────────────────────────────────────────────

pub fn demo_errors() -> Result<(), Error> {
    println!("\n╔══ Error Handling ═══╗");
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
    let chained = Error::new(Errc::WriteFailure, "write failed").with_cause(root);
    println!("  Chained: {}", chained);
    println!("  root_cause: {}", chained.root_cause());
    Ok(())
}

// ── Reactive State ────────────────────────────────────────────────────────

pub fn demo_state() {
    println!("\n╔══ Reactive State ═══╗");
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

// ── Flex Layout ───────────────────────────────────────────────────────────

pub fn demo_flex() {
    println!("\n╔══ Flex Layout ═══╗");
    let out = compute_flex_layout(&FlexInput {
        direction: GDir::Row,
        gap: 8.0,
        padding: EdgeInsets::uniform(16.0),
        container: Rect::new(0.0, 0.0, 400.0, 300.0),
        children: vec![
            FlexChild {
                flex_grow: 1.0,
                ..Default::default()
            },
            FlexChild {
                flex_grow: 2.0,
                ..Default::default()
            },
            FlexChild {
                flex_grow: 1.0,
                ..Default::default()
            },
        ],
        child_sizes: vec![
            Size::new(60.0, 200.0),
            Size::new(100.0, 200.0),
            Size::new(60.0, 200.0),
        ],
        justify_content: GJustify::Center,
        align_items: GAlign::Center,
        ..Default::default()
    });
    println!("  Row flex(1,2,1) in 400x300 pad=16 gap=8:");
    for (i, r) in out.child_rects.iter().enumerate() {
        println!(
            "    [{}] x={:.0} y={:.0} w={:.0} h={:.0}",
            i, r.x, r.y, r.w, r.h
        );
    }
}

// ── Settings Service ──────────────────────────────────────────────────────

pub fn demo_settings() -> Result<(), Error> {
    println!("\n╔══ Settings Service ═══╗");
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

// ── Middleware ─────────────────────────────────────────────────────────────

pub fn demo_middleware() {
    println!("\n╔══ Middleware ═══╗");
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

// ── File Service ──────────────────────────────────────────────────────────

pub fn demo_file_service() -> Result<(), Error> {
    println!("\n╔══ File Service ═══╗");
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

// ── Theme ─────────────────────────────────────────────────────────────────

pub fn demo_theme() {
    println!("\n╔══ Theme (Ant Design 5) ═══╗");
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

// ── Graphics Engine ───────────────────────────────────────────────────────

pub fn demo_graphics_engine() {
    println!("\n╔══ Graphics Engine ═══╗");
    let mut e = graphics::NullEngine::new();
    let _ = e.initialize(std::ptr::null_mut(), 800, 600);
    e.fill_rect(Rect::new(10.0, 10.0, 100.0, 50.0), colors::PRIMARY, None);
    e.shutdown();
    let mut boxed: Box<dyn GraphicsEngine> = Box::new(graphics::NullEngine::new());
    let _ = boxed.initialize(std::ptr::null_mut(), 640, 480);
    println!("  Box<dyn GraphicsEngine> width={}", boxed.width());
    boxed.shutdown();
}

// ── DI Container ──────────────────────────────────────────────────────────

pub fn demo_di_container() {
    println!("\n╔══ DI Container ═══╗");
    let mut c = uix::app::Container::new();
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

// ── Run All ───────────────────────────────────────────────────────────────

pub fn run_all_cli() -> Result<(), Error> {
    println!("\n  ╔══════════════════════════════════╗");
    println!("  ║   UIX Framework — CLI Demo      ║");
    println!("  ╚══════════════════════════════════╝");
    demo_core_types()?;
    demo_errors()?;
    demo_state();
    demo_flex();
    demo_settings()?;
    demo_middleware();
    demo_file_service()?;
    demo_theme();
    demo_graphics_engine();
    demo_di_container();
    println!("\n  ╔══════════════════════════════════╗");
    println!("  ║   All demos completed!           ║");
    println!("  ╚══════════════════════════════════╝\n");
    Ok(())
}
