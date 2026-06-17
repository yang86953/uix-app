use super::widgets::{BounceBall, Counter, PulseRing};
use super::{iw, row, sub, wrap_page};
use uix::graphics::{AlignItems, FlexDirection};
use uix::ui::theme::DesignTokens;
use uix::ui::widget::WidgetNode;
use uix::ui::{
    Breadcrumb, BreadcrumbItem, Button, ButtonSize, Collapse, CollapsePanel, Container, Divider,
    Dropdown, Grid, IntoWidgetNode, Label, Segmented, Space, SpaceSize, TabPosition, Tabs,
};

// ── Page 6: Navigation ──

pub fn page_nav(tk: &DesignTokens) -> WidgetNode {
    let mut out = Vec::new();
    out.push(
        Space::new()
            .size(SpaceSize::Small)
            .width(iw())
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
        uix::tree! { Container::new().size(iw(), 200.0).bg(tk.color_bg_elevated)
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
    wrap_page(tk, out)
}

// ── Page 7: Tabs ──

pub fn page_tabs(tk: &DesignTokens) -> WidgetNode {
    let mut out = Vec::new();
    out.push(
        Space::new()
            .size(SpaceSize::Small)
            .width(iw())
            .height(12.0)
            .into_node(),
    );
    out.push(uix::tree! { Tabs::new().tab("Users", "u").tab("Settings", "s").tab("Analytics", "a")
        .active(0).position(TabPosition::Top).size(iw(), 240.0) => [
        uix::tree! { Container::new().size(iw(), 200.0) => [
            Label::new("Users Panel — manage team members").color(tk.color_text).font_size(14.0),
            Label::new("Invite, remove, or change roles.").color(tk.color_text_tertiary).font_size(12.0),
            Button::new("+ Invite User").primary().size(ButtonSize::Small),
        ]},
        uix::tree! { Container::new().size(iw(), 200.0) => [
            Label::new("Settings Panel — app configuration").color(tk.color_text).font_size(14.0),
            Label::new("Theme, notifications, privacy.").color(tk.color_text_tertiary).font_size(12.0),
        ]},
        uix::tree! { Container::new().size(iw(), 200.0) => [
            Label::new("Analytics Panel — usage metrics").color(tk.color_text).font_size(14.0),
            Label::new("Charts, reports, export options.").color(tk.color_text_tertiary).font_size(12.0),
        ]},
    ]}.into_node());
    wrap_page(tk, out)
}

// ── Page 8: Breadcrumb ──

pub fn page_breadcrumb(tk: &DesignTokens) -> WidgetNode {
    let mut out = Vec::new();
    out.push(
        Space::new()
            .size(SpaceSize::Small)
            .width(iw())
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
    wrap_page(tk, out)
}

// ── Page 9: Layout ──

pub fn page_layout(tk: &DesignTokens) -> WidgetNode {
    let mut out = Vec::new();
    out.push(
        Space::new()
            .size(SpaceSize::Small)
            .width(iw())
            .height(12.0)
            .into_node(),
    );

    out.push(sub(tk, "网格 — 2 列 (1fr 1fr)").into_node());
    out.push(uix::tree! { Grid::two_columns().gap(8.0).pad(uix::base::EdgeInsets::uniform(4.0)).size(iw(), 70.0) => [
        uix::tree! { Container::new().size(100.0, 60.0).bg(tk.color_primary_bg).rounded(tk.border_radius_sm) => [Label::new("Column 1").color(tk.color_primary).font_size(13.0)]},
        uix::tree! { Container::new().size(100.0, 60.0).bg(tk.color_success_bg).rounded(tk.border_radius_sm) => [Label::new("Column 2").color(tk.color_success).font_size(13.0)]},
    ]}.into_node());

    out.push(sub(tk, "网格 — 3 列 (1fr 1fr 1fr)").into_node());
    out.push(uix::tree! { Grid::three_columns().gap(8.0).pad(uix::base::EdgeInsets::uniform(4.0)).size(iw(), 70.0) => [
        uix::tree! { Container::new().size(100.0, 60.0).bg(tk.color_primary_bg).rounded(tk.border_radius_sm) => [Label::new("Cell A").color(tk.color_primary).font_size(13.0)]},
        uix::tree! { Container::new().size(100.0, 60.0).bg(tk.color_warning_bg).rounded(tk.border_radius_sm) => [Label::new("Cell B").color(tk.color_warning).font_size(13.0)]},
        uix::tree! { Container::new().size(100.0, 60.0).bg(tk.color_success_bg).rounded(tk.border_radius_sm) => [Label::new("Cell C").color(tk.color_success).font_size(13.0)]},
    ]}.into_node());

    out.push(sub(tk, "网格 — 自定义 (1fr 2fr)").into_node());
    out.push(uix::tree! { Grid::new().columns(vec![uix::graphics::GridTrack::Fr(1.0), uix::graphics::GridTrack::Fr(2.0)])
        .gap(8.0).pad(uix::base::EdgeInsets::uniform(4.0)).size(iw(), 70.0) => [
        uix::tree! { Container::new().size(100.0, 60.0).bg(tk.color_primary_bg).rounded(tk.border_radius_sm) => [Label::new("1fr").color(tk.color_primary).font_size(13.0)]},
        uix::tree! { Container::new().size(200.0, 60.0).bg(tk.color_info_bg).rounded(tk.border_radius_sm) => [Label::new("2fr").color(tk.color_info).font_size(13.0)]},
    ]}.into_node());

    out.push(sub(tk, "间距 — Small (8px) & Middle (16px)").into_node());
    for (sz, label, h) in [
        (SpaceSize::Small, "Small (8px)", 32.0),
        (SpaceSize::Middle, "Middle (16px)", 56.0),
    ] {
        out.push(
            Space::new()
                .size(sz)
                .width(iw())
                .height(h)
                .direction(FlexDirection::Row)
                .align(AlignItems::Center)
                .child(
                    Label::new(label)
                        .color(tk.color_text_tertiary)
                        .font_size(11.0),
                )
                .child(Button::new("甲").size(ButtonSize::Small))
                .child(Button::new("乙").size(ButtonSize::Small))
                .child(Button::new("丙").size(ButtonSize::Small))
                .into_node(),
        );
    }

    out.push(sub(tk, "分割线 — 垂直").into_node());
    out.push(
        row(36.0)
            .child(Label::new("Left").color(tk.color_text).font_size(14.0))
            .child(Divider::new().vertical().color(tk.color_border))
            .child(Label::new("Center").color(tk.color_text).font_size(14.0))
            .child(Divider::new().vertical().color(tk.color_border))
            .child(Label::new("右侧").color(tk.color_text).font_size(14.0))
            .into_node(),
    );

    out.push(sub(tk, "Collapse — 折叠面板").into_node());
    out.push(
        Space::new()
            .size(SpaceSize::Small)
            .width(iw())
            .height(140.0)
            .direction(FlexDirection::Column)
            .align(AlignItems::Stretch)
            .child(Collapse::new().panels(vec![
                CollapsePanel::new("面板 1：常规", "面板 1 的内容。").expanded(),
                CollapsePanel::new("面板 2：设置", "配置选项与偏好设置。"),
                CollapsePanel::new("面板 3：高级", "面向高级用户的设置。"),
            ]))
            .into_node(),
    );

    out.push(sub(tk, "Segmented — 分段器").into_node());
    out.push(
        row(36.0)
            .child(
                Segmented::new()
                    .options(vec!["每日", "每周", "每月", "每年"])
                    .selected(2),
            )
            .into_node(),
    );
    wrap_page(tk, out)
}

// ── Page 10: Colors ──

pub fn page_colors(tk: &DesignTokens) -> WidgetNode {
    let mut out = Vec::new();
    out.push(
        Space::new()
            .size(SpaceSize::Small)
            .width(iw())
            .height(12.0)
            .into_node(),
    );
    out.push(sub(tk, "语义背景色与文字色").into_node());
    out.push(
        row(44.0)
            .child(
                Container::new()
                    .size(80.0, 32.0)
                    .bg(tk.color_primary_bg)
                    .rounded(tk.border_radius_sm),
            )
            .child(
                Container::new()
                    .size(80.0, 32.0)
                    .bg(tk.color_success_bg)
                    .rounded(tk.border_radius_sm),
            )
            .child(
                Container::new()
                    .size(80.0, 32.0)
                    .bg(tk.color_warning_bg)
                    .rounded(tk.border_radius_sm),
            )
            .child(
                Container::new()
                    .size(80.0, 32.0)
                    .bg(tk.color_error_bg)
                    .rounded(tk.border_radius_sm),
            )
            .child(
                Container::new()
                    .size(80.0, 32.0)
                    .bg(tk.color_info_bg)
                    .rounded(tk.border_radius_sm),
            )
            .into_node(),
    );
    out.push(
        row(28.0)
            .child(
                Label::new("Primary")
                    .color(tk.color_primary)
                    .font_size(11.0),
            )
            .child(
                Label::new("Success")
                    .color(tk.color_success)
                    .font_size(11.0),
            )
            .child(
                Label::new("Warning")
                    .color(tk.color_warning)
                    .font_size(11.0),
            )
            .child(Label::new("Error").color(tk.color_error).font_size(11.0))
            .child(Label::new("Info").color(tk.color_info).font_size(11.0))
            .into_node(),
    );
    out.push(sub(tk, "填充色层级").into_node());
    out.push(
        row(28.0)
            .child(
                Container::new()
                    .size(60.0, 20.0)
                    .bg(tk.color_fill)
                    .rounded(tk.border_radius_sm),
            )
            .child(
                Container::new()
                    .size(60.0, 20.0)
                    .bg(tk.color_fill_secondary)
                    .rounded(tk.border_radius_sm),
            )
            .child(
                Container::new()
                    .size(60.0, 20.0)
                    .bg(tk.color_fill_tertiary)
                    .rounded(tk.border_radius_sm),
            )
            .child(
                Container::new()
                    .size(60.0, 20.0)
                    .bg(tk.color_fill_quaternary)
                    .rounded(tk.border_radius_sm),
            )
            .into_node(),
    );
    out.push(sub(tk, "边框色").into_node());
    out.push(
        row(28.0)
            .child(
                Container::new()
                    .size(60.0, 20.0)
                    .bg(tk.color_border)
                    .rounded(tk.border_radius_sm),
            )
            .child(
                Container::new()
                    .size(60.0, 20.0)
                    .bg(tk.color_border_secondary)
                    .rounded(tk.border_radius_sm),
            )
            .into_node(),
    );
    wrap_page(tk, out)
}

// ── Page 11: Custom Widgets & Animation ──

pub fn page_custom(tk: &DesignTokens) -> WidgetNode {
    let mut out = Vec::new();
    out.push(
        Space::new()
            .size(SpaceSize::Small)
            .width(iw())
            .height(12.0)
            .into_node(),
    );
    out.push(sub(tk, "计数器 — 点击递增").into_node());
    out.push(Counter { count: 0 }.into_node());
    out.push(sub(tk, "PulseRing — 呼吸脉冲动画").into_node());
    out.push(PulseRing { time: 0.0 }.into_node());
    out.push(sub(tk, "BounceBall — 弹跳动画 + 动态阴影").into_node());
    out.push(BounceBall { time: 0.0 }.into_node());
    out.push(sub(tk, "描述列表（组合示例）").into_node());
    out.push(uix::tree! { Container::new().size(iw(), 80.0).bg(tk.color_bg_elevated)
        .rounded(tk.border_radius_lg).dir(FlexDirection::Column).pad(uix::base::EdgeInsets::uniform(12.0)) => [
        row(20.0).child(Label::new("用户名：").color(tk.color_text_tertiary).font_size(11.0))
            .child(Label::new("张伟").color(tk.color_text).font_size(13.0)),
        row(20.0).child(Label::new("邮箱：").color(tk.color_text_tertiary).font_size(11.0))
            .child(Label::new("zhang@ex.com").color(tk.color_text).font_size(13.0)),
        row(20.0).child(Label::new("角色：").color(tk.color_text_tertiary).font_size(11.0))
            .child(Label::new("管理员").color(tk.color_primary).font_size(13.0)),
    ]}.into_node());
    out.push(sub(tk, "列表 + 头像（组合示例）").into_node());
    out.push(uix::tree! { Container::new().size(iw(), 120.0).bg(tk.color_bg_elevated)
        .rounded(tk.border_radius_lg).dir(FlexDirection::Column).pad(uix::base::EdgeInsets::uniform(8.0)) => [
        row(32.0).child(Container::new().size(28.0, 28.0).bg(tk.color_primary_bg).rounded(14.0))
            .child(Label::new("  Alice  —  Designer").color(tk.color_text).font_size(13.0)),
        row(32.0).child(Container::new().size(28.0, 28.0).bg(tk.color_success_bg).rounded(14.0))
            .child(Label::new("  Bob  —  Developer").color(tk.color_text).font_size(13.0)),
        row(32.0).child(Container::new().size(28.0, 28.0).bg(tk.color_warning_bg).rounded(14.0))
            .child(Label::new("  Carol  —  Manager").color(tk.color_text).font_size(13.0)),
    ]}.into_node());
    wrap_page(tk, out)
}
