//! Ant Design 5 全组件展示 — 按分类展示所有 UIX widget 及 Ant Design 典型组合。
//!
//! Run: `cargo run --bin uix-demo`

use uix::app::{map_ui_event, App};
use uix::base::{EdgeInsets, Rect, Size};
use uix::graphics::{Color, GraphicsEngine};
use uix::platform::event::{UiEvent, UiEventPayload, UiEventType};
use uix::platform::types::KeyCode;
use uix::ui::render_context::RenderContext;
use std::sync::Arc;
use uix::ui::theme::{DesignTokens, DynTokens, Theme};
use uix::ui::widget::{EventResult, WidgetCore, WidgetEvent, WidgetNode, WidgetTree};
use uix::ui::widgets::icon::init_lucide_font;
use uix::ui::{
    Alert, AlertType, AlignItems, Avatar, Badge, BarChart, BarData, Breadcrumb, BreadcrumbItem,
    Button, ButtonSize, Card, Checkbox, Collapse, CollapsePanel, Container, Divider, Dropdown, Empty, Icon,
    LineChart, LineData, PieChart, PieData, Popconfirm, Popover, Radio, Rate, Segmented, Select, Skeleton,
    SkeletonShape, Slider, Spin, Switch, Table, TableColumn, Tag, TagColor, Tooltip, TooltipPlacement,
    DividerOrientation, FlexDirection, Grid, Input,
    InputSize, IntoWidgetNode, Label, Navigation, ProgressBar,
    ScrollDirection, ScrollView, SharedActive, Space, SpaceSize, TabPosition, Tabs,
};
use uix::{define_widget, tree};

const GW: i32 = 1100;
const GH: i32 = 740;
const SB: f32 = 200.0;         // sidebar width
fn cw() -> f32 { GW as f32 - SB }
fn iw() -> f32 { cw() - 40.0 }

// ── 辅助构建函数 ──

fn heading_icon(name: &str, label: &str, tk: &DesignTokens) -> WidgetNode {
    tree! { Container::new().dir(FlexDirection::Row) => [
        Icon::new(name).size(20.0),
        Container::new().size(6.0, 0.0),
        Label::new(label).color(tk.color_text).font_size(18.0),
    ]}.into_node()
}
fn sub(tk: &DesignTokens, text: &str) -> Label {
    Label::new(text).color(tk.color_text_secondary).font_size(12.0)
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
        .child(Label::new(value).color(color).font_size(26.0))
        .child(Label::new(title).color(tk.color_text_quaternary).font_size(11.0))
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
        ctx.draw_text(&format!("Count: {}", self.count), uix::base::Point::new(frame.x + 8.0, frame.y + 8.0), color, 14.0);
    }
}

// ── PulseRing — 脉冲动画 ──

define_widget! {
    pub struct PulseRing {
        time: f32,
    }
    @new -> Self { Self { time: 0.0 } }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        Size::new(48.0, 48.0)
    }

    on_update => (&mut self, dt: f32) {
        self.time += dt;
        if self.time > std::f32::consts::TAU { self.time -= std::f32::consts::TAU; }
    }

    needs_continuous_update => (&self) -> bool { true }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let cx = frame.x + frame.w * 0.5;
        let cy = frame.y + frame.h * 0.5;
        let phase = (self.time * 1.5).sin() * 0.5 + 0.5; // 0..1
        let r = 6.0 + phase * 16.0;
        let alpha = (1.0 - phase * 0.6) * 255.0;
        let base = ctx.tokens().color_primary();
        let c = uix::graphics::Color::from_rgba(base.r, base.g, base.b, alpha as u8);
        let eng = ctx.engine();
        eng.stroke_circle(cx, cy, r, c, 3.0);
        if r > 10.0 {
            eng.fill_circle(cx, cy, r * 0.3, c);
        }
    }
}

// ── BounceBall — 弹跳动画 ──

define_widget! {
    pub struct BounceBall {
        time: f32,
    }
    @new -> Self { Self { time: 0.0 } }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        Size::new(200.0, 60.0)
    }

    on_update => (&mut self, dt: f32) {
        self.time += dt;
        if self.time > 2.0 { self.time -= 2.0; }
    }

    needs_continuous_update => (&self) -> bool { true }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let t = self.time / 2.0; // 0..1
        // 弹跳：快速上升，慢速下降
        let bounce = if t < 0.5 {
            1.0 - (t * 2.0).powf(2.0)  // 快速上升
        } else {
            ((t - 0.5) * 2.0 - 1.0).powf(2.0) * -1.0 + 1.0  // 慢速下降
        };
        let cy = frame.y + frame.h - 10.0 - bounce * 40.0;
        let cx = frame.x + frame.w * 0.5;
        let primary = ctx.tokens().color_primary();
        // 影子（根据高度变化大小和透明度）
        let shadow_alpha = (0.3 + bounce * 0.5 * 0.7) * 255.0;
        let shadow_r = 6.0 + bounce * 8.0;
        let shadow_c = uix::graphics::Color::from_rgba(0, 0, 0, shadow_alpha as u8);
        ctx.fill_circle(cx, frame.y + frame.h - 6.0, shadow_r, shadow_c);
        ctx.engine().fill_circle(cx, cy, 10.0, primary);
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Section 构建 — 每个分类一个函数，每个函数不超过 60 行
// ════════════════════════════════════════════════════════════════════════════

fn sec_dashboard(tk: &DesignTokens, out: &mut Vec<WidgetNode>) {
    out.push(heading_icon("chart-bar", " Dashboard — StatCards", tk));
    out.push(Space::new().size(SpaceSize::Small).width(iw()).height(110.0)
        .direction(FlexDirection::Row).align(AlignItems::Stretch)
        .child(stat_card(tk, "Total Users", "12,834", tk.color_primary, 2))
        .child(stat_card(tk, "Revenue", "$8,291", tk.color_success, 1))
        .child(stat_card(tk, "Orders", "1,289", tk.color_warning, 1))
        .child(stat_card(tk, "Growth", "12.5%", tk.color_info, 1))
        .into_node());
}

