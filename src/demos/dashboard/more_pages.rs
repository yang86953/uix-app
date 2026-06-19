//! 组件展示页面（导航、标签页、面包屑）。
//!
//! 其余页面（布局、主题色、自定义组件）在 `pages_extra.rs` 中。

use super::{INNER_W, row, sub, wrap_page};
use uix::graphics::FlexDirection;
use uix::ui::theme::DesignTokens;
use uix::ui::widget::WidgetNode;
use uix::ui::{
    Breadcrumb, BreadcrumbItem, Button, ButtonSize, Container, Dropdown,
    IntoWidgetNode, Label, Space, SpaceSize, TabPosition, Tabs,
};

// ── Page 6: Navigation ──

pub fn page_nav(tk: &DesignTokens) -> WidgetNode {
    let mut out = Vec::new();
    out.push(
        Space::new()
            .size(SpaceSize::Small)
            .width(INNER_W)
            .height(12.0)
            .into_node(),
    );
    out.push(sub(tk, "Dropdown — 下拉菜单").into_node());
    out.push(
        row(36.0)
            .child(Dropdown::new("Actions").items(vec!["Edit", "Copy", "Delete", "Export"]))
            .into_node(),
    );
    out.push(sub(tk, "侧边栏预览 (Navigation::build)").into_node());
    out.push(
        uix::tree! { Container::new().size(INNER_W, 200.0).bg(tk.color_bg_elevated)
            .rounded(tk.border_radius_lg).dir(FlexDirection::Row)
            .pad(uix::base::EdgeInsets::uniform(8.0)) => [
            uix::tree! { Container::new().size(160.0, 180.0).dir(FlexDirection::Column) => [
                Label::new("  Dashboard").color(tk.color_primary).font_size(13.0),
                Space::new().size(SpaceSize::Small).width(160.0).height(24.0),
                Label::new("  Settings").color(tk.color_text_secondary).font_size(13.0),
                Space::new().size(SpaceSize::Small).width(160.0).height(24.0),
                Label::new("  About").color(tk.color_text_secondary).font_size(13.0),
            ]},
        ]}
        .into_node(),
    );
    wrap_page(out)
}

// ── Page 7: Tabs ──

pub fn page_tabs(tk: &DesignTokens) -> WidgetNode {
    let mut out = Vec::new();
    out.push(
        Space::new()
            .size(SpaceSize::Small)
            .width(INNER_W)
            .height(12.0)
            .into_node(),
    );
    out.push(uix::tree! { Tabs::new().tab("Users", "u").tab("Settings", "s").tab("Analytics", "a")
        .active(0).position(TabPosition::Top).size(INNER_W, 240.0) => [
        uix::tree! { Container::new().size(INNER_W, 200.0) => [
            Label::new("Users Panel — manage team members").color(tk.color_text).font_size(14.0),
            Label::new("Invite, remove, or change roles.").color(tk.color_text_tertiary).font_size(12.0),
            Button::new("+ Invite User").primary().size(ButtonSize::Small),
        ]},
        uix::tree! { Container::new().size(INNER_W, 200.0) => [
            Label::new("Settings Panel — app configuration").color(tk.color_text).font_size(14.0),
            Label::new("Theme, notifications, privacy.").color(tk.color_text_tertiary).font_size(12.0),
        ]},
        uix::tree! { Container::new().size(INNER_W, 200.0) => [
            Label::new("Analytics Panel — usage metrics").color(tk.color_text).font_size(14.0),
            Label::new("Charts, reports, export options.").color(tk.color_text_tertiary).font_size(12.0),
        ]},
    ]}.into_node());
    wrap_page(out)
}

// ── Page 8: Breadcrumb ──

pub fn page_breadcrumb(tk: &DesignTokens) -> WidgetNode {
    let mut out = Vec::new();
    out.push(
        Space::new()
            .size(SpaceSize::Small)
            .width(INNER_W)
            .height(12.0)
            .into_node(),
    );
    out.push(sub(tk, "面包屑组件").into_node());
    out.push(
        row(28.0)
            .child(
                Breadcrumb::new()
                    .item(BreadcrumbItem::new("首页"))
                    .item(BreadcrumbItem::new("组件"))
                    .item(BreadcrumbItem::new("面包屑").active()),
            )
            .into_node(),
    );
    out.push(sub(tk, "内联 / 分隔符样式").into_node());
    out.push(
        row(28.0)
            .child(
                Label::new("Dashboard")
                    .color(tk.color_text_secondary)
                    .font_size(12.0),
            )
            .child(
                Label::new("/")
                    .color(tk.color_text_quaternary)
                    .font_size(12.0),
            )
            .child(
                Label::new("Components")
                    .color(tk.color_text_secondary)
                    .font_size(12.0),
            )
            .child(
                Label::new("/")
                    .color(tk.color_text_quaternary)
                    .font_size(12.0),
            )
            .child(
                Label::new("Breadcrumb")
                    .color(tk.color_primary)
                    .font_size(12.0),
            )
            .into_node(),
    );
    wrap_page(out)
}
