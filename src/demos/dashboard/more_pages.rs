//! 组件展示页面（导航、标签页、面包屑）。
//! 其余页面（布局、主题色、自定义组件）在 `pages_extra.rs` 中。

use super::{INNER_W, PageBuilder, row};
use uix::graphics::FlexDirection;
use uix::ui::theme::DesignTokens;
use uix::ui::widget::WidgetNode;
use uix::ui::{
    Breadcrumb, BreadcrumbItem, Button, ButtonSize, Container, Dropdown,
    Label, Space, SpaceSize, TabPosition, Tabs,
};

// ── Page 6: Navigation ──

pub fn page_nav(tk: &DesignTokens) -> WidgetNode {
    use uix::ui;
    PageBuilder::new(tk)
        .gap()
        .section("Dropdown — 下拉菜单")
        .push(row(36.0).child(Dropdown::new("Actions").items(vec!["Edit", "Copy", "Delete", "Export"])))
        .section("侧边栏预览 (Navigation::build)")
        .push(ui! {
            Container(bg: tk.color_bg_elevated, rounded: tk.border_radius_lg,
                      dir: FlexDirection::Row, pad: uix::base::EdgeInsets::uniform(8.0),
                      w: INNER_W, h: 200.0) {
                Container(dir: FlexDirection::Column, w: 160.0, h: 180.0) {
                    Label("  Dashboard", color: tk.color_primary, font_size: 13.0),
                    Space(size: SpaceSize::Small, width: 160.0, height: 24.0),
                    Label("  Settings", color: tk.color_text_secondary, font_size: 13.0),
                    Space(size: SpaceSize::Small, width: 160.0, height: 24.0),
                    Label("  About", color: tk.color_text_secondary, font_size: 13.0),
                }
            }
        })
        .build()
}

// ── Page 7: Tabs ──

pub fn page_tabs(tk: &DesignTokens) -> WidgetNode {
    PageBuilder::new(tk)
        .gap()
        .push(uix::tree! { Tabs::new().tab("Users", "u").tab("Settings", "s").tab("Analytics", "a")
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
        ]})
        .build()
}

// ── Page 8: Breadcrumb ──

pub fn page_breadcrumb(tk: &DesignTokens) -> WidgetNode {
    PageBuilder::new(tk)
        .gap()
        .section("面包屑组件")
        .push(
            row(28.0)
                .child(Breadcrumb::new()
                    .item(BreadcrumbItem::new("首页"))
                    .item(BreadcrumbItem::new("组件"))
                    .item(BreadcrumbItem::new("面包屑").active())),
        )
        .section("内联 / 分隔符样式")
        .push(
            row(28.0)
                .child(Label::new("Dashboard").color(tk.color_text_secondary).font_size(12.0))
                .child(Label::new("/").color(tk.color_text_quaternary).font_size(12.0))
                .child(Label::new("Components").color(tk.color_text_secondary).font_size(12.0))
                .child(Label::new("/").color(tk.color_text_quaternary).font_size(12.0))
                .child(Label::new("Breadcrumb").color(tk.color_primary).font_size(12.0)),
        )
        .build()
}