fn sec_typography(tk: &DesignTokens, out: &mut Vec<WidgetNode>) {
    out.push(heading_icon("type", " Typography — Label", tk));
    out.push(row(36.0)
        .child(Label::new("12px Small").color(tk.color_text).font_size(12.0))
        .child(Label::new("14px Default").color(tk.color_text).font_size(14.0))
        .child(Label::new("20px Large").color(tk.color_text).font_size(20.0))
        .into_node());
    out.push(row(28.0)
        .child(Label::new("Primary").color(tk.color_primary).font_size(14.0))
        .child(Label::new("Secondary").color(tk.color_text_secondary).font_size(14.0))
        .child(Label::new("Tertiary").color(tk.color_text_tertiary).font_size(14.0))
        .child(Label::new("Quaternary").color(tk.color_text_quaternary).font_size(14.0))
        .into_node());
}

fn sec_buttons(tk: &DesignTokens, out: &mut Vec<WidgetNode>) {
    out.push(heading_icon("square", " Buttons — 5 variants × 3 sizes", tk));
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
    out.push(heading_icon("keyboard", " Inputs", tk));
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

    out.push(sub(tk, "Switch & Checkbox").into_node());
    out.push(row(30.0)
        .child(Switch::new().checked(true))
        .child(Switch::new().checked(false))
        .child(Switch::new().checked(true).disabled(true))
        .into_node());
    out.push(row(30.0)
        .child(Checkbox::new("Option A").checked(true))
        .child(Checkbox::new("Option B"))
        .child(Checkbox::new("Option C").disabled(true))
        .into_node());
}

