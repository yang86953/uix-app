use super::*;
use crate::core::Point;
// 引入真实渲染布局所需的字体服务。
use crate::draw::resources::font::font_service::FontService;
// 引入稳定的测试字体句柄。
use crate::draw::FontHandle;
// 引入事件与测量契约。
use crate::ui::component::traits::{EventHandler, WidgetLayout};
use crate::ui::event::SystemEvent;
// 引入键盘、修饰键与指针按钮。
use crate::ui::{KeyCode, KeyMod, MouseButton};

fn dummy_event() -> SystemEvent {
    SystemEvent::PointerUp {
        pos: Point::new(0.0, 0.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    }
}

#[test]
fn on_link_callback_fires_when_submit_emitted() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let hook = calls.clone();
    let rich = RichText::new().on_link(move |url| hook.borrow_mut().push(url.to_string()));
    rich.pending_submit
        .replace(Some("https://example.com".to_string()));

    let event = rich.semantic_event(ComponentId::default(), &dummy_event());
    assert!(event.is_some(), "应发出 Submit 语义事件");
    assert_eq!(*calls.borrow(), vec!["https://example.com".to_string()]);
}

#[test]
fn on_link_not_called_without_pending_submit() {
    let calls = Rc::new(RefCell::new(0usize));
    let hook = calls.clone();
    let rich = RichText::new().on_link(move |_| *hook.borrow_mut() += 1);

    let event = rich.semantic_event(ComponentId::default(), &dummy_event());
    assert!(event.is_none());
    assert_eq!(*calls.borrow(), 0, "无待提交链接时不应触发回调");
}

#[test]
fn on_link_and_submit_semantic_event_coexist() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let hook = calls.clone();
    let rich = RichText::new().on_link(move |url| hook.borrow_mut().push(url.to_string()));
    rich.pending_submit
        .replace(Some("https://uix.dev/route".to_string()));

    let Some(event) = rich.semantic_event(ComponentId::default(), &dummy_event()) else {
        panic!("Submit 语义事件保留");
    };
    assert_eq!(
        event.kind,
        crate::ui::SemanticKind::Submit,
        "与 SemanticKind::Submit 共存"
    );
    assert!(
        matches!(&event.payload, crate::ui::SemanticPayload::Text(url) if url == "https://uix.dev/route")
    );
    assert_eq!(*calls.borrow(), vec!["https://uix.dev/route".to_string()]);
}

// 验证跨样式段的组合序列仍形成单一可选择字素簇。
#[test]
fn selection_expands_across_style_segments_to_complete_grapheme() {
    // 基字符和组合音标故意拆到两个不同样式段。
    let rich = RichText::new()
        // 显式开启本测试覆盖的文字选择能力。
        .selectable(true)
        // 注入跨样式段的组合字符。
        .content(vec![
            // 第一段保存基础拉丁字母。
            RichTextSegment::Text {
                // 使用基础字符正文。
                content: "a".to_owned(),
                // 使用默认样式。
                style: RichTextStyle::default(),
            },
            // 第二段保存组合音标并改变样式。
            RichTextSegment::Text {
                // 使用组合锐音正文。
                content: "\u{0301}".to_owned(),
                // 使用粗体样式验证跨段拼接。
                style: RichTextStyle {
                    // 启用粗体以形成不同样式 run。
                    bold: true,
                    // 其余样式沿用默认值。
                    ..RichTextStyle::default()
                },
            },
            // 第三段提供相邻普通字符。
            RichTextSegment::Text {
                // 使用尾随拉丁字符。
                content: "z".to_owned(),
                // 使用默认样式。
                style: RichTextStyle::default(),
            },
        ]);
    // 模拟旧调用方从组合序列内部选择到其终点。
    rich.set_selection_range(1, 2);
    // 选择必须向前扩展并包含基础字符。
    assert_eq!(rich.selection.get(), Some((0, 2)));
    // 提取文本必须保留完整组合序列。
    assert_eq!(rich.selected_text().as_deref(), Some("a\u{0301}"));
    // 同一点空选择不得意外选中整个字素簇。
    rich.set_selection_range(1, 1);
    // 空选择应被清除。
    assert_eq!(rich.selection.get(), None);
}

