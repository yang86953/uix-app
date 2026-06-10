//! Ant Design 5 全组件展示 — 按分类展示所有 UIX widget 及 Ant Design 典型组合。
//!
//! Run: `cargo run --bin uix-demo`

use uix::app::{map_ui_event, App};
use uix::graphics::{Color, EdgeInsets, GraphicsEngine, Rect, Size};
use uix::platform::event::{UiEvent, UiEventPayload, UiEventType};
use uix::platform::types::KeyCode;
use uix::ui::render_context::RenderContext;
use uix::ui::theme::DesignTokens;
use uix::ui::widget::{EventResult, WidgetEvent, WidgetNode, WidgetTree};
use uix::ui::{
    AlignItems, Button, ButtonSize, Card, Container, Divider,
    DividerOrientation, FlexDirection, Grid, Input,
    InputSize, IntoWidgetNode, Label, Navigation, ProgressBar,
    ScrollDirection, ScrollView, Space, SpaceSize, TabPosition, Tabs,
};
use uix::{define_widget, tree};

const GW: i32 = 1100;
const GH: i32 = 740;
const SB: f32 = 200.0;         // sidebar width
fn cw() -> f32 { GW as f32 - SB }
fn iw() -> f32 { cw() - 40.0 }

// ── 辅助构建函数 ──

fn heading(tk: &DesignTokens, text: &str) -> Label {
    Label::new(text, tk.color_text).font_size(18.0)
}
fn sub(tk: &DesignTokens, text: &str) -> Label {
    Label::new(text, tk.color_text_secondary).font_size(12.0)
}
fn row(h: f32) -> Space {
    Space::new().size(SpaceSize::Small).width(iw()).height(h)
        .direction(FlexDirection::Row).align(AlignItems::Center)
}
fn col(h: f32) -> Space {
    Space::new().size(SpaceSize::Small).width(iw()).height(h)
        .direction(FlexDirection::Column).align(AlignItems::Stretch)
}

/// 统计卡片
fn stat_card(tk: &DesignTokens, title: &str, value: &str, color: Color, elev: u8) -> Card {
    let w = (iw() - 24.0) / 4.0;
    Card::new().title(title).elevation(elev).hoverable()
        .size(w, 100.0)
        .child(Label::new(value, color).font_size(26.0))
        .child(Label::new(title, tk.color_text_quaternary).font_size(11.0))
}

// ── Counter — 自定义 widget 示例 ──

