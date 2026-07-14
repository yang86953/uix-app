//! CLI 演示 — 按功能域展示 `core` / `native` / `draw` / `ui` / `app` / `data` 能力。
//! 使用 [`使用.md`](../../docs/使用.md) 风格。
//!
//! 运行：`cargo run --bin uix-demo -- --cli`

use uix::core::diagnostic::{
    LogMiddleware as SvcLogMiddleware, MiddlewareContext, MiddlewarePipeline, RetryMiddleware,
};
use uix::core::log::{info_fn, Level, Logger};
use uix::data::SettingsService;
use uix::native::file_service::FileService;
use uix::prelude::*;

#[allow(dead_code)]
pub fn init_logger() {
    Logger::instance().set_level(Level::Info);
    info_fn("演示日志初始化完成");
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
    // 使用 使用.md 的 Error API
    let e1 = Error::new(Errc::InvalidArgument, "bad input");
    println!("  invalid_arg: {}", e1);
    let e2 = Error::not_found("config.json");
    println!("  not_found: {}", e2);
    let e3 = Error::invalid_state("not mounted");
    println!(
        "  invalid_state: {} .is(InvalidState): {}",
        e3,
        e3.code() == Errc::InvalidState
    );
    // 链式错误
    let root = Error::new(Errc::IoError, "disk full");
    let wrapped = Error::new(Errc::WriteFailure, "write failed").with_source(root);
    println!("  Chained: {}", wrapped);
    println!("  root_cause: {}", wrapped.root_cause());
    // 严重级别
    let fatal = Error::new(Errc::InvalidState, "窗口已销毁")
        .set_severity(uix::core::diagnostic::ErrorSeverity::Fatal);
    println!("  Fatal severity: {:?}", fatal.severity());
}

pub fn demo_middleware() {
    println!("\n╔══ core：诊断中间件 ═══╗");
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
}

pub fn demo_state() {
    println!("\n╔══ ui：响应式状态 ═══╗");
    // State
    let count = State::new(0i32);
    println!("  State(0) = {}", count.get());
    count.set(1);
    count.update(|v| *v += 10);
    println!("  After set(1) + update(+10): {}", count.get());

    // Computed —— 派生状态
    let first = State::new("Ada");
    let last = State::new("Lovelace");
    let full = Computed::new(move || format!("{} {}", first.get(), last.get()));
    println!("  Computed: {}", full.get());

    // Effect —— 副作用
    let effect_count = State::new(0);
    let _effect = Effect::new({
        let cnt = effect_count.clone();
        move || {
            let v = cnt.get();
            if v > 0 {
                info_fn(&format!("Effect 触发: count={v}"));
            }
        }
    });
    effect_count.set(1);
    println!("  Effect: registered, count={}", effect_count.get());
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
        "  Light: primary:{} bg_elevated:{} text:{} dark:{}",
        l.tokens().color_primary(),
        l.tokens().color_bg_elevated(),
        l.tokens().color_text(),
        l.tokens().is_dark()
    );
    let d = Theme::antd_dark();
    println!(
        "  Dark:  primary:{} bg_elevated:{} text:{} dark:{}",
        d.tokens().color_primary(),
        d.tokens().color_bg_elevated(),
        d.tokens().color_text(),
        d.tokens().is_dark()
    );

    // 自定义主题
    let primitives = ThemePrimitives {
        primary: Color::hex("#722ed1"),
        success: Color::hex("#52c41a"),
        warning: Color::hex("#faad14"),
        error: Color::hex("#ff4d4f"),
        info: Color::hex("#722ed1"),
        bg: Color::hex("#f5f5f7"),
        text: Color::BLACK,
        border: Color::hex("#e4e4e7"),
    };
    let custom = DesignTokens::from_primitives(primitives, false);
    println!(
        "  Custom: primary:{} bg:{}",
        custom.color_primary, custom.color_bg
    );
}

pub fn demo_settings() -> Result<(), Error> {
    println!("\n╔══ data：设置服务 ═══╗");
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

pub fn demo_file_service() -> Result<(), Error> {
    println!("\n╔══ native：文件服务 ═══╗");
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

pub fn demo_graphics_engine() -> Result<(), Error> {
    println!("\n╔══ draw：图形引擎 ═══╗");
    let mut e = NullEngine::new();
    e.initialize(800, 600)?;
    e.canvas_2d()
        .fill_rect(Rect::new(10.0, 10.0, 100.0, 50.0), colors::PRIMARY, None);
    e.try_shutdown()?;
    let mut boxed: Box<dyn GraphicsEngine> = Box::new(NullEngine::new());
    boxed.initialize(640, 480)?;
    println!(
        "  Box<dyn GraphicsEngine> width={}",
        boxed.canvas_2d().width()
    );
    boxed.try_shutdown()?;
    Ok(())
}

pub fn demo_di_container() {
    println!("\n╔══ app：依赖注入容器 ═══╗");
    let mut c = DiContainer::new();
    c.singleton(42i32);
    c.singleton("config_value".to_string());
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
    demo_middleware();
    demo_state();
    demo_flex();
    demo_settings()?;
    demo_file_service()?;
    demo_theme();
    demo_graphics_engine()?;
    demo_di_container();
    println!("\n  ╔══════════════════════════════════╗");
    println!("  ║   全部演示完成！                  ║");
    println!("  ╚══════════════════════════════════╝\n");
    Ok(())
}
