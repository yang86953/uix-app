use super::widgets::{BounceBall, Counter, PulseRing};
use super::{INNER_W, PageBuilder, row};
use uix::ui::layout::{AlignItems, FlexDirection};
use uix::ui::theme::DesignTokens;
use uix::ui::widget::WidgetNode;
use uix::ui::{
    Button, ButtonSize, Carousel, Collapse, CollapsePanel, Container, Divider,
    Grid, Icon, IntoWidgetNode, Label, MenuMode, ResultType, Segmented,
    Space, SpaceSize, Splitter, StepStatus,
};

// ── Page 9: Layout ──

pub fn page_layout(tk: &DesignTokens) -> WidgetNode {
    PageBuilder::new(tk)
        .gap()
        .section("网格 — 2 列 (1fr 1fr)")
        .push(uix::tree! { Grid::two_columns().gap(8.0).pad(uix::base::EdgeInsets::uniform(4.0)).size(INNER_W, 70.0) => [
            uix::tree! { Container::new().size(100.0, 60.0).bg(tk.color_primary_bg).rounded(tk.border_radius_sm) =>
                [Label::new("Column 1").color(tk.color_primary).font_size(13.0)]},
            uix::tree! { Container::new().size(100.0, 60.0).bg(tk.color_success_bg).rounded(tk.border_radius_sm) =>
                [Label::new("Column 2").color(tk.color_success).font_size(13.0)]},
        ]})
        .section("网格 — 3 列 (1fr 1fr 1fr)")
        .push(uix::tree! { Grid::three_columns().gap(8.0).pad(uix::base::EdgeInsets::uniform(4.0)).size(INNER_W, 70.0) => [
            uix::tree! { Container::new().size(100.0, 60.0).bg(tk.color_primary_bg).rounded(tk.border_radius_sm) =>
                [Label::new("Cell A").color(tk.color_primary).font_size(13.0)]},
            uix::tree! { Container::new().size(100.0, 60.0).bg(tk.color_warning_bg).rounded(tk.border_radius_sm) =>
                [Label::new("Cell B").color(tk.color_warning).font_size(13.0)]},
            uix::tree! { Container::new().size(100.0, 60.0).bg(tk.color_success_bg).rounded(tk.border_radius_sm) =>
                [Label::new("Cell C").color(tk.color_success).font_size(13.0)]},
        ]})
        .section("网格 — 自定义 (1fr 2fr)")
        .push(uix::tree! { Grid::new()
            .columns(vec![uix::ui::GridTrack::Fr(1.0), uix::ui::GridTrack::Fr(2.0)])
            .gap(8.0).pad(uix::base::EdgeInsets::uniform(4.0)).size(INNER_W, 70.0) => [
            uix::tree! { Container::new().size(100.0, 60.0).bg(tk.color_primary_bg).rounded(tk.border_radius_sm) =>
                [Label::new("1fr").color(tk.color_primary).font_size(13.0)]},
            uix::tree! { Container::new().size(200.0, 60.0).bg(tk.color_info_bg).rounded(tk.border_radius_sm) =>
                [Label::new("2fr").color(tk.color_info).font_size(13.0)]},
        ]})
        .section("间距 — Small (8px) & Middle (16px)")
        .push({
            let mut sp = Space::new().size(SpaceSize::Small).width(INNER_W).height(0.0)
                .direction(FlexDirection::Column);
            for (sz, label, h) in [
                (SpaceSize::Small, "Small (8px)", 32.0),
                (SpaceSize::Middle, "Middle (16px)", 56.0),
            ] {
                sp = sp.child(
                    Space::new().size(sz).width(INNER_W).height(h)
                        .direction(FlexDirection::Row).align(AlignItems::Center)
                        .child(Label::new(label).color(tk.color_text_tertiary).font_size(11.0))
                        .child(Button::new("甲").size(ButtonSize::Small))
                        .child(Button::new("乙").size(ButtonSize::Small))
                        .child(Button::new("丙").size(ButtonSize::Small)),
                );
            }
            sp
        })
        .section("分割线 — 垂直")
        .push(
            row(36.0)
                .child(Label::new("Left").color(tk.color_text).font_size(14.0))
                .child(Divider::new().vertical().color(tk.color_border))
                .child(Label::new("Center").color(tk.color_text).font_size(14.0))
                .child(Divider::new().vertical().color(tk.color_border))
                .child(Label::new("右侧").color(tk.color_text).font_size(14.0)),
        )
        .section("Collapse — 折叠面板")
        .push(
            Space::new().size(SpaceSize::Small).width(INNER_W).height(140.0)
                .direction(FlexDirection::Column).align(AlignItems::Stretch)
                .child(Collapse::new().panels(vec![
                    CollapsePanel::new("面板 1：常规", "面板 1 的内容。").expanded(),
                    CollapsePanel::new("面板 2：设置", "配置选项与偏好设置。"),
                    CollapsePanel::new("面板 3：高级", "面向高级用户的设置。"),
                ])),
        )
        .section("Segmented — 分段器")
        .push(
            row(36.0)
                .child(Segmented::new().options(vec!["每日", "每周", "每月", "每年"]).selected(2)),
        )
        .build()
}

