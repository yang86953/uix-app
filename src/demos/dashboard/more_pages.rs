use super::widgets::{BounceBall, Counter, PulseRing};
use super::{iw, row, sub, wrap_page};
use uix::graphics::{AlignItems, FlexDirection};
use uix::ui::theme::DesignTokens;
use uix::ui::widget::WidgetNode;
use uix::ui::{
    AutoComplete, Breadcrumb, BreadcrumbItem, Button, ButtonSize, Calendar, Collapse, CollapsePanel,
    Container, Descriptions, DescriptionsItem, Divider, Dropdown, Form, Grid, IntoWidgetNode,
    Label, List, Menu, MenuItem, MenuMode, Pagination, Result, ResultType, Segmented,
    Space, SpaceSize, Step, StepStatus, Steps, TabPosition, Tabs, Timeline, TimelineItem, Tree,
    TreeNode, Typography,
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

// ── Page 11: Custom Widgets & New Components ──

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
    out.push(sub(tk, "Typography — 排版").into_node());
    out.push(uix::tree! { Container::new().size(iw(), 120.0).bg(uix::graphics::Color::from_rgba(255, 200, 200, 120)).dir(FlexDirection::Column).border(uix::graphics::Color::from_rgba(255, 0, 0, 200), 2.0)
        .pad(uix::base::EdgeInsets::uniform(4.0)) => [
        uix::ui::Typography::heading("Heading 1 - 标题一", 1).into_node(),
        uix::ui::Typography::heading("Heading 2 - 标题二", 2).into_node(),
        uix::ui::Typography::heading("Heading 3 - 标题三", 3).into_node(),
        uix::ui::Typography::paragraph("这是段落文本，用于展示正文内容。Typography 支持 h1-h5、段落、行内文本等多种排版。").into_node(),
    ]}.into_node());
    out.push(sub(tk, "Steps — 步骤条").into_node());
    out.push(uix::tree! { Container::new().size(iw(), 90.0).bg(uix::graphics::Color::from_rgba(255, 200, 200, 120)).dir(FlexDirection::Column).border(uix::graphics::Color::from_rgba(255, 0, 0, 200), 2.0).pad(uix::base::EdgeInsets::uniform(4.0)) => [
        uix::ui::Steps::new(vec![
            uix::ui::Step::new("注册").status(StepStatus::Finish),
            uix::ui::Step::new("验证").status(StepStatus::Process),
            uix::ui::Step::new("完成").status(StepStatus::Wait),
        ]).current(1).into_node(),
    ]}.into_node());
    out.push(sub(tk, "Pagination — 分页").into_node());
    out.push(uix::tree! { Container::new().size(iw(), 40.0).bg(uix::graphics::Color::from_rgba(255, 200, 200, 120)).dir(FlexDirection::Column).border(uix::graphics::Color::from_rgba(255, 0, 0, 200), 2.0).pad(uix::base::EdgeInsets::uniform(4.0)) => [
        uix::ui::Pagination::new(85, 10).into_node(),
    ]}.into_node());
    out.push(sub(tk, "Result — 结果页缩略").into_node());
    out.push(uix::tree! { Container::new().size(iw(), 160.0).dir(FlexDirection::Row).gap(16.0)
        .pad(uix::base::EdgeInsets::uniform(4.0)) => [
        uix::ui::Result::new(ResultType::Success).title("提交成功").into_node(),
        uix::ui::Result::new(ResultType::Error).title("提交失败").into_node(),
    ]}.into_node());
    out.push(sub(tk, "Timeline — 时间线").into_node());
    out.push(uix::tree! { Container::new().size(iw(), 160.0).bg(uix::graphics::Color::from_rgba(255, 200, 200, 120)).dir(FlexDirection::Column).border(uix::graphics::Color::from_rgba(255, 0, 0, 200), 2.0).pad(uix::base::EdgeInsets::uniform(4.0)) => [
        uix::ui::Timeline::new()
            .add(uix::ui::TimelineItem::new("创建项目").description("2024-01-15"))
            .add(uix::ui::TimelineItem::new("完成设计").description("2024-02-20"))
            .add(uix::ui::TimelineItem::new("部署上线").description("2024-03-10"))
            .into_node(),
    ]}.into_node());
    out.push(sub(tk, "Descriptions — 描述列表").into_node());
    out.push(uix::tree! { Container::new().size(iw(), 100.0).bg(uix::graphics::Color::from_rgba(255, 200, 200, 120)).dir(FlexDirection::Column).border(uix::graphics::Color::from_rgba(255, 0, 0, 200), 2.0).pad(uix::base::EdgeInsets::uniform(4.0)) => [
        uix::ui::Descriptions::new().title("用户信息")
            .add(uix::ui::DescriptionsItem::new("姓名", "张三"))
            .add(uix::ui::DescriptionsItem::new("邮箱", "zhang@ex.com"))
            .add(uix::ui::DescriptionsItem::new("角色", "管理员"))
            .column(3).into_node(),
    ]}.into_node());
    out.push(sub(tk, "Tree — 树形控件").into_node());
    out.push(uix::tree! { Container::new().size(iw(), 120.0).bg(uix::graphics::Color::from_rgba(255, 200, 200, 120)).dir(FlexDirection::Column).border(uix::graphics::Color::from_rgba(255, 0, 0, 200), 2.0).pad(uix::base::EdgeInsets::uniform(4.0)) => [
        uix::ui::Tree::new(vec![
            uix::ui::TreeNode::new("根节点", "1")
                .add(uix::ui::TreeNode::new("子节点 A", "1-1"))
                .add(uix::ui::TreeNode::new("子节点 B", "1-2")
                    .add(uix::ui::TreeNode::new("叶子节点", "1-2-1"))),
        ]).into_node(),
    ]}.into_node());
    out.push(sub(tk, "Menu — 横向菜单").into_node());
    out.push(uix::tree! { Container::new().size(iw(), 40.0).bg(uix::graphics::Color::from_rgba(255, 200, 200, 120)).dir(FlexDirection::Column).border(uix::graphics::Color::from_rgba(255, 0, 0, 200), 2.0).pad(uix::base::EdgeInsets::uniform(4.0)) => [
        uix::ui::Menu::new()
            .add_item(uix::ui::MenuItem { key: "home".into(), label: "首页".into(), icon: "".into(), disabled: false })
            .add_item(uix::ui::MenuItem { key: "docs".into(), label: "文档".into(), icon: "".into(), disabled: false })
            .add_item(uix::ui::MenuItem { key: "about".into(), label: "关于".into(), icon: "".into(), disabled: false })
            .mode(MenuMode::Horizontal)
            .active_key("home").into_node(),
    ]}.into_node());
    out.push(sub(tk, "AutoComplete — 自动补全").into_node());
    out.push(uix::tree! { Container::new().size(iw(), 40.0).bg(uix::graphics::Color::from_rgba(255, 200, 200, 120)).dir(FlexDirection::Column).border(uix::graphics::Color::from_rgba(255, 0, 0, 200), 2.0).pad(uix::base::EdgeInsets::uniform(4.0)) => [
        uix::ui::AutoComplete::new()
            .placeholder("输入城市名称...")
            .options(vec!["北京", "上海", "广州", "深圳", "杭州"])
            .into_node(),
    ]}.into_node());
    out.push(sub(tk, "Form + FormItem — 表单").into_node());
    out.push(uix::tree! { Container::new().size(iw(), 130.0).bg(uix::graphics::Color::from_rgba(255, 200, 200, 120)).dir(FlexDirection::Column).border(uix::graphics::Color::from_rgba(255, 0, 0, 200), 2.0).pad(uix::base::EdgeInsets::uniform(4.0)) => [
        uix::ui::Form::new().gap(4.0).into_node(),
    ]}.into_node());
    out.push(sub(tk, "Calendar — 日历").into_node());
    out.push(uix::tree! { Container::new().size(iw(), 240.0).bg(uix::graphics::Color::from_rgba(255, 200, 200, 120)).dir(FlexDirection::Column).border(uix::graphics::Color::from_rgba(255, 0, 0, 200), 2.0).pad(uix::base::EdgeInsets::uniform(4.0)) => [
        uix::ui::Calendar::new().cell_size(30.0).into_node(),
    ]}.into_node());
    out.push(sub(tk, "List — 列表").into_node());
    out.push(uix::tree! { Container::new().size(iw(), 100.0).bg(uix::graphics::Color::from_rgba(255, 200, 200, 120)).dir(FlexDirection::Column).border(uix::graphics::Color::from_rgba(255, 0, 0, 200), 2.0).pad(uix::base::EdgeInsets::uniform(4.0)) => [
        uix::ui::List::new()
            .header("用户列表")
            .items(vec!["Alice - 设计师", "Bob - 开发者", "Carol - 管理者"])
            .footer("共 3 人")
            .into_node(),
    ]}.into_node());
    wrap_page(tk, out)
}