define_widget! {
    pub struct Counter { count: u32 }
    @new -> Self { Self { count: 0 } }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size { Size::new(120.0, 36.0) }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event { WidgetEvent::MouseDown { .. } => { self.count += 1; EventResult::Handled } _ => EventResult::NotHandled }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_primary_bg();
        let color = ctx.tokens().color_primary();
        ctx.fill_rect(frame, bg, None);
        ctx.draw_text(&format!("Count: {}", self.count), uix::graphics::Point::new(frame.x + 8.0, frame.y + 8.0), color, 14.0);
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Section 构建 — 每个分类一个函数，每个函数不超过 60 行
// ════════════════════════════════════════════════════════════════════════════

fn sec_dashboard(tk: &DesignTokens, out: &mut Vec<WidgetNode>) {
    out.push(heading(tk, "📊 Dashboard — StatCards").into_node());
    out.push(Space::new().size(SpaceSize::Small).width(iw()).height(110.0)
        .direction(FlexDirection::Row).align(AlignItems::Stretch)
        .child(stat_card(tk, "Total Users", "12,834", tk.color_primary, 2))
        .child(stat_card(tk, "Revenue", "$8,291", tk.color_success, 1))
        .child(stat_card(tk, "Orders", "1,289", tk.color_warning, 1))
        .child(stat_card(tk, "Growth", "12.5%", tk.color_info, 1))
        .into_node());
}

fn sec_typography(tk: &DesignTokens, out: &mut Vec<WidgetNode>) {
    out.push(heading(tk, "🔤 Typography — Label").into_node());
    out.push(row(36.0)
        .child(Label::new("12px Small", tk.color_text).font_size(12.0))
        .child(Label::new("14px Default", tk.color_text).font_size(14.0))
        .child(Label::new("20px Large", tk.color_text).font_size(20.0))
        .into_node());
    out.push(row(28.0)
        .child(Label::new("Primary", tk.color_primary).font_size(14.0))
        .child(Label::new("Secondary", tk.color_text_secondary).font_size(14.0))
        .child(Label::new("Tertiary", tk.color_text_tertiary).font_size(14.0))
        .child(Label::new("Quaternary", tk.color_text_quaternary).font_size(14.0))
        .into_node());
}

fn sec_buttons(tk: &DesignTokens, out: &mut Vec<WidgetNode>) {
    out.push(heading(tk, "🔘 Buttons — 5 variants × 3 sizes").into_node());
    for (label, h, sz) in [("Small", 28.0, ButtonSize::Small), ("Middle", 36.0, ButtonSize::Middle), ("Large", 44.0, ButtonSize::Large)] {
        out.push(sub(tk, label).into_node());
        out.push(row(h)
            .child(Button::new("Primary").primary().size(sz))
            .child(Button::new("Default").size(sz))
            .child(Button::new("Dashed").dashed().size(sz))
            .child(Button::new("Text").text().size(sz))
            .child(Button::new("Link").link().size(sz))
            .into_node());
    }
    out.push(sub(tk, "Button Group — Save + Cancel").into_node());
    out.push(row(44.0)
        .child(Button::new("Save").primary().size(ButtonSize::Middle))
        .child(Button::new("Cancel").size(ButtonSize::Middle))
        .into_node());
}

fn sec_inputs(tk: &DesignTokens, out: &mut Vec<WidgetNode>) {
    out.push(heading(tk, "⌨️ Inputs").into_node());
    out.push(sub(tk, "Small / Middle / Large").into_node());
    out.push(row(28.0)
        .child(Input::new("Small...").size(InputSize::Small))
        .child(Input::new("Middle...").size(InputSize::Middle))
        .child(Input::new("Large...").size(InputSize::Large))
        .into_node());
    out.push(sub(tk, "Form Row — Input + Button").into_node());
    out.push(row(40.0)
        .child(Input::new("Enter your email...").size(InputSize::Middle))
        .child(Button::new("Subscribe").primary().size(ButtonSize::Middle))
        .into_node());
}

fn sec_data_display(tk: &DesignTokens, out: &mut Vec<WidgetNode>) {
    out.push(heading(tk, "📋 Data Display").into_node());

    // Card — elevation
    out.push(sub(tk, "Card — Elevation 0 ~ 3").into_node());
    let c4 = (iw() - 24.0) / 4.0;
    out.push(Space::new().size(SpaceSize::Middle).width(iw()).height(130.0)
        .direction(FlexDirection::Row).align(AlignItems::Stretch)
        .child(Card::new().title("Elevation 0").elevation(0).bordered(true).size(c4, 120.0)
            .child(Label::new("Bordered, no shadow", tk.color_text_tertiary).font_size(12.0)))
        .child(Card::new().title("Elevation 1").elevation(1).size(c4, 120.0)
            .child(Label::new("Soft shadow", tk.color_text_tertiary).font_size(12.0)))
        .child(Card::new().title("Elevation 2").elevation(2).size(c4, 120.0)
            .child(Label::new("Medium shadow + accent", tk.color_text_tertiary).font_size(12.0)))
        .child(Card::new().title("Elevation 3").elevation(3).size(c4, 120.0)
            .child(Label::new("Deep shadow + accent", tk.color_text_tertiary).font_size(12.0)))
        .into_node());

    // Card — hoverable
    out.push(sub(tk, "Card — Hoverable & Plain").into_node());
    out.push(row(100.0).align(AlignItems::Stretch)
        .child(Card::new().title("Hoverable Card").elevation(1).hoverable().size((iw() - 10.0) / 2.0, 90.0)
            .child(Label::new("Hover to lighten", tk.color_text_tertiary).font_size(12.0)))
        .child(Card::new().title("Plain Card").elevation(1).size((iw() - 10.0) / 2.0, 90.0)
            .child(Label::new("No hover effect", tk.color_text_tertiary).font_size(12.0)))
        .into_node());

    // Card — empty
    out.push(sub(tk, "Card — Empty State").into_node());
    out.push(Card::new().title("Empty List").elevation(1).size(iw(), 80.0)
        .child(Label::new("No data available", tk.color_text_quaternary).font_size(14.0))
        .into_node());

    // ProgressBar
    out.push(sub(tk, "ProgressBar — Determinate").into_node());
    for (pct, lbl) in [(0.25, "25%"), (0.50, "50%"), (0.75, "75%"), (1.00, "100%")] {
        out.push(col(32.0)
            .child(Label::new(lbl, tk.color_text_tertiary).font_size(11.0))
            .child(ProgressBar::new().progress(pct).track_color(tk.color_fill_tertiary))
            .into_node());
    }

    out.push(sub(tk, "ProgressBar — Indeterminate").into_node());
    out.push(col(28.0).child(ProgressBar::new().indeterminate()).into_node());

    // Tags — use Container dir=Row directly because Space::child() needs impl Widget
    out.push(sub(tk, "Tags").into_node());
    fn tag_node(text: &str, bg: Color, fg: Color) -> WidgetNode {
        tree! { Container::new().bg(bg).rounded(4.0).size(text.len() as f32 * 8.0 + 16.0, 24.0) => [
            Label::new(text, fg).font_size(12.0),
        ]}
    }
    out.push(tree! { Container::new().size(iw(), 36.0).dir(FlexDirection::Row) => [
        tag_node("Default", tk.color_fill, tk.color_text),
        tag_node("Primary", tk.color_primary_bg, tk.color_primary),
        tag_node("Success", tk.color_success_bg, tk.color_success),
        tag_node("Warning", tk.color_warning_bg, tk.color_warning),
        tag_node("Error", tk.color_error_bg, tk.color_error),
    ]}.into_node());

    // Table — Grid 实现
    out.push(sub(tk, "Table — 4 columns, alternating bg").into_node());
    let tbl_inner = |bg| tree! { Grid::new().columns(vec![uix::graphics::GridTrack::Fr(1.0); 4]).size(iw() - 2.0, 32.0) => [
        tree! { Container::new().bg(bg).size(100.0, 32.0) => [Label::new("Alice", tk.color_text).font_size(12.0)]},
        tree! { Container::new().bg(bg).size(100.0, 32.0) => [Label::new("alice@ex.com", tk.color_text_secondary).font_size(12.0)]},
        tree! { Container::new().bg(bg).size(100.0, 32.0) => [Label::new("Designer", tk.color_text).font_size(12.0)]},
        tree! { Container::new().bg(bg).size(100.0, 32.0) => [Label::new("Active", tk.color_success).font_size(12.0)]},
    ]};
    out.push(tree! { Container::new().size(iw(), 124.0).bg(tk.color_bg_elevated).rounded(tk.border_radius_lg)
        .dir(FlexDirection::Column).pad(EdgeInsets::uniform(1.0)).border(tk.color_border, 1.0) => [
        tree! { Grid::new().columns(vec![uix::graphics::GridTrack::Fr(1.0); 4]).size(iw() - 2.0, 28.0) => [
            tree! { Container::new().bg(tk.color_fill).size(100.0, 28.0) => [Label::new("Name", tk.color_text).font_size(12.0)]},
            tree! { Container::new().bg(tk.color_fill).size(100.0, 28.0) => [Label::new("Email", tk.color_text).font_size(12.0)]},
            tree! { Container::new().bg(tk.color_fill).size(100.0, 28.0) => [Label::new("Role", tk.color_text).font_size(12.0)]},
            tree! { Container::new().bg(tk.color_fill).size(100.0, 28.0) => [Label::new("Status", tk.color_text).font_size(12.0)]},
        ]},
        tbl_inner(Color::transparent()),
        tbl_inner(tk.color_fill_quaternary),
        tbl_inner(Color::transparent()),
    ]}.into_node());

    // Alert
    out.push(sub(tk, "Alert — semantic bg colors").into_node());
    out.push(row(36.0).align(AlignItems::Stretch)
        .child(Container::new().bg(tk.color_success_bg).rounded(tk.border_radius_sm).size((iw() - 10.0) / 2.0, 32.0))
        .child(Container::new().bg(tk.color_warning_bg).rounded(tk.border_radius_sm).size((iw() - 10.0) / 2.0, 32.0))
        .into_node());
    out.push(row(36.0).align(AlignItems::Stretch)
        .child(Container::new().bg(tk.color_error_bg).rounded(tk.border_radius_sm).size((iw() - 10.0) / 2.0, 32.0))
        .child(Container::new().bg(tk.color_info_bg).rounded(tk.border_radius_sm).size((iw() - 10.0) / 2.0, 32.0))
        .into_node());
}

fn sec_nav(tk: &DesignTokens, out: &mut Vec<WidgetNode>) {
    out.push(heading(tk, "📌 Navigation — NavGroup").into_node());
    out.push(sub(tk, "Sidebar: Navigation::build(); below: NavGroup demo").into_node());
    out.push(tree! { Container::new().size(iw(), 80.0).bg(tk.color_bg_elevated)
        .rounded(tk.border_radius_lg).dir(FlexDirection::Row)
        .pad(EdgeInsets::uniform(8.0)) => [
        tree! { Container::new().size(160.0, 64.0).dir(FlexDirection::Column) => [
            Label::new("🏠 Dashboard", tk.color_primary).font_size(13.0),
            Label::new("⚙️ Settings", tk.color_text_secondary).font_size(13.0),
            Label::new("ℹ️ About", tk.color_text_secondary).font_size(13.0),
        ]},
    ]}.into_node());
}

fn sec_tabs(tk: &DesignTokens, out: &mut Vec<WidgetNode>) {
    out.push(heading(tk, "📑 Tabs").into_node());
    out.push(tree! { Tabs::new().tab("Users", "u").tab("Settings", "s").tab("Analytics", "a")
        .active(0).position(TabPosition::Top).size(iw(), 180.0) => [
        tree! { Container::new().size(iw(), 140.0) => [
            Label::new("Users Panel — manage team members", tk.color_text).font_size(14.0),
            Label::new("Invite, remove, or change roles.", tk.color_text_tertiary).font_size(12.0),
        ]},
        tree! { Container::new().size(iw(), 140.0) => [
            Label::new("Settings Panel — app configuration", tk.color_text).font_size(14.0),
            Label::new("Theme, notifications, privacy.", tk.color_text_tertiary).font_size(12.0),
        ]},
        tree! { Container::new().size(iw(), 140.0) => [
            Label::new("Analytics Panel — usage metrics", tk.color_text).font_size(14.0),
            Label::new("Charts, reports, export options.", tk.color_text_tertiary).font_size(12.0),
        ]},
    ]}.into_node());
}

fn sec_breadcrumb(tk: &DesignTokens, out: &mut Vec<WidgetNode>) {
    out.push(heading(tk, "🥖 Breadcrumb").into_node());
    out.push(sub(tk, "Label with '/' separator").into_node());
    out.push(row(28.0)
        .child(Label::new("Dashboard", tk.color_text_secondary).font_size(12.0))
        .child(Label::new("/", tk.color_text_quaternary).font_size(12.0))
        .child(Label::new("Components", tk.color_text_secondary).font_size(12.0))
        .child(Label::new("/", tk.color_text_quaternary).font_size(12.0))
        .child(Label::new("Table", tk.color_primary).font_size(12.0))
        .into_node());
}

fn sec_layout(tk: &DesignTokens, out: &mut Vec<WidgetNode>) {
    out.push(heading(tk, "🧩 Layout").into_node());

    // Grid — 2col
    out.push(sub(tk, "Grid — 2 columns (1fr 1fr)").into_node());
    out.push(tree! { Grid::two_columns().gap(8.0).pad(EdgeInsets::uniform(4.0)).size(iw(), 70.0) => [
        tree! { Container::new().size(100.0, 60.0).bg(tk.color_primary_bg).rounded(tk.border_radius_sm) => [Label::new("Column 1", tk.color_primary).font_size(13.0)]},
        tree! { Container::new().size(100.0, 60.0).bg(tk.color_success_bg).rounded(tk.border_radius_sm) => [Label::new("Column 2", tk.color_success).font_size(13.0)]},
    ]}.into_node());

    // Grid — 3col
    out.push(sub(tk, "Grid — 3 columns (1fr 1fr 1fr)").into_node());
    out.push(tree! { Grid::three_columns().gap(8.0).pad(EdgeInsets::uniform(4.0)).size(iw(), 70.0) => [
        tree! { Container::new().size(100.0, 60.0).bg(tk.color_primary_bg).rounded(tk.border_radius_sm) => [Label::new("Cell A", tk.color_primary).font_size(13.0)]},
        tree! { Container::new().size(100.0, 60.0).bg(tk.color_warning_bg).rounded(tk.border_radius_sm) => [Label::new("Cell B", tk.color_warning).font_size(13.0)]},
        tree! { Container::new().size(100.0, 60.0).bg(tk.color_success_bg).rounded(tk.border_radius_sm) => [Label::new("Cell C", tk.color_success).font_size(13.0)]},
    ]}.into_node());

    // Grid — custom 1fr 2fr
    out.push(sub(tk, "Grid — Custom (1fr 2fr)").into_node());
    out.push(tree! { Grid::new().columns(vec![uix::graphics::GridTrack::Fr(1.0), uix::graphics::GridTrack::Fr(2.0)])
        .gap(8.0).pad(EdgeInsets::uniform(4.0)).size(iw(), 70.0) => [
        tree! { Container::new().size(100.0, 60.0).bg(tk.color_primary_bg).rounded(tk.border_radius_sm) => [Label::new("1fr", tk.color_primary).font_size(13.0)]},
        tree! { Container::new().size(200.0, 60.0).bg(tk.color_info_bg).rounded(tk.border_radius_sm) => [Label::new("2fr", tk.color_info).font_size(13.0)]},
    ]}.into_node());

    // Space variants
    out.push(sub(tk, "Space — Small (8px) & Middle (16px) gaps").into_node());
    for (sz, label, h) in [(SpaceSize::Small, "Small (8px)", 32.0), (SpaceSize::Middle, "Middle (16px)", 56.0)] {
        out.push(Space::new().size(sz).width(iw()).height(h)
            .direction(FlexDirection::Row).align(AlignItems::Center)
            .child(Label::new(label, tk.color_text_tertiary).font_size(11.0))
            .child(Button::new("A").size(ButtonSize::Small))
            .child(Button::new("B").size(ButtonSize::Small))
            .child(Button::new("C").size(ButtonSize::Small))
            .into_node());
    }

    // Divider vertical
    out.push(sub(tk, "Divider — Vertical").into_node());
    out.push(row(36.0)
        .child(Label::new("Left", tk.color_text).font_size(14.0))
        .child(Divider::new().vertical().color(tk.color_border))
        .child(Label::new("Center", tk.color_text).font_size(14.0))
        .child(Divider::new().vertical().color(tk.color_border))
        .child(Label::new("Right", tk.color_text).font_size(14.0))
        .into_node());

    // Description list
    out.push(sub(tk, "Description List (composite)").into_node());
    out.push(tree! { Container::new().size(iw(), 80.0).bg(tk.color_bg_elevated)
        .rounded(tk.border_radius_lg).dir(FlexDirection::Column).pad(EdgeInsets::uniform(12.0)) => [
        row(20.0).child(Label::new("User Name:", tk.color_text_tertiary).font_size(11.0))
            .child(Label::new("Zhang Wei", tk.color_text).font_size(13.0)),
        row(20.0).child(Label::new("Email:", tk.color_text_tertiary).font_size(11.0))
            .child(Label::new("zhang@ex.com", tk.color_text).font_size(13.0)),
        row(20.0).child(Label::new("Role:", tk.color_text_tertiary).font_size(11.0))
            .child(Label::new("Admin", tk.color_primary).font_size(13.0)),
    ]}.into_node());

    // List + Avatar
    out.push(sub(tk, "List + Avatar (composite)").into_node());
    out.push(tree! { Container::new().size(iw(), 120.0).bg(tk.color_bg_elevated)
        .rounded(tk.border_radius_lg).dir(FlexDirection::Column).pad(EdgeInsets::uniform(8.0)) => [
        row(32.0).child(Container::new().size(28.0, 28.0).bg(tk.color_primary_bg).rounded(14.0))
            .child(Label::new("  Alice  —  Designer", tk.color_text).font_size(13.0)),
        row(32.0).child(Container::new().size(28.0, 28.0).bg(tk.color_success_bg).rounded(14.0))
            .child(Label::new("  Bob  —  Developer", tk.color_text).font_size(13.0)),
        row(32.0).child(Container::new().size(28.0, 28.0).bg(tk.color_warning_bg).rounded(14.0))
            .child(Label::new("  Carol  —  Manager", tk.color_text).font_size(13.0)),
    ]}.into_node());
}

fn sec_colors(tk: &DesignTokens, out: &mut Vec<WidgetNode>) {
    out.push(heading(tk, "🎨 Theme Colors — Semantic Tokens").into_node());
    out.push(sub(tk, "Semantic background colors:").into_node());
    out.push(row(44.0)
        .child(Container::new().size(80.0, 32.0).bg(tk.color_primary_bg).rounded(tk.border_radius_sm))
        .child(Container::new().size(80.0, 32.0).bg(tk.color_success_bg).rounded(tk.border_radius_sm))
        .child(Container::new().size(80.0, 32.0).bg(tk.color_warning_bg).rounded(tk.border_radius_sm))
        .child(Container::new().size(80.0, 32.0).bg(tk.color_error_bg).rounded(tk.border_radius_sm))
        .child(Container::new().size(80.0, 32.0).bg(tk.color_info_bg).rounded(tk.border_radius_sm))
        .into_node());
    out.push(row(28.0)
        .child(Label::new("Primary", tk.color_primary).font_size(11.0))
        .child(Label::new("Success", tk.color_success).font_size(11.0))
        .child(Label::new("Warning", tk.color_warning).font_size(11.0))
        .child(Label::new("Error", tk.color_error).font_size(11.0))
        .child(Label::new("Info", tk.color_info).font_size(11.0))
        .into_node());
    out.push(sub(tk, "Fill levels:").into_node());
    out.push(row(28.0)
        .child(Container::new().size(60.0, 20.0).bg(tk.color_fill).rounded(tk.border_radius_sm))
        .child(Container::new().size(60.0, 20.0).bg(tk.color_fill_secondary).rounded(tk.border_radius_sm))
        .child(Container::new().size(60.0, 20.0).bg(tk.color_fill_tertiary).rounded(tk.border_radius_sm))
        .child(Container::new().size(60.0, 20.0).bg(tk.color_fill_quaternary).rounded(tk.border_radius_sm))
        .into_node());
}

fn sec_custom(tk: &DesignTokens, out: &mut Vec<WidgetNode>) {
    out.push(heading(tk, "✨ Custom Widget & Modal").into_node());
    out.push(sub(tk, "Counter (define_widget! example)").into_node());
    out.push(Label::new("Click to increment:", tk.color_text_tertiary).font_size(13.0).into_node());
    out.push(Counter { count: 0 }.into_node());
    out.push(sub(tk, "Modal Dialog").into_node());
    out.push(tree! { Container::new().size(iw(), 60.0).bg(tk.color_bg_elevated)
        .rounded(tk.border_radius_lg).dir(FlexDirection::Row).pad(EdgeInsets::uniform(12.0)) => [
        Label::new("🪟", tk.color_text).font_size(24.0),
        Container::new().size(12.0, 0.0),
        tree! { Container::new().size(iw() - 80.0, 36.0).dir(FlexDirection::Column) => [
            Label::new("Modal::new(\"Title\").show()", tk.color_text).font_size(14.0),
            Label::new("Use .open() / .close() at runtime.", tk.color_text_tertiary).font_size(12.0),
        ]},
    ]}.into_node());
}

// ════════════════════════════════════════════════════════════════════════════
// UI 组装
// ════════════════════════════════════════════════════════════════════════════

fn build_showcase(tree: &mut WidgetTree) {
    let tk = DesignTokens::antd_light();
    let mut content: Vec<WidgetNode> = Vec::new();

    sec_dashboard(&tk, &mut content);
    content.push(Divider::new().color(tk.color_border).with_text("Typography").orientation(DividerOrientation::Left).into_node());
    sec_typography(&tk, &mut content);
    content.push(Divider::new().color(tk.color_border).with_text("Buttons").orientation(DividerOrientation::Left).into_node());
    sec_buttons(&tk, &mut content);
    content.push(Divider::new().color(tk.color_border).with_text("Inputs").orientation(DividerOrientation::Left).into_node());
    sec_inputs(&tk, &mut content);
    content.push(Divider::new().color(tk.color_border).with_text("Data Display").orientation(DividerOrientation::Left).into_node());
    sec_data_display(&tk, &mut content);
    content.push(Divider::new().color(tk.color_border_secondary).into_node());
    sec_nav(&tk, &mut content);
    content.push(Divider::new().color(tk.color_border_secondary).into_node());
    sec_tabs(&tk, &mut content);
    content.push(Divider::new().color(tk.color_border_secondary).into_node());
    sec_breadcrumb(&tk, &mut content);
    content.push(Divider::new().color(tk.color_border_secondary).into_node());
    sec_layout(&tk, &mut content);
    content.push(Divider::new().color(tk.color_border_secondary).into_node());
    sec_colors(&tk, &mut content);
    content.push(Divider::new().color(tk.color_border_secondary).into_node());
    sec_custom(&tk, &mut content);
    content.push(Divider::new().color(tk.color_border_secondary).into_node());
    content.push(Label::new("UIX Framework — Native Rust UI, Ant Design 5 Theming", tk.color_text_quaternary).font_size(11.0).into_node());

    tree.build(tree! {
        // 根容器初始尺寸 GW×GH，Resize 事件会覆盖
        Container::new().size(GW as f32, GH as f32).bg(tk.color_bg_layout).dir(FlexDirection::Row) => [
            Navigation::new("UIX Ant Design")
                .item(" Dashboard", "📊").item(" Typography", "🔤").item(" Buttons", "🔘")
                .item(" Inputs", "⌨️").item(" Data Display", "📋").item(" Navigation", "📌")
                .item(" Tabs", "📑").item(" Breadcrumb", "🥖").item(" Layout", "🧩")
                .item(" Colors", "🎨").item(" Custom", "✨")
                .active_index(0).width(SB).height(GH as f32).build(&tk),
            // 内容区 flex-grow=1 填充剩余宽度
            WidgetNode::new(
                Box::new(Container::new().bg(tk.color_bg_container).dir(FlexDirection::Column)
                    .flex_grow(1.0)),
                vec![
                    // Header: 固定高度44px，宽度由flex Stretch自动填充
                    tree! { Container::new().bg(tk.color_bg_elevated).dir(FlexDirection::Row)
                        .size(0.0, 44.0) => [
                        Label::new("  Ant Design 5 Component Showcase", tk.color_text)
                            .font_size(16.0).size(600.0, 44.0),
                        Container::new().flex_grow(1.0),  // 弹性 spacer
                        Label::new("UIX v0.1.0", tk.color_text_quaternary)
                            .font_size(12.0).size(100.0, 44.0),
                    ]},
                    // ScrollView flex-grow=1 填充剩余高度
                    WidgetNode::new(
                        Box::new(ScrollView::new(ScrollDirection::Vertical).flex_grow(1.0)),
                        vec![WidgetNode::new(
                            // 不设固定宽 → ScrollView 拉伸到视口宽
                            Box::new(Container::new().size(0.0, 4000.0)
                                .dir(FlexDirection::Column)),
                            content,
                        )],
                    ),
                ],
            ),
        ]
    });
}

// ════════════════════════════════════════════════════════════════════════════
// 入口
// ════════════════════════════════════════════════════════════════════════════

pub fn run_gui_demo() {
    let mut tree = WidgetTree::new();
    build_showcase(&mut tree);

    let mut app = App::new();
    app.title("UIX — Ant Design 5 Component Showcase");
    app.run_widget(
        &mut tree, GW, GH, map_ui_event,
        |ev: &UiEvent| -> bool {
            if let UiEventType::KeyDown = ev.type_ {
                if let UiEventPayload::Key(ref d) = ev.payload {
                    return d.key == KeyCode::Escape;
                }
            }
            false
        },
    );
}