// ── Page 10: Colors ──

pub fn page_colors(tk: &DesignTokens) -> WidgetNode {
    PageBuilder::new(tk)
        .gap()
        .section("语义背景色与文字色")
        .push(
            row(44.0)
                .child(Container::new().size(80.0, 32.0).bg(tk.color_primary_bg).rounded(tk.border_radius_sm))
                .child(Container::new().size(80.0, 32.0).bg(tk.color_success_bg).rounded(tk.border_radius_sm))
                .child(Container::new().size(80.0, 32.0).bg(tk.color_warning_bg).rounded(tk.border_radius_sm))
                .child(Container::new().size(80.0, 32.0).bg(tk.color_error_bg).rounded(tk.border_radius_sm))
                .child(Container::new().size(80.0, 32.0).bg(tk.color_info_bg).rounded(tk.border_radius_sm)),
        )
        .push(
            row(28.0)
                .child(Label::new("Primary").color(tk.color_primary).font_size(11.0))
                .child(Label::new("Success").color(tk.color_success).font_size(11.0))
                .child(Label::new("Warning").color(tk.color_warning).font_size(11.0))
                .child(Label::new("Error").color(tk.color_error).font_size(11.0))
                .child(Label::new("Info").color(tk.color_info).font_size(11.0)),
        )
        .section("填充色层级")
        .push(
            row(28.0)
                .child(Container::new().size(60.0, 20.0).bg(tk.color_fill).rounded(tk.border_radius_sm))
                .child(Container::new().size(60.0, 20.0).bg(tk.color_fill_secondary).rounded(tk.border_radius_sm))
                .child(Container::new().size(60.0, 20.0).bg(tk.color_fill_tertiary).rounded(tk.border_radius_sm))
                .child(Container::new().size(60.0, 20.0).bg(tk.color_fill_quaternary).rounded(tk.border_radius_sm)),
        )
        .section("边框色")
        .push(
            row(28.0)
                .child(Container::new().size(60.0, 20.0).bg(tk.color_border).rounded(tk.border_radius_sm))
                .child(Container::new().size(60.0, 20.0).bg(tk.color_border_secondary).rounded(tk.border_radius_sm)),
        )
        .build()
}

// ── Page 11: Custom Widgets & New Components ──