fn sec_data_display(tk: &DesignTokens, out: &mut Vec<WidgetNode>) {
    out.push(heading_icon("table", " Data Display", tk));

    // Card — elevation
    out.push(sub(tk, "Card — Elevation 0 ~ 3").into_node());
    let c4 = (iw() - 24.0) / 4.0;
    out.push(Space::new().size(SpaceSize::Middle).width(iw()).height(130.0)
        .direction(FlexDirection::Row).align(AlignItems::Stretch)
        .child(Card::new().title("Elevation 0").elevation(0).bordered(true).size(c4, 120.0)
            .child(Label::new("Bordered, no shadow").color(tk.color_text_tertiary).font_size(12.0)))
        .child(Card::new().title("Elevation 1").elevation(1).size(c4, 120.0)
            .child(Label::new("Soft shadow").color(tk.color_text_tertiary).font_size(12.0)))
        .child(Card::new().title("Elevation 2").elevation(2).size(c4, 120.0)
            .child(Label::new("Medium shadow + accent").color(tk.color_text_tertiary).font_size(12.0)))
        .child(Card::new().title("Elevation 3").elevation(3).size(c4, 120.0)
            .child(Label::new("Deep shadow + accent").color(tk.color_text_tertiary).font_size(12.0)))
        .into_node());

    // Card — hoverable
    out.push(sub(tk, "Card — Hoverable & Plain").into_node());
    out.push(row(100.0).align(AlignItems::Stretch)
        .child(Card::new().title("Hoverable Card").elevation(1).hoverable().size((iw() - 10.0) / 2.0, 90.0)
            .child(Label::new("Hover to lighten").color(tk.color_text_tertiary).font_size(12.0)))
        .child(Card::new().title("Plain Card").elevation(1).size((iw() - 10.0) / 2.0, 90.0)
            .child(Label::new("No hover effect").color(tk.color_text_tertiary).font_size(12.0)))
        .into_node());

    // Card — empty
    out.push(sub(tk, "Card — Empty State").into_node());
    out.push(Card::new().title("Empty List").elevation(1).size(iw(), 80.0)
        .child(Label::new("No data available").color(tk.color_text_quaternary).font_size(14.0))
        .into_node());

    // Chart — BarChart
    out.push(sub(tk, "BarChart — Monthly Active Users").into_node());
    out.push(BarChart::new()
        .width(iw())
        .height(180.0)
        .show_value(true)
        .data(vec![
            BarData::new("Jan", 420.0, tk.color_primary),
            BarData::new("Feb", 380.0, tk.color_primary),
            BarData::new("Mar", 530.0, tk.color_success),
            BarData::new("Apr", 490.0, tk.color_primary),
            BarData::new("May", 620.0, tk.color_success),
            BarData::new("Jun", 580.0, tk.color_warning),
            BarData::new("Jul", 710.0, tk.color_success),
            BarData::new("Aug", 680.0, tk.color_primary),
        ])
        .into_node());

    out.push(sub(tk, "BarChart — Revenue Distribution (100% stacked)").into_node());
    out.push(BarChart::new()
        .width(iw())
        .height(140.0)
        .max_value(100.0)
        .show_value(true)
        .data(vec![
            BarData::new("Product", 62.0, tk.color_primary),
            BarData::new("Service", 28.0, tk.color_success),
            BarData::new("License", 15.0, tk.color_warning),
            BarData::new("Other",   8.0, tk.color_error),
        ])
        .into_node());

    out.push(sub(tk, "PieChart — Browser Market Share").into_node());
    out.push(tree! { Container::new().size(iw(), 220.0).dir(FlexDirection::Row) => [
        PieChart::new().size(180.0).data(vec![
            PieData::new("Chrome",  65.0, tk.color_primary),
            PieData::new("Firefox", 15.0, tk.color_success),
            PieData::new("Safari",  10.0, tk.color_warning),
            PieData::new("Edge",     8.0, tk.color_error),
            PieData::new("Other",    2.0, tk.color_fill_tertiary),
        ]).into_node(),
        tree! { Container::new().size(20.0, 0.0) },
        PieChart::new().size(180.0).donut(0.45).data(vec![
            PieData::new("Chrome",  65.0, tk.color_primary),
            PieData::new("Firefox", 15.0, tk.color_success),
            PieData::new("Safari",  10.0, tk.color_warning),
            PieData::new("Edge",     8.0, tk.color_error),
            PieData::new("Other",    2.0, tk.color_fill_tertiary),
        ]).into_node(),
    ]}.into_node());

    out.push(sub(tk, "LineChart — CPU Temperature (°C)").into_node());
    out.push(LineChart::new()
        .width(iw())
        .height(160.0)
        .line_color(tk.color_error)
        .show_dots(true)
        .show_grid(true)
        .line_width(2.0)
        .data(vec![
            LineData::new("00:00", 42.0),
            LineData::new("01:00", 44.0),
            LineData::new("02:00", 41.0),
            LineData::new("03:00", 48.0),
            LineData::new("04:00", 55.0),
            LineData::new("05:00", 53.0),
            LineData::new("06:00", 51.0),
            LineData::new("07:00", 49.0),
        ])
        .into_node());

    // ProgressBar
    out.push(sub(tk, "ProgressBar — Determinate").into_node());
    for (pct, lbl) in [(0.25, "25%"), (0.50, "50%"), (0.75, "75%"), (1.00, "100%")] {
        out.push(col(32.0)
            .child(Label::new(lbl).color(tk.color_text_tertiary).font_size(11.0))
            .child(ProgressBar::new().progress(pct).track_color(tk.color_fill_tertiary))
            .into_node());
    }

    out.push(sub(tk, "ProgressBar — Indeterminate").into_node());
    out.push(col(28.0).child(ProgressBar::new().indeterminate()).into_node());

    // Avatar
    out.push(sub(tk, "Avatar").into_node());
    out.push(row(44.0)
        .child(Avatar::new("A").bg(tk.color_primary_bg).text_color(tk.color_primary))
        .child(Avatar::new("B").bg(tk.color_success_bg).text_color(tk.color_success))
        .child(Avatar::new("C").bg(tk.color_warning_bg).text_color(tk.color_warning))
        .child(Avatar::new("D").bg(tk.color_error_bg).text_color(tk.color_error))
        .child(Avatar::new("U").bg(tk.color_primary_bg).text_color(tk.color_primary).size(48.0))
        .into_node());

    // Icons
    out.push(sub(tk, "Icons — Lucide").into_node());
    out.push(row(36.0)
        .child(Icon::new("search").size(18.0))
        .child(Icon::new("home").size(18.0))
        .child(Icon::new("settings").size(18.0))
        .child(Icon::new("user").size(18.0))
        .child(Icon::new("menu").size(18.0))
        .child(Icon::new("bell").size(18.0))
        .child(Icon::new("heart").size(18.0))
        .child(Icon::new("star").size(18.0))
        .child(Icon::new("github").size(18.0))
        .into_node());
    out.push(row(28.0)
        .child(Icon::new("check").size(16.0))
        .child(Icon::new("x").size(16.0))
        .child(Icon::new("plus").size(16.0))
        .child(Icon::new("minus").size(16.0))
        .child(Icon::new("chevron-left").size(16.0))
        .child(Icon::new("chevron-right").size(16.0))
        .child(Icon::new("chevron-down").size(16.0))
        .child(Icon::new("chevron-up").size(16.0))
        .child(Icon::new("edit").size(16.0))
        .child(Icon::new("trash").size(16.0))
        .child(Icon::new("external-link").size(16.0))
        .into_node());

    // Tags — use Container dir=Row directly because Space::child() needs impl Widget
    out.push(sub(tk, "Tags").into_node());
    fn tag_node(text: &str, bg: Color, fg: Color) -> WidgetNode {
        tree! { Container::new().bg(bg).rounded(4.0).size(text.len() as f32 * 8.0 + 16.0, 24.0) => [
            Label::new(text).color(fg).font_size(12.0),
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
        tree! { Container::new().bg(bg).size(100.0, 32.0) => [Label::new("Alice").color(tk.color_text).font_size(12.0)]},
        tree! { Container::new().bg(bg).size(100.0, 32.0) => [Label::new("alice@ex.com").color(tk.color_text_secondary).font_size(12.0)]},
        tree! { Container::new().bg(bg).size(100.0, 32.0) => [Label::new("Designer").color(tk.color_text).font_size(12.0)]},
        tree! { Container::new().bg(bg).size(100.0, 32.0) => [Label::new("Active").color(tk.color_success).font_size(12.0)]},
    ]};
    out.push(tree! { Container::new().size(iw(), 124.0).bg(tk.color_bg_elevated).rounded(tk.border_radius_lg)
        .dir(FlexDirection::Column).pad(EdgeInsets::uniform(1.0)).border(tk.color_border, 1.0) => [
        tree! { Grid::new().columns(vec![uix::graphics::GridTrack::Fr(1.0); 4]).size(iw() - 2.0, 28.0) => [
            tree! { Container::new().bg(tk.color_fill).size(100.0, 28.0) => [Label::new("Name").color(tk.color_text).font_size(12.0)]},
            tree! { Container::new().bg(tk.color_fill).size(100.0, 28.0) => [Label::new("Email").color(tk.color_text).font_size(12.0)]},
            tree! { Container::new().bg(tk.color_fill).size(100.0, 28.0) => [Label::new("Role").color(tk.color_text).font_size(12.0)]},
            tree! { Container::new().bg(tk.color_fill).size(100.0, 28.0) => [Label::new("Status").color(tk.color_text).font_size(12.0)]},
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
    out.push(heading_icon("navigation", " Navigation — NavGroup", tk));
    out.push(sub(tk, "Sidebar: Navigation::build(); below: NavGroup demo").into_node());
    out.push(tree! { Container::new().size(iw(), 80.0).bg(tk.color_bg_elevated)
        .rounded(tk.border_radius_lg).dir(FlexDirection::Row)
        .pad(EdgeInsets::uniform(8.0)) => [
        tree! { Container::new().size(160.0, 64.0).dir(FlexDirection::Column) => [
            Icon::new("home").size(16.0),
            Label::new(" Dashboard").color(tk.color_primary).font_size(13.0),
            Icon::new("settings").size(16.0),
            Label::new(" Settings").color(tk.color_text_secondary).font_size(13.0),
            Icon::new("info").size(16.0),
            Label::new(" About").color(tk.color_text_secondary).font_size(13.0),
        ]},
    ]}.into_node());
}

fn sec_tabs(tk: &DesignTokens, out: &mut Vec<WidgetNode>) {
    out.push(heading_icon("layout", " Tabs", tk));
    out.push(tree! { Tabs::new().tab("Users", "u").tab("Settings", "s").tab("Analytics", "a")
        .active(0).position(TabPosition::Top).size(iw(), 180.0) => [
        tree! { Container::new().size(iw(), 140.0) => [
            Label::new("Users Panel — manage team members").color(tk.color_text).font_size(14.0),
            Label::new("Invite, remove, or change roles.").color(tk.color_text_tertiary).font_size(12.0),
        ]},
        tree! { Container::new().size(iw(), 140.0) => [
            Label::new("Settings Panel — app configuration").color(tk.color_text).font_size(14.0),
            Label::new("Theme, notifications, privacy.").color(tk.color_text_tertiary).font_size(12.0),
        ]},
        tree! { Container::new().size(iw(), 140.0) => [
            Label::new("Analytics Panel — usage metrics").color(tk.color_text).font_size(14.0),
            Label::new("Charts, reports, export options.").color(tk.color_text_tertiary).font_size(12.0),
        ]},
    ]}.into_node());
}

fn sec_breadcrumb(tk: &DesignTokens, out: &mut Vec<WidgetNode>) {
    out.push(heading_icon("list", " Breadcrumb", tk));
    out.push(sub(tk, "Label with '/' separator").into_node());
    out.push(row(28.0)
        .child(Label::new("Dashboard").color(tk.color_text_secondary).font_size(12.0))
        .child(Label::new("/").color(tk.color_text_quaternary).font_size(12.0))
        .child(Label::new("Components").color(tk.color_text_secondary).font_size(12.0))
        .child(Label::new("/").color(tk.color_text_quaternary).font_size(12.0))
        .child(Label::new("Table").color(tk.color_primary).font_size(12.0))
        .into_node());
}

fn sec_layout(tk: &DesignTokens, out: &mut Vec<WidgetNode>) {
    out.push(heading_icon("grid", " Layout", tk));

    // Grid — 2col
    out.push(sub(tk, "Grid — 2 columns (1fr 1fr)").into_node());
    out.push(tree! { Grid::two_columns().gap(8.0).pad(EdgeInsets::uniform(4.0)).size(iw(), 70.0) => [
        tree! { Container::new().size(100.0, 60.0).bg(tk.color_primary_bg).rounded(tk.border_radius_sm) => [Label::new("Column 1").color(tk.color_primary).font_size(13.0)]},
        tree! { Container::new().size(100.0, 60.0).bg(tk.color_success_bg).rounded(tk.border_radius_sm) => [Label::new("Column 2").color(tk.color_success).font_size(13.0)]},
    ]}.into_node());

    // Grid — 3col
    out.push(sub(tk, "Grid — 3 columns (1fr 1fr 1fr)").into_node());
    out.push(tree! { Grid::three_columns().gap(8.0).pad(EdgeInsets::uniform(4.0)).size(iw(), 70.0) => [
        tree! { Container::new().size(100.0, 60.0).bg(tk.color_primary_bg).rounded(tk.border_radius_sm) => [Label::new("Cell A").color(tk.color_primary).font_size(13.0)]},
        tree! { Container::new().size(100.0, 60.0).bg(tk.color_warning_bg).rounded(tk.border_radius_sm) => [Label::new("Cell B").color(tk.color_warning).font_size(13.0)]},
        tree! { Container::new().size(100.0, 60.0).bg(tk.color_success_bg).rounded(tk.border_radius_sm) => [Label::new("Cell C").color(tk.color_success).font_size(13.0)]},
    ]}.into_node());

    // Grid — custom 1fr 2fr
    out.push(sub(tk, "Grid — Custom (1fr 2fr)").into_node());
    out.push(tree! { Grid::new().columns(vec![uix::graphics::GridTrack::Fr(1.0), uix::graphics::GridTrack::Fr(2.0)])
        .gap(8.0).pad(EdgeInsets::uniform(4.0)).size(iw(), 70.0) => [
        tree! { Container::new().size(100.0, 60.0).bg(tk.color_primary_bg).rounded(tk.border_radius_sm) => [Label::new("1fr").color(tk.color_primary).font_size(13.0)]},
        tree! { Container::new().size(200.0, 60.0).bg(tk.color_info_bg).rounded(tk.border_radius_sm) => [Label::new("2fr").color(tk.color_info).font_size(13.0)]},
    ]}.into_node());

    // Space variants
    out.push(sub(tk, "Space — Small (8px) & Middle (16px) gaps").into_node());
    for (sz, label, h) in [(SpaceSize::Small, "Small (8px)", 32.0), (SpaceSize::Middle, "Middle (16px)", 56.0)] {
        out.push(Space::new().size(sz).width(iw()).height(h)
            .direction(FlexDirection::Row).align(AlignItems::Center)
            .child(Label::new(label).color(tk.color_text_tertiary).font_size(11.0))
            .child(Button::new("A").size(ButtonSize::Small))
            .child(Button::new("B").size(ButtonSize::Small))
            .child(Button::new("C").size(ButtonSize::Small))
            .into_node());
    }

    // Divider vertical
    out.push(sub(tk, "Divider — Vertical").into_node());
    out.push(row(36.0)
        .child(Label::new("Left").color(tk.color_text).font_size(14.0))
        .child(Divider::new().vertical().color(tk.color_border))
        .child(Label::new("Center").color(tk.color_text).font_size(14.0))
        .child(Divider::new().vertical().color(tk.color_border))
        .child(Label::new("Right").color(tk.color_text).font_size(14.0))
        .into_node());

    // Description list
    out.push(sub(tk, "Description List (composite)").into_node());
    out.push(tree! { Container::new().size(iw(), 80.0).bg(tk.color_bg_elevated)
        .rounded(tk.border_radius_lg).dir(FlexDirection::Column).pad(EdgeInsets::uniform(12.0)) => [
        row(20.0).child(Label::new("User Name:").color(tk.color_text_tertiary).font_size(11.0))
            .child(Label::new("Zhang Wei").color(tk.color_text).font_size(13.0)),
        row(20.0).child(Label::new("Email:").color(tk.color_text_tertiary).font_size(11.0))
            .child(Label::new("zhang@ex.com").color(tk.color_text).font_size(13.0)),
        row(20.0).child(Label::new("Role:").color(tk.color_text_tertiary).font_size(11.0))
            .child(Label::new("Admin").color(tk.color_primary).font_size(13.0)),
    ]}.into_node());

    // List + Avatar
    out.push(sub(tk, "List + Avatar (composite)").into_node());
    out.push(tree! { Container::new().size(iw(), 120.0).bg(tk.color_bg_elevated)
        .rounded(tk.border_radius_lg).dir(FlexDirection::Column).pad(EdgeInsets::uniform(8.0)) => [
        row(32.0).child(Container::new().size(28.0, 28.0).bg(tk.color_primary_bg).rounded(14.0))
            .child(Label::new("  Alice  —  Designer").color(tk.color_text).font_size(13.0)),
        row(32.0).child(Container::new().size(28.0, 28.0).bg(tk.color_success_bg).rounded(14.0))
            .child(Label::new("  Bob  —  Developer").color(tk.color_text).font_size(13.0)),
        row(32.0).child(Container::new().size(28.0, 28.0).bg(tk.color_warning_bg).rounded(14.0))
            .child(Label::new("  Carol  —  Manager").color(tk.color_text).font_size(13.0)),
    ]}.into_node());
}

fn sec_phase3(tk: &DesignTokens, out: &mut Vec<WidgetNode>) {
    out.push(heading_icon("sliders", " Phase 3 — Skeleton / Select / Dropdown / Table / Popover / Popconfirm", tk));

    out.push(sub(tk, "Skeleton — 骨架屏").into_node());
    out.push(row(40.0)
        .child(Skeleton::new().shape(SkeletonShape::Rect).size(200.0, 16.0))
        .child(Skeleton::new().shape(SkeletonShape::Circle).size(32.0, 32.0))
        .child(Skeleton::new().shape(SkeletonShape::Text).size(120.0, 24.0))
        .into_node());

    out.push(sub(tk, "Select — 下拉选择").into_node());
    out.push(row(36.0)
        .child(Select::new().options(vec!["Option 1", "Option 2", "Option 3", "Option 4"]).selected(2))
        .into_node());

    out.push(sub(tk, "Dropdown — 下拉菜单").into_node());
    out.push(row(36.0)
        .child(Dropdown::new("Actions").items(vec!["Edit", "Copy", "Delete", "Export"]))
        .into_node());

    out.push(sub(tk, "Table — 数据表格").into_node());
    out.push(row(160.0)
        .child(Table::new()
            .columns(vec![
                TableColumn::new("Name", 100.0),
                TableColumn::new("Age", 60.0),
                TableColumn::new("City", 100.0),
                TableColumn::new("Role", 80.0),
            ])
            .rows(vec![
                vec!["Alice".to_string(), "28".to_string(), "Beijing".to_string(), "Dev".to_string()],
                vec!["Bob".to_string(), "35".to_string(), "Shanghai".to_string(), "PM".to_string()],
                vec!["Charlie".to_string(), "42".to_string(), "Shenzhen".to_string(), "QA".to_string()],
                vec!["Diana".to_string(), "31".to_string(), "Guangzhou".to_string(), "Design".to_string()],
            ]))
        .into_node());

    out.push(sub(tk, "Popover / Popconfirm — 弹出").into_node());
    out.push(row(36.0)
        .child(Popover::new("This is popover content.").title("Popover Title"))
        .child(Popconfirm::new().title("Delete this item?"))
        .into_node());
}

fn sec_phase2(tk: &DesignTokens, out: &mut Vec<WidgetNode>) {
    out.push(heading_icon("sliders", " Phase 2 — Collapse / Radio / Segmented / Slider / Rate / Tooltip", tk));

    out.push(sub(tk, "Collapse — 折叠面板").into_node());
    out.push(Space::new().size(SpaceSize::Small).width(iw()).height(140.0)
        .child(Collapse::new()
            .panels(vec![
                CollapsePanel::new("Panel 1: General", "Content for panel 1 goes here.").expanded(),
                CollapsePanel::new("Panel 2: Settings", "Configuration options and preferences."),
                CollapsePanel::new("Panel 3: Advanced", "Advanced settings for power users."),
            ]))
        .into_node());

    out.push(sub(tk, "Radio — 单选组").into_node());
    out.push(row(32.0)
        .child(Radio::new()
            .options(vec!["Apple", "Banana", "Cherry"])
            .selected(1))
        .into_node());

    out.push(sub(tk, "Segmented — 分段器").into_node());
    out.push(row(36.0)
        .child(Segmented::new()
            .options(vec!["Daily", "Weekly", "Monthly", "Yearly"])
            .selected(2))
        .into_node());

    out.push(sub(tk, "Slider — 滑块").into_node());
    out.push(row(30.0)
        .child(Slider::new().range(0.0, 100.0).step(5.0).value(42.0))
        .into_node());

    out.push(sub(tk, "Rate — 评分").into_node());
    out.push(row(30.0)
        .child(Rate::new().value(3))
        .child(Rate::new().count(7).value(5))
        .child(Rate::new().value(2).allow_half())
        .into_node());

    out.push(sub(tk, "Tooltip — 悬浮提示").into_node());
    out.push(row(40.0)
        .child(Tooltip::new("This is a tooltip").placement(TooltipPlacement::Top))
        .child(Tooltip::new("Bottom tooltip").placement(TooltipPlacement::Bottom))
        .child(Tooltip::new("Left side").placement(TooltipPlacement::Left))
        .child(Tooltip::new("Right side").placement(TooltipPlacement::Right))
        .into_node());
}

fn sec_phase1(tk: &DesignTokens, out: &mut Vec<WidgetNode>) {
    out.push(heading_icon("package", " Phase 1 — Tag / Badge / Spin / Alert / Empty", tk));

    out.push(sub(tk, "Tag — 彩色标签").into_node());
    out.push(row(32.0)
        .child(Tag::new("Default").color(TagColor::Default))
        .child(Tag::new("Success").color(TagColor::Success))
        .child(Tag::new("Info").color(TagColor::Info))
        .child(Tag::new("Warning").color(TagColor::Warning))
        .child(Tag::new("Error").color(TagColor::Error))
        .child(Tag::new("Closable").color(TagColor::Info).closable())
        .into_node());

    out.push(sub(tk, "Badge — 徽章计数 & 红点").into_node());
    out.push(row(32.0)
        .child(Badge::new().count(5))
        .child(Badge::new().count(23))
        .child(Badge::new().count(100).max(99))
        .child(Badge::new().dot())
        .into_node());

    out.push(sub(tk, "Breadcrumb — 面包屑").into_node());
    out.push(row(28.0)
        .child(Breadcrumb::new()
            .item(BreadcrumbItem::new("Home"))
            .item(BreadcrumbItem::new("Components"))
            .item(BreadcrumbItem::new("Tag").active()))
        .into_node());

    out.push(sub(tk, "Spin — 加载动画").into_node());
    out.push(row(40.0)
        .child(Spin::new().small())
        .child(Spin::new())
        .child(Spin::new().large())
        .child(Spin::new().color(tk.color_success))
        .child(Spin::new().color(tk.color_warning))
        .child(Spin::new().color(tk.color_error))
        .into_node());

    out.push(sub(tk, "Alert — 警示条").into_node());
    out.push(Space::new().size(SpaceSize::Small).width(iw()).height(120.0)
        .direction(FlexDirection::Column)
        .child(Alert::new("Success: Operation completed").type_(AlertType::Success))
        .child(Alert::new("Info: This is an information message").type_(AlertType::Info))
        .child(Alert::new("Warning: Check your input").type_(AlertType::Warning))
        .child(Alert::new("Error: Something went wrong").type_(AlertType::Error))
        .into_node());

    out.push(sub(tk, "Alert — 带描述").into_node());
    out.push(Space::new().size(SpaceSize::Small).width(iw()).height(90.0)
        .direction(FlexDirection::Column)
        .child(Alert::new("Update available").description("Version 2.0.0 is ready to install").type_(AlertType::Info).closable())
        .child(Alert::new("Connection lost").description("Attempting to reconnect...").type_(AlertType::Warning))
        .into_node());

    out.push(sub(tk, "Empty — 空状态").into_node());
    out.push(row(120.0)
        .child(Empty::new())
        .child(Empty::new().description("No search results").icon("search"))
        .child(Empty::new().description("No messages").icon("mail"))
        .into_node());
}

fn sec_colors(tk: &DesignTokens, out: &mut Vec<WidgetNode>) {
    out.push(heading_icon("palette", " Theme Colors — Semantic Tokens", tk));
    out.push(sub(tk, "Semantic background colors:").into_node());
    out.push(row(44.0)
        .child(Container::new().size(80.0, 32.0).bg(tk.color_primary_bg).rounded(tk.border_radius_sm))
        .child(Container::new().size(80.0, 32.0).bg(tk.color_success_bg).rounded(tk.border_radius_sm))
        .child(Container::new().size(80.0, 32.0).bg(tk.color_warning_bg).rounded(tk.border_radius_sm))
        .child(Container::new().size(80.0, 32.0).bg(tk.color_error_bg).rounded(tk.border_radius_sm))
        .child(Container::new().size(80.0, 32.0).bg(tk.color_info_bg).rounded(tk.border_radius_sm))
        .into_node());
    out.push(row(28.0)
        .child(Label::new("Primary").color(tk.color_primary).font_size(11.0))
        .child(Label::new("Success").color(tk.color_success).font_size(11.0))
        .child(Label::new("Warning").color(tk.color_warning).font_size(11.0))
        .child(Label::new("Error").color(tk.color_error).font_size(11.0))
        .child(Label::new("Info").color(tk.color_info).font_size(11.0))
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
    out.push(heading_icon("settings", " Custom Widget & Animation", tk));

    out.push(sub(tk, "Counter (click to increment)").into_node());
    out.push(Counter { count: 0 }.into_node());

    out.push(sub(tk, "PulseRing — 呼吸脉冲动画").into_node());
    out.push(PulseRing { time: 0.0 }.into_node());

    out.push(sub(tk, "BounceBall — 弹跳动画 + 动态阴影").into_node());
    out.push(BounceBall { time: 0.0 }.into_node());

    out.push(sub(tk, "Modal Dialog").into_node());
    out.push(tree! { Container::new().size(iw(), 60.0).bg(tk.color_bg_elevated)
        .rounded(tk.border_radius_lg).dir(FlexDirection::Row).pad(EdgeInsets::uniform(12.0)) => [
        Icon::new("layout").size(24.0),
        Container::new().size(12.0, 0.0),
        tree! { Container::new().size(iw() - 80.0, 36.0).dir(FlexDirection::Column) => [
            Label::new("Modal::new(\"Title\").show()").color(tk.color_text).font_size(14.0),
            Label::new("Use .open() / .close() at runtime.").color(tk.color_text_tertiary).font_size(12.0),
        ]},
    ]}.into_node());
}

// ════════════════════════════════════════════════════════════════════════════
// UI 组装
// ════════════════════════════════════════════════════════════════════════════

/// 生成内容区 widgets（不含外层布局），并在各 section 前插入 SectionAnchor。
fn build_showcase_content(tk: &DesignTokens, content: &mut Vec<WidgetNode>) {
    /// 插入一个 SectionAnchor 并递增计数器。
    macro_rules! anchor {
        ($content:expr, $counter:expr) => {{
            let mut a = SectionAnchor::new();
            a.section_index = $counter;
            $counter += 1;
            $content.push(WidgetNode::leaf(Box::new(a)));
        }};
    }

    let mut section_idx = 0usize;

    anchor!(content, section_idx);
    sec_dashboard(tk, content);
    content.push(Divider::new().color(tk.color_border).with_text("Typography").orientation(DividerOrientation::Left).into_node());
    anchor!(content, section_idx);
    sec_typography(tk, content);
    content.push(Divider::new().color(tk.color_border).with_text("Buttons").orientation(DividerOrientation::Left).into_node());
    anchor!(content, section_idx);
    sec_buttons(tk, content);
    content.push(Divider::new().color(tk.color_border).with_text("Inputs").orientation(DividerOrientation::Left).into_node());
    anchor!(content, section_idx);
    sec_inputs(tk, content);
    content.push(Divider::new().color(tk.color_border).with_text("Data Display").orientation(DividerOrientation::Left).into_node());
    anchor!(content, section_idx);
    sec_data_display(tk, content);
    content.push(Divider::new().color(tk.color_border_secondary).into_node());
    anchor!(content, section_idx);
    sec_nav(tk, content);
    content.push(Divider::new().color(tk.color_border_secondary).into_node());
    anchor!(content, section_idx);
    sec_tabs(tk, content);
    content.push(Divider::new().color(tk.color_border_secondary).into_node());
    anchor!(content, section_idx);
    sec_breadcrumb(tk, content);
    content.push(Divider::new().color(tk.color_border_secondary).into_node());
    anchor!(content, section_idx);
    sec_layout(tk, content);
    content.push(Divider::new().color(tk.color_border).with_text("Phase 1").orientation(DividerOrientation::Left).into_node());
    anchor!(content, section_idx);
    sec_phase1(tk, content);
    content.push(Divider::new().color(tk.color_border_secondary).into_node());
    anchor!(content, section_idx);
    sec_phase2(tk, content);
    content.push(Divider::new().color(tk.color_border_secondary).into_node());
    anchor!(content, section_idx);
    sec_colors(tk, content);
    content.push(Divider::new().color(tk.color_border_secondary).into_node());
    anchor!(content, section_idx);
    sec_phase3(tk, content);
    content.push(Divider::new().color(tk.color_border_secondary).into_node());
    anchor!(content, section_idx);
    sec_custom(tk, content);
    content.push(Divider::new().color(tk.color_border_secondary).into_node());
    content.push(Label::new("UIX Framework — Native Rust UI, Ant Design 5 Theming").color(tk.color_text_quaternary).font_size(11.0).into_node());
    // 抑制 unused_assignments 警告：anchor 宏会自增到最终值
    let _ = section_idx;
}

// ════════════════════════════════════════════════════════════════════════════
// SectionAnchor — 导航锚点 widget（零尺寸，仅标记 section 索引）
// ════════════════════════════════════════════════════════════════════════════

// ════════════════════════════════════════════════════════════════════════════
// ThemeToggle — 暗色/亮色切换按钮
// ════════════════════════════════════════════════════════════════════════════

define_widget! {
    /// 主题切换按钮（暗色 ↔ 亮色）。
    pub struct ThemeToggle {
        dark: std::cell::Cell<bool>,
    }

    @new -> Self { Self { dark: std::cell::Cell::new(false) } }

    preferred_size => (&self, _eng: Option<&dyn GraphicsEngine>) -> Size {
        Size::new(32.0, 32.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if let WidgetEvent::MouseDown { .. } = event {
            let new = !self.dark.get();
            self.dark.set(new);
            EventResult::Handled
        } else {
            EventResult::NotHandled
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let icon = if self.dark.get() { "☀️" } else { "🌙" };
        let text_color = ctx.tokens().color_text();
        ctx.text_center(icon, frame, text_color, 18.0);
    }
}

// ════════════════════════════════════════════════════════════════════════════
// SectionAnchor — 导航锚点
// ════════════════════════════════════════════════════════════════════════════

define_widget! {
    /// 不可见的导航锚点，用于在 layout 后定位各 section 的 Y 坐标。
    pub struct SectionAnchor {
        pub section_index: usize,
    }

    @new -> Self { Self { section_index: 0 } }

    preferred_size => (&self, _eng: Option<&dyn GraphicsEngine>) -> Size { Size::new(0.0, 0.0) }

    render => (&self, _frame: Rect, _ctx: &mut RenderContext, _tree: &WidgetTree) {}
}

/// 构建完整的 demo widget tree。
fn build_demo_tree(tk: &DesignTokens, _nav_active: &SharedActive) -> WidgetNode {
    let nav = Navigation::new("UIX Ant Design")
        .item(" Dashboard", "chart-bar").item(" Typography", "type").item(" Buttons", "square")
        .item(" Inputs", "edit").item(" Data Display", "table").item(" Navigation", "menu")
        .item(" Tabs", "layout").item(" Breadcrumb", "list").item(" Layout", "grid")
        .item(" Colors", "palette").item(" Custom", "settings")
        .active_index(0).width(SB).height(GH as f32);
    // 用传入的 nav_active_cell 替换 Navigation 内部使用的 Cell
    let nav_node = nav.build(tk);

    let mut content: Vec<WidgetNode> = Vec::new();
    build_showcase_content(tk, &mut content);

    let content_container = WidgetNode::new(
        Box::new(Container::new().bg(tk.color_bg_container).dir(FlexDirection::Column)
            .flex_grow(1.0)),
        vec![
            tree! { Container::new().bg(tk.color_bg_elevated).dir(FlexDirection::Row)
                .size(0.0, 44.0) => [
                Label::new("  Ant Design 5 Component Showcase").color(tk.color_text)
                    .font_size(16.0).size(600.0, 44.0),
                Container::new().flex_grow(1.0),
                ThemeToggle::new(),
                Label::new("UIX v0.1.0").color(tk.color_text_quaternary)
                    .font_size(12.0).size(100.0, 44.0),
            ]},
            WidgetNode::new(
                Box::new(ScrollView::new(ScrollDirection::Vertical).flex_grow(1.0)),
                vec![WidgetNode::new(
                    Box::new(Container::new().size(0.0, 4000.0)
                        .dir(FlexDirection::Column)),
                    content,
                )],
            ),
        ],
    );

    tree! {
        Container::new().size(GW as f32, GH as f32).bg(tk.color_bg_layout).dir(FlexDirection::Row) => [
            nav_node,
            content_container,
        ]
    }
}

pub fn run_gui_demo() {
    let tk = DesignTokens::antd_light();
    let nav_active: SharedActive = std::rc::Rc::new(std::cell::Cell::new(0));
    let root_node = build_demo_tree(&tk, &nav_active);
    let mut tree = WidgetTree::new();
    tree.build(root_node);

    // 记录上一次激活索引，变化时触发滚动
    let prev_active = std::cell::Cell::new(0usize);
    // 动态计算的 section Y 坐标（首次 on_frame 时从 tree 中扫描 SectionAnchor 获取）
    let section_ys: std::cell::RefCell<Vec<f32>> = std::cell::RefCell::new(Vec::new());

    // 使用 DynTokens 实现运行时主题切换（不重建树即可切换暗/亮模式）
    let dyn_tokens = Arc::new(DynTokens::new(DesignTokens::antd_light()));
    let dark_mode = std::cell::Cell::new(false);
    let mut app = App::new();
    app.title("UIX — Ant Design 5 Component Showcase");

    // 创建引擎并运行（与旧版 run_widget 相同逻辑）
    let mut engine = match app.take_engine(GW, GH) {
        Some(e) => e,
        None => return,
    };

    // 加载 Lucide 图标字体（非致命，无字体时图标显示为空白）
    if let Ok(ttf) = std::fs::read("assets/fonts/lucide.ttf") {
        init_lucide_font(&ttf, &mut *engine);
    } else {
        log::warn!("Lucide font not found at assets/fonts/lucide.ttf — icons will be blank");
    }

    let theme_cell = std::cell::RefCell::new(Theme::from_arc(dyn_tokens.clone()));
    let tc_ref = &theme_cell;

    let exit_code = app.run_widget_with_tokens(
        &mut *engine,
        &mut tree,
        GW, GH, tc_ref, map_ui_event,
        |ev: &UiEvent| -> bool {
            if let UiEventType::KeyDown = ev.type_ {
                if let UiEventPayload::Key(ref d) = ev.payload {
                    return d.key == KeyCode::Escape;
                }
            }
            false
        },
        move |tree, _eng| {
            // 检查主题切换按钮状态
            let mut new_dark = false;
            tree.find_by_type_and_modify::<ThemeToggle>(|w| {
                new_dark = w.dark.get();
            });
            if new_dark != dark_mode.get() {
                dark_mode.set(new_dark);
                  // 通过 DynTokens 切换主题（渲染循环每帧从 ctx.tokens() 获取的新颜色自动更新）
                  dyn_tokens.set_mode(new_dark);
                  // 保存 ScrollView 的滚动位置
                  let scrolled_x = std::cell::Cell::new(0.0f32);
                  let scrolled_y = std::cell::Cell::new(0.0f32);
                  tree.find_by_type_and_modify::<ScrollView>(|sv| {
                      scrolled_x.set(sv.scroll_x());
                      scrolled_y.set(sv.scroll_y());
                  });
                  // 重建整棵树以应用新主题（widget 在构建时从 tk 读取颜色值）
                  let new_tk = dyn_tokens.snapshot();
                  let new_root = build_demo_tree(&new_tk, &nav_active);
                  tree.build(new_root);
                  // 恢复滚动位置
                  tree.find_by_type_and_modify::<ScrollView>(|sv| {
                      sv.scroll_to_xy(scrolled_x.get(), scrolled_y.get());
                  });
                  // 同步新树中 ThemeToggle 的状态，避免下一帧又检测到变化而陷入无限循环
                  tree.find_by_type_and_modify::<ThemeToggle>(|w| { w.dark.set(dark_mode.get()); });
                  // 强制全帧 dirty，确保重建后的树在下一帧被渲染
                  tree.mark_full_frame_dirty();
                  *section_ys.borrow_mut() = Vec::new();
              }
            // ── 动态初始化 section Y 坐标 ──
            if section_ys.borrow().is_empty() {
                let mut ys: Vec<(usize, f32)> = tree
                    .find_all_by_type::<SectionAnchor>()
                    .into_iter()
                    .map(|(id, anchor)| (anchor.section_index, tree.get(id).unwrap().frame().y))
                    .collect();
                if ys.len() == 11 {
                    ys.sort_by_key(|(idx, _)| *idx);
                    *section_ys.borrow_mut() = ys.into_iter().map(|(_, y)| y).collect();
                }
            }

            // ── 导航索引变化 → 滚动到对应 section ──
            let active = nav_active.get();
            if active != prev_active.get() {
                prev_active.set(active);
                if let Some(&target_y) = section_ys.borrow().get(active) {
                    tree.find_by_type_and_modify::<ScrollView>(|sv| {
                        sv.scroll_to_xy(0.0, target_y);
                    });
                }
            }
        },
    );

    engine.shutdown();
    std::process::exit(exit_code);
}