// 验证默认关闭与显式开启共同约束键盘全选入口。
#[test]
// 定义默认值与显式配置的选择入口测试。
fn selectable_controls_keyboard_selection_and_defaults_to_false() {
    // 构造沿用默认不可选择契约的富文本。
    let mut disabled = RichText::new().content(parse_rich_text("abc"));
    // 构造统一的 Ctrl+A 输入事件。
    let select_all = SystemEvent::KeyDown {
        // 使用全选键。
        key: KeyCode::A,
        // 同时按下控制修饰键。
        mods: KeyMod::CTRL,
    };
    // 默认实例不得消费全选快捷键。
    assert_eq!(disabled.on_event(&select_all), EventResult::NotHandled);
    // 默认实例不得建立任何选区。
    assert_eq!(disabled.selected_text(), None);
    // 构造显式开启选择能力的同内容实例。
    let mut enabled = RichText::new()
        // 开启普通文字选择。
        .selectable(true)
        // 注入可预测的三字符正文。
        .content(parse_rich_text("abc"));
    // 开启后的实例应消费全选快捷键。
    assert_eq!(enabled.on_event(&select_all), EventResult::Handled);
    // 开启后的实例应返回完整选中文本。
    assert_eq!(enabled.selected_text().as_deref(), Some("abc"));
}

// 验证 reconcile 关闭能力时立即终止既有选择生命周期。
#[test]
// 定义 reconcile 关闭选择能力的生命周期测试。
fn reconcile_disabling_selectable_clears_selection_and_dragging() {
    // 构造已开启选择能力的当前组件。
    let mut rich = RichText::new()
        // 开启选择以建立待清理状态。
        .selectable(true)
        // 注入稳定正文。
        .content(parse_rich_text("abc"));
    // 建立完整文本选区。
    rich.set_selection_range(0, 3);
    // 模拟仍在进行的拖拽会话。
    rich.sel_dragging.set(true);
    // 使用相同内容但默认关闭选择的声明进行 reconcile。
    rich.sync_from(RichText::new().content(parse_rich_text("abc")));
    // 关闭后的组件不得参加跨节点选择。
    assert!(!rich.participates_in_cross_text_selection());
    // 关闭边界必须清除已有选区。
    assert_eq!(rich.selected_text(), None);
    // 关闭边界必须重置旧选择锚点。
    assert_eq!(rich.sel_anchor.get(), 0);
    // 关闭边界必须终止拖拽会话。
    assert!(!rich.sel_dragging.get());
}

// 以小误差比较测量路径与真实渲染布局的浮点几何。
fn assert_geometry_close(
    // 接收组件测量得到的尺寸。
    measured: Size,
    // 接收真实渲染布局得到的高度。
    rendered_height: f32,
    // 接收真实渲染布局得到的最大行宽。
    rendered_width: f32,
) {
    // 测量高度必须与实际绘制行盒总高度一致。
    assert!(
        // 允许浮点计算产生极小误差。
        (measured.h - rendered_height).abs() < 0.01,
        // 失败时同时报告两条路径的高度。
        "测量与渲染高度不一致：measured={measured:?}, rendered_height={rendered_height}",
    );
    // 测量宽度必须与实际绘制最大行宽一致。
    assert!(
        // 允许浮点计算产生极小误差。
        (measured.w - rendered_width).abs() < 0.01,
        // 失败时同时报告两条路径的宽度。
        "测量与渲染宽度不一致：measured={measured:?}, rendered_width={rendered_width}",
    );
}