pub fn page_custom(tk: &DesignTokens) -> WidgetNode {
    PageBuilder::new(tk)
        .gap()
        .section("图标 (Icon) — 使用 Lucide 字体")
        .push(
            Space::new().size(SpaceSize::Small).width(INNER_W).height(140.0)
                .direction(FlexDirection::Row).wrap(true).align(AlignItems::Start)
                .child(Icon::new("search").size(16.0)).child(Label::new(" search").color(tk.color_text_secondary).font_size(11.0))
                .child(Icon::new("home").size(16.0)).child(Label::new(" home").color(tk.color_text_secondary).font_size(11.0))
                .child(Icon::new("settings").size(16.0)).child(Label::new(" settings").color(tk.color_text_secondary).font_size(11.0))
                .child(Icon::new("user").size(16.0)).child(Label::new(" user").color(tk.color_text_secondary).font_size(11.0))
                .child(Icon::new("menu").size(16.0)).child(Label::new(" menu").color(tk.color_text_secondary).font_size(11.0))
                .child(Icon::new("check").size(16.0)).child(Label::new(" check").color(tk.color_text_secondary).font_size(11.0))
                .child(Icon::new("star").size(16.0)).child(Label::new(" star").color(tk.color_text_secondary).font_size(11.0))
                .child(Icon::new("heart").size(16.0)).child(Label::new(" heart").color(tk.color_text_secondary).font_size(11.0))
                .child(Icon::new("bell").size(16.0)).child(Label::new(" bell").color(tk.color_text_secondary).font_size(11.0))
                .child(Icon::new("mail").size(16.0)).child(Label::new(" mail").color(tk.color_text_secondary).font_size(11.0))
                .child(Icon::new("clock").size(16.0)).child(Label::new(" clock").color(tk.color_text_secondary).font_size(11.0))
                .child(Icon::new("calendar").size(16.0)).child(Label::new(" calendar").color(tk.color_text_secondary).font_size(11.0))
                .child(Icon::new("moon").size(16.0)).child(Label::new(" moon").color(tk.color_text_secondary).font_size(11.0))
                .child(Icon::new("sun").size(16.0)).child(Label::new(" sun").color(tk.color_text_secondary).font_size(11.0))
                .child(Icon::new("edit").size(16.0)).child(Label::new(" edit").color(tk.color_text_secondary).font_size(11.0))
                .child(Icon::new("tag").size(16.0)).child(Label::new(" tag").color(tk.color_text_secondary).font_size(11.0)),
        )
        .section("计数器 — 点击递增")
        .push(Counter { count: 0 })
        .section("PulseRing — 呼吸脉冲动画")
        .push(PulseRing { time: 0.0 })
        .section("BounceBall — 弹跳动画 + 动态阴影")
        .push(BounceBall { time: 0.0 })
        .section("Typography — 排版")
        .push(uix::tree! { Container::new().size(INNER_W, 120.0).dir(FlexDirection::Column)
            .pad(uix::base::EdgeInsets::uniform(4.0)) => [
            uix::ui::Typography::heading("Heading 1 - 标题一", 1).into_node(),
            uix::ui::Typography::heading("Heading 2 - 标题二", 2).into_node(),
            uix::ui::Typography::heading("Heading 3 - 标题三", 3).into_node(),
            uix::ui::Typography::paragraph("这是段落文本，用于展示正文内容。Typography 支持 h1-h5、段落、行内文本等多种排版。").into_node(),
        ]})
        .section("Steps — 步骤条")
        .push(uix::tree! { Container::new().size(INNER_W, 90.0).dir(FlexDirection::Column)
            .pad(uix::base::EdgeInsets::uniform(4.0)) => [
            uix::ui::Steps::new(vec![
                uix::ui::Step::new("注册").status(StepStatus::Finish),
                uix::ui::Step::new("验证").status(StepStatus::Process),
                uix::ui::Step::new("完成").status(StepStatus::Wait),
            ]).current(1).into_node(),
        ]})
        .section("Pagination — 分页")
        .push(uix::tree! { Container::new().size(INNER_W, 40.0).dir(FlexDirection::Column)
            .pad(uix::base::EdgeInsets::uniform(4.0)) => [
            uix::ui::Pagination::new(85, 10).into_node(),
        ]})
        .section("Result — 结果页缩略")
        .push(uix::tree! { Container::new().size(INNER_W, 160.0).dir(FlexDirection::Row).gap(16.0)
            .pad(uix::base::EdgeInsets::uniform(4.0)) => [
            uix::ui::Result::new(ResultType::Success).title("提交成功").into_node(),
            uix::ui::Result::new(ResultType::Error).title("提交失败").into_node(),
        ]})
        .section("Timeline — 时间线")
        .push(uix::tree! { Container::new().size(INNER_W, 160.0).dir(FlexDirection::Column)
            .pad(uix::base::EdgeInsets::uniform(4.0)) => [
            uix::ui::Timeline::new()
                .add(uix::ui::TimelineItem::new("创建项目").description("2024-01-15"))
                .add(uix::ui::TimelineItem::new("完成设计").description("2024-02-20"))
                .add(uix::ui::TimelineItem::new("部署上线").description("2024-03-10"))
                .into_node(),
        ]})
        .section("Descriptions — 描述列表")
        .push(uix::tree! { Container::new().size(INNER_W, 100.0).dir(FlexDirection::Column)
            .pad(uix::base::EdgeInsets::uniform(4.0)) => [
            uix::ui::Descriptions::new().title("用户信息")
                .add(uix::ui::DescriptionsItem::new("姓名", "张三"))
                .add(uix::ui::DescriptionsItem::new("邮箱", "zhang@ex.com"))
                .add(uix::ui::DescriptionsItem::new("角色", "管理员"))
                .column(3).into_node(),
        ]})
        .section("Tree — 树形控件")
        .push(uix::tree! { Container::new().size(INNER_W, 120.0).dir(FlexDirection::Column)
            .pad(uix::base::EdgeInsets::uniform(4.0)) => [
            uix::ui::Tree::new(vec![
                uix::ui::TreeNode::new("根节点", "1")
                    .add(uix::ui::TreeNode::new("子节点 A", "1-1"))
                    .add(uix::ui::TreeNode::new("子节点 B", "1-2")
                        .add(uix::ui::TreeNode::new("叶子节点", "1-2-1"))),
            ]).into_node(),
        ]})
        .section("Menu — 横向菜单")
        .push(uix::tree! { Container::new().size(INNER_W, 40.0).dir(FlexDirection::Column)
            .pad(uix::base::EdgeInsets::uniform(4.0)) => [
            uix::ui::Menu::new()
                .add_item(uix::ui::MenuItem { key: "home".into(), label: "首页".into(), icon: "".into(), disabled: false })
                .add_item(uix::ui::MenuItem { key: "docs".into(), label: "文档".into(), icon: "".into(), disabled: false })
                .add_item(uix::ui::MenuItem { key: "about".into(), label: "关于".into(), icon: "".into(), disabled: false })
                .mode(MenuMode::Horizontal).active_key("home").into_node(),
        ]})
        .section("AutoComplete — 自动补全")
        .push(uix::tree! { Container::new().size(INNER_W, 40.0).dir(FlexDirection::Column)
            .pad(uix::base::EdgeInsets::uniform(4.0)) => [
            uix::ui::AutoComplete::new().placeholder("输入城市名称...")
                .options(vec!["北京", "上海", "广州", "深圳", "杭州"]).into_node(),
        ]})
        .section("Form + FormItem — 表单")
        .push(uix::tree! { Container::new().size(INNER_W, 130.0).dir(FlexDirection::Column)
            .pad(uix::base::EdgeInsets::uniform(4.0)) => [
            uix::ui::Form::new().gap(4.0).into_node(),
        ]})
        .section("Calendar — 日历")
        .push(uix::tree! { Container::new().size(INNER_W, 240.0).dir(FlexDirection::Column)
            .pad(uix::base::EdgeInsets::uniform(4.0)) => [
            uix::ui::Calendar::new().cell_size(30.0).into_node(),
        ]})
        .section("轮播图 (Carousel)")
        .push(
            Carousel::new()
                .autoplay(3.0)
                .show_dots(true)
                .show_arrows(true),
        )
        .section("分割面板 (Splitter)")
        .push(
            Space::new().size(SpaceSize::Custom(200.0)).height(200.0)
                .child(Splitter::new().panels(3).vertical(false)),
        )
        .section("List — 列表")
        .push(uix::tree! { Container::new().size(INNER_W, 100.0).dir(FlexDirection::Column)
            .pad(uix::base::EdgeInsets::uniform(4.0)) => [
            uix::ui::List::new()
                .header("用户列表")
                .items(vec!["Alice - 设计师", "Bob - 开发者", "Carol - 管理者"])
                .footer("共 3 人")
                .into_node(),
        ]})
        .build()
}
