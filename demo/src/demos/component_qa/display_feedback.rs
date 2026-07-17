use uix::prelude::*;

use super::{qa_row, qa_target, qa_target_view, qa_variant};

fn sample_tree() -> Vec<TreeNode> {
    vec![
        TreeNode::new("组件", "components")
            .add(TreeNode::new("输入", "input"))
            .add(TreeNode::new("展示", "display")),
        TreeNode::new("质量", "quality")
            .add(TreeNode::new("视觉", "visual"))
            .add(TreeNode::new("交互", "interaction")),
    ]
}

pub fn build(id: &str, tk: &DesignTokens) -> Option<ViewNode> {
    let view = match id {
        "avatar" => qa_row([
            qa_variant("32 / target", qa_target(Avatar::new("UI").size(32.0))),
            qa_variant(
                "48",
                embed(Avatar::new("贝").size(48.0).bg(tk.color_primary)),
            ),
            qa_variant(
                "64",
                embed(Avatar::new("QA").size(64.0).bg(tk.color_success)),
            ),
        ]),
        "badge" => qa_row([
            qa_variant("Count / target", qa_target(Badge::new().count(8))),
            qa_variant("Overflow", embed(Badge::new().count(120).max(99))),
            qa_variant(
                "Success",
                embed(Badge::new().status(BadgeStatus::Success).text("通过")),
            ),
            qa_variant(
                "Error",
                embed(Badge::new().status(BadgeStatus::Error).text("失败")),
            ),
        ]),
        "calendar" => qa_target(Calendar::new().cell_size(26.0)),
        "card" => qa_row([
            qa_variant(
                "Bordered / target",
                qa_target(
                    Card::new()
                        .title("质量卡片")
                        .bordered(true)
                        .size(210.0, 128.0)
                        .child(Label::new("正文层级与安全边距")),
                ),
            ),
            qa_variant(
                "Elevation",
                embed(
                    Card::new()
                        .title("Elevation 2")
                        .elevation(2)
                        .size(210.0, 128.0)
                        .child(Label::new("阴影与背景对比")),
                ),
            ),
        ]),
        "carousel" => qa_target(Carousel::new().show_dots(true).show_arrows(true)),
        "collapse" => qa_target(Collapse::new().panels(vec![
            CollapsePanel::new("已展开", "展开内容需要保持完整可读。").expanded(),
            CollapsePanel::new("已折叠", "折叠内容"),
            CollapsePanel::new("保持折叠", "折叠内容"),
        ])),
        "descriptions" => qa_target(
            Descriptions::new()
                .title("组件验收信息")
                .column(3)
                .add(DescriptionsItem::new("状态", "执行中"))
                .add(DescriptionsItem::new("平台", "Windows 原生窗口"))
                .add(DescriptionsItem::new("证据", "Light / Dark / Compact"))
                .add(DescriptionsItem::new(
                    "长内容",
                    "Component Visual Quality Gate",
                )),
        ),
        "empty" => qa_row([
            qa_variant("Default / target", qa_target(Empty::new())),
            qa_variant(
                "Custom",
                embed(Empty::new().description("当前筛选条件下没有组件")),
            ),
        ]),
        "image" => qa_row([
            qa_variant(
                "Loaded / target",
                qa_target(
                    Image::new(128.0, 88.0)
                        .src("assets/images/demo.png")
                        .alt("UIX demo"),
                ),
            ),
            qa_variant(
                "Placeholder",
                embed(Image::new(128.0, 88.0).alt("图片占位")),
            ),
        ]),
        "list" => qa_row([
            qa_variant(
                "Data / target",
                qa_target(
                    List::new()
                        .header("质量清单")
                        .items(vec!["视觉基线", "语义断言", "交互回归"])
                        .footer("共 3 项"),
                ),
            ),
            qa_variant("Empty", embed(List::new())),
        ]),
        "result-view" => qa_row([
            qa_variant(
                "Success / target",
                qa_target(ResultView::new(ResultType::Success).title("验收通过")),
            ),
            qa_variant(
                "Warning",
                embed(ResultView::new(ResultType::Warning).title("需要复核")),
            ),
            qa_variant(
                "Error",
                embed(ResultView::new(ResultType::Error).title("存在缺陷")),
            ),
        ]),
        "selectable-list" => qa_target(
            SelectableList::new()
                .items(vec![
                    SelectableItem::new("visual", "视觉质量").icon("eye"),
                    SelectableItem::new("semantic", "语义质量").icon("file-text"),
                    SelectableItem::new("interaction", "交互质量").icon("mouse-pointer"),
                ])
                .active(1)
                .header_button("全选")
                .footer("3 个验收维度"),
        ),
        "skeleton" => qa_row([
            qa_variant(
                "Rect / target",
                qa_target(Skeleton::new().shape(SkeletonShape::Rect).size(240.0, 18.0)),
            ),
            qa_variant(
                "Circle",
                embed(
                    Skeleton::new()
                        .shape(SkeletonShape::Circle)
                        .size(48.0, 48.0),
                ),
            ),
        ]),
        "table" => qa_target(
            Table::new()
                .columns(vec![
                    TableColumn::new("组件", 140.0).sortable(true),
                    TableColumn::new("主题", 120.0),
                    TableColumn::new("状态", 100.0),
                ])
                .rows(vec![
                    vec!["Button".into(), "Light".into(), "通过".into()],
                    vec!["Input".into(), "Dark".into(), "复核".into()],
                    vec!["Table".into(), "Compact".into(), "通过".into()],
                    vec!["Modal".into(), "Light".into(), "通过".into()],
                ])
                .sortable(true)
                .selection(true)
                .bordered(true)
                .size(560.0, 190.0),
        ),
        "tag" => qa_row([
            qa_variant("Default / target", qa_target(Tag::new("Default"))),
            qa_variant(
                "Success",
                embed(Tag::new("Success").color(TagColor::Success)),
            ),
            qa_variant(
                "Warning",
                embed(Tag::new("Warning").color(TagColor::Warning)),
            ),
            qa_variant("Error", embed(Tag::new("Error").color(TagColor::Error))),
            qa_variant("Info", embed(Tag::new("Info").color(TagColor::Info))),
        ]),
        "timeline" => qa_target(
            Timeline::new()
                .add(TimelineItem::new("库存完成").description("88 个组件"))
                .add(TimelineItem::new("视觉执行").description("真实原生窗口"))
                .add(TimelineItem::new("质量门禁").description("逐组件可追溯")),
        ),
        "tree" => qa_target(Tree::new(sample_tree())),
        "rich-text" => qa_target(RichText::new().content(vec![
            RichTextSegment::Text {
                content: "UIX 组件视觉验收 ".to_string(),
                style: RichTextStyle::default(),
            },
            RichTextSegment::Link {
                content: "质量报告".to_string(),
                url: "https://uix.dev/quality".to_string(),
            },
            RichTextSegment::Text {
                content: " — Light / Dark / Compact".to_string(),
                style: RichTextStyle::default(),
            },
        ])),
        "alert" => column_fit([
            qa_target(Alert::new("成功：组件视觉符合基线").type_(StatusLevel::Success)),
            embed(Alert::new("信息：保留可追溯证据").type_(StatusLevel::Info)),
            embed(Alert::new("警告：需要人工复核").type_(StatusLevel::Warning)),
            embed(
                Alert::new("错误：发现阻断缺陷")
                    .type_(StatusLevel::Error)
                    .closable(),
            ),
        ])
        .gap(8.0),
        "drawer" => qa_target(
            Drawer::new("抽屉标题")
                .placement(DrawerPlacement::Right)
                .closable(true),
        ),
        "message" => {
            let message = Message::new().placement(Placement::Top);
            message.success("视觉基线已通过");
            message.info("正在归档证据");
            message.warning("一项需要复核");
            message.error("一项存在缺陷");
            qa_target(message)
        }
        "modal" => qa_target_view(ViewNode::new(
            Modal::new("组件质量确认").closable(true).overlay(true),
            vec![qa_row([button("取消"), button("确认").primary()])],
        )),
        "notification" => {
            let notification = Notification::new().placement(Placement::TopRight);
            notification.success("验收通过", "Button 已符合全部适用状态");
            notification.info("证据归档", "Light / Dark / Compact");
            notification.warning("需要复核", "检查文本截断");
            notification.error("验收失败", "存在视觉阻断问题");
            qa_target(notification)
        }
        "popconfirm" => qa_target(Popconfirm::new().title("确认删除这条视觉基线？")),
        "popover" => qa_target(Popover::new("逐组件证据与状态矩阵").title("质量详情")),
        "progress-bar" => column_fit([
            qa_variant(
                "45% / target",
                qa_target(ProgressBar::new().progress(45.0).size(420.0, 14.0)),
            ),
            qa_variant(
                "Success",
                embed(
                    ProgressBar::new()
                        .progress(100.0)
                        .stroke_color(tk.color_success),
                ),
            ),
            qa_variant("Circle", embed(ProgressBar::new().circle().progress(0.72))),
            qa_variant("Indeterminate", embed(ProgressBar::new().indeterminate())),
        ])
        .gap(12.0),
        "spin" => qa_row([
            qa_variant("Small", qa_target(Spin::new().small())),
            qa_variant("Middle", embed(Spin::new())),
            qa_variant("Large", embed(Spin::new().large())),
        ]),
        "tooltip" => qa_target(Tooltip::new("组件级悬停提示").placement(TooltipPlacement::Top)),
        "focus-trap" => qa_target_view(ViewNode::new(
            FocusTrap::new(),
            vec![qa_row([
                input().placeholder("第一个焦点").build(),
                button("第二个焦点").into(),
                button("第三个焦点").primary().into(),
            ])],
        )),
        _ => return None,
    };
    Some(view)
}