// 验证跨字号与跨宽度时，组件测量和生产渲染布局共享同一几何结果。
#[test]
fn measure_matches_render_layout_across_font_sizes_and_widths() {
    // 构造同时包含段级大字号和默认字号的富文本组件。
    let rich = RichText::new()
        // 固定组件默认字号以形成可预测的十二像素基线。
        .font_size(12.0)
        // 注入足以在窄宽度下折行的混合字号文本。
        .content(vec![
            // 第一段覆盖段级大字号路径。
            RichTextSegment::Text {
                // 使用短前缀确保大字号字形保留在首行。
                content: "Large ".to_owned(),
                // 将当前段字号提升为默认字号的两倍。
                style: RichTextStyle {
                    // 显式指定二十四像素字号。
                    font_size: Some(24.0),
                    // 其余样式保持默认。
                    ..RichTextStyle::default()
                },
            },
            // 第二段覆盖默认字号和自动折行路径。
            RichTextSegment::Text {
                // 使用多个单词在九十六像素宽度下产生多行。
                content: "alpha beta gamma delta epsilon zeta eta theta".to_owned(),
                // 默认样式继承组件的十二像素字号。
                style: RichTextStyle::default(),
            },
        ]);
    // 使用不加载系统字体的服务，让真实布局稳定走估算宽度兜底。
    let font_service = FontService::new();
    // 使用字体服务的零号占位句柄。
    let font = FontHandle::new(0);
    // 解析组件在标准九十六 DPI 下的默认字号。
    let default_font_size = rich.resolved_font_size_px(96.0);
    // 保存测量与渲染共同使用的默认文字色。
    let default_color = rich.default_color;
    // 选择足以容纳全部文本或只产生少量折行的宽约束。
    let wide_width = 480.0;
    // 在宽约束下执行组件测量路径。
    let wide_measured = rich.measure(Constraints::loose(Size::new(wide_width, 1_000.0)));
    // 在同一宽约束下执行生产 render 调用的真实布局路径。
    let (wide_lines, wide_rendered_height, wide_rendered_width) = layout_rich_text_real(
        // 复用组件持有的完整段列表。
        &rich.segments,
        // 传入宽布局约束。
        wide_width,
        // 传入组件解析后的默认字号。
        default_font_size,
        // 传入组件默认颜色。
        default_color,
        // 传入无系统依赖的字体服务。
        &font_service,
        // 传入稳定占位字体句柄。
        &font,
    );
    // 宽约束下两条路径必须给出相同几何。
    assert_geometry_close(
        // 传入测量结果。
        wide_measured,
        // 传入真实布局高度。
        wide_rendered_height,
        // 传入真实布局最大行宽。
        wide_rendered_width,
    );
    // 选择会迫使默认字号文本自动折行的窄约束。
    let narrow_width = 96.0;
    // 使用同一组件实例验证宽度变化会使测量缓存失效。
    let narrow_measured = rich.measure(Constraints::loose(Size::new(narrow_width, 1_000.0)));
    // 在窄约束下执行生产 render 调用的真实布局路径。
    let (narrow_lines, narrow_rendered_height, narrow_rendered_width) = layout_rich_text_real(
        // 复用同一段列表以隔离宽度变量。
        &rich.segments,
        // 传入窄布局约束。
        narrow_width,
        // 传入相同默认字号。
        default_font_size,
        // 传入相同默认颜色。
        default_color,
        // 复用同一字体服务。
        &font_service,
        // 复用同一字体句柄。
        &font,
    );
    // 窄约束下两条路径也必须给出相同几何。
    assert_geometry_close(
        // 传入窄约束测量结果。
        narrow_measured,
        // 传入窄约束真实布局高度。
        narrow_rendered_height,
        // 传入窄约束真实布局最大行宽。
        narrow_rendered_width,
    );
    // 窄宽度必须产生比宽宽度更多的实际绘制行。
    assert!(narrow_lines.len() > wide_lines.len());
    // 更多绘制行必须使真实内容总高度增加。
    assert!(narrow_rendered_height > wide_rendered_height);
    // 查找包含二十四像素段级字号字形的实际绘制行。
    let large_font_line = narrow_lines
        // 遍历窄布局的全部实际绘制行。
        .iter()
        // 选择包含目标大字号字形的行。
        .find(|line| {
            // 检查行内任一字形是否保留二十四像素字号。
            line.glyphs
                // 遍历当前行的全部字形。
                .iter()
                // 使用小误差匹配目标字号。
                .any(|glyph| (glyph.font_size - 24.0).abs() < 0.01)
        })
        // 测试数据必须生成至少一个大字号绘制行。
        .expect("真实渲染布局应保留段级大字号字形");
    // 大字号所在行必须采用字号乘一又二分之一的行高。
    assert!((large_font_line.height - 36.0).abs() < 0.01);
}
