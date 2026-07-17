use uix::prelude::*;

use super::{qa_row, qa_target, qa_target_view, qa_variant};

fn sample_tree() -> Vec<TreeNode> {
    let mut components = TreeNode::new("组件（展开后可滚动）", "components");
    for index in 0..32 {
        components = components.add(TreeNode::new(
            &format!("组件验收项 {index:02}"),
            &format!("component-{index:02}"),
        ));
    }
    vec![
        components,
        TreeNode::new("质量", "quality")
            .add(TreeNode::new("视觉", "visual"))
            .add(TreeNode::new("交互", "interaction")),
    ]
}

fn carousel_slide(text: &str, color: ColorValue) -> ViewNode {
    column([label(text).font_size(22.0).padding_v(70.0)])
        .align(AlignItems::Center)
        .bg(color)
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
        "carousel" => qa_target_view(
            ViewNode::new(
                Carousel::new().show_dots(true).show_arrows(true),
                vec![
                    carousel_slide(
                        "Slide 1 / 默认",
                        ColorValue::Palette(PaletteColor::PrimaryBg),
                    ),
                    carousel_slide(
                        "Slide 2 / 切换",
                        ColorValue::Palette(PaletteColor::SuccessBg),
                    ),
                    carousel_slide(
                        "Slide 3 / 回归",
                        ColorValue::Palette(PaletteColor::WarningBg),
                    ),
                ],
            )
            .width(560.0)
            .height(190.0),
        ),
        "collapse" => qa_row([
            qa_variant(
                "Default / target",
                qa_target(Collapse::new().panels(vec![
                    CollapsePanel::new("已展开", "展开内容需要保持完整可读。").expanded(),
                    CollapsePanel::new("已折叠", "折叠内容"),
                    CollapsePanel::new("保持折叠", "折叠内容"),
                ])),
            ),
            qa_variant(
                "Accordion",
                embed(
                    Collapse::new()
                        .panels(vec![
                            CollapsePanel::new("手风琴 A", "仅一个面板保持展开").expanded(),
                            CollapsePanel::new("手风琴 B", "折叠内容"),
                        ])
                        .accordion(),
                ),
            ),
        ]),
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
                qa_target(
                    ResultView::new(ResultType::Success)
                        .title("验收通过")
                        .extra_text("查看证据"),
                )
                .width(172.0)
                .height(190.0),
            )
            .width(172.0),
            qa_variant(
                "Warning",
                embed(ResultView::new(ResultType::Warning).title("需要复核"))
                    .width(172.0)
                    .height(190.0),
            )
            .width(172.0),
            qa_variant(
                "Error",
                embed(ResultView::new(ResultType::Error).title("存在缺陷"))
                    .width(172.0)
                    .height(190.0),
            )
            .width(172.0),
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
        "table" => qa_row([
            qa_variant(
                "Data / target",
                qa_target(
                    Table::new()
                        .columns(vec![
                            TableColumn::new("组件", 140.0).sortable(true),
                            TableColumn::new("主题", 100.0),
                            TableColumn::new("状态", 90.0),
                        ])
                        .rows(
                            (0..18)
                                .map(|index| {
                                    vec![
                                        format!("Component {index:02}"),
                                        if index % 2 == 0 {
                                            "Light".into()
                                        } else {
                                            "Dark".into()
                                        },
                                        if index % 5 == 0 {
                                            "复核".into()
                                        } else {
                                            "通过".into()
                                        },
                                    ]
                                })
                                .collect(),
                        )
                        .sortable(true)
                        .selection(true)
                        .bordered(true)
                        .size(405.0, 190.0),
                ),
            ),
            qa_variant(
                "Empty",
                embed(
                    Table::new()
                        .columns(vec![TableColumn::new("空表", 115.0)])
                        .rows(Vec::new())
                        .bordered(true)
                        .size(135.0, 190.0),
                ),
            ),
        ]),
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
        "tree" => qa_target_view(
            ViewNode::leaf(Tree::new(sample_tree()))
                .width(560.0)
                .height(190.0),
        ),
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
        "drawer" => qa_target_view(ViewNode::new(
            Drawer::new("抽屉标题")
                .placement(DrawerPlacement::Right)
                .closable(true),
            vec![qa_row([
                button("取消").automation_id("component-qa-drawer-cancel"),
                button("保存")
                    .primary()
                    .automation_id("component-qa-drawer-save"),
            ])],
        )),
        "message" => {
            let message = Message::new().placement(Placement::Top);
            for (type_, content) in [
                (StatusLevel::Success, "视觉基线已通过"),
                (StatusLevel::Info, "正在归档证据"),
                (StatusLevel::Warning, "一项需要复核"),
                (StatusLevel::Error, "一项存在缺陷"),
            ] {
                message.add(MessageItem {
                    type_,
                    content: content.into(),
                    duration_ms: 0,
                    closable: true,
                });
            }
            qa_target_view(column([embed(message)]).width(520.0).height(160.0))
        }
        "modal" => qa_target_view(ViewNode::new(
            Modal::new("组件质量确认").closable(true).overlay(true),
            vec![qa_row([
                button("取消").automation_id("component-qa-modal-cancel"),
                button("确认")
                    .primary()
                    .automation_id("component-qa-modal-confirm"),
            ])],
        )),
        "notification" => {
            let notification = Notification::new().placement(Placement::TopRight);
            for (type_, title, description) in [
                (
                    StatusLevel::Success,
                    "验收通过",
                    "Button 已符合全部适用状态",
                ),
                (StatusLevel::Info, "证据归档", "Light / Dark / Compact"),
                (StatusLevel::Warning, "需要复核", "检查文本截断"),
                (StatusLevel::Error, "验收失败", "存在视觉阻断问题"),
            ] {
                notification.add(NotificationItem {
                    type_,
                    title: title.into(),
                    description: description.into(),
                    duration_ms: 0,
                    closable: true,
                });
            }
            qa_target_view(column([embed(notification)]).width(520.0).height(180.0))
        }
        "popconfirm" => qa_target(Popconfirm::new().title("确认删除这条视觉基线？")),
        "popover" => qa_target(Popover::new("逐组件证据与状态矩阵").title("质量详情")),
        "progress-bar" => column_fit([
            qa_variant(
                "45% / target",
                qa_target(ProgressBar::new().progress(45.0).size(520.0, 14.0)),
            ),
            qa_row([
                qa_variant(
                    "Success",
                    embed(
                        ProgressBar::new()
                            .progress(100.0)
                            .stroke_color(tk.color_success)
                            .size(250.0, 10.0),
                    ),
                ),
                qa_variant(
                    "Error",
                    embed(
                        ProgressBar::new()
                            .progress(64.0)
                            .stroke_color(tk.color_error)
                            .size(250.0, 10.0),
                    ),
                ),
            ]),
            qa_row([
                qa_variant(
                    "Circle",
                    embed(ProgressBar::new().circle().progress(0.72).size(96.0, 96.0)),
                ),
                qa_variant(
                    "Indeterminate",
                    embed(ProgressBar::new().indeterminate().size(380.0, 12.0)),
                ),
            ]),
        ])
        .gap(10.0),
        "spin" => qa_row([
            qa_variant("Small", qa_target(Spin::new().small())),
            qa_variant("Middle", embed(Spin::new())),
            qa_variant("Large", embed(Spin::new().large())),
        ]),
        "tooltip" => column_fit([
            row([
                ViewNode::new(
                    Tooltip::new("Top 提示").placement(TooltipPlacement::Top),
                    vec![button("Top").into()],
                )
                .width(100.0)
                .height(36.0)
                .automation_id("component-qa-target"),
                ViewNode::new(
                    Tooltip::new("Bottom 提示").placement(TooltipPlacement::Bottom),
                    vec![button("Bottom").into()],
                )
                .width(100.0)
                .height(36.0)
                .automation_id("component-qa-tooltip-bottom"),
                ViewNode::new(
                    Tooltip::new("Left 提示").placement(TooltipPlacement::Left),
                    vec![button("Left").into()],
                )
                .width(100.0)
                .height(36.0)
                .automation_id("component-qa-tooltip-left"),
                ViewNode::new(
                    Tooltip::new("Right 提示").placement(TooltipPlacement::Right),
                    vec![button("Right").into()],
                )
                .width(100.0)
                .height(36.0)
                .automation_id("component-qa-tooltip-right"),
            ])
            .align(AlignItems::Center)
            .gap(16.0)
            .height(60.0),
            ViewNode::new(
                Tooltip::new("Focus 提示").trigger(TriggerMode::Focus),
                vec![button("Focus").automation_id("component-qa-tooltip-focus")],
            )
            .width(120.0)
            .height(36.0),
        ])
        .gap(18.0)
        .height(114.0),
        "focus-trap" => qa_target_view(
            column([ViewNode::new(
                FocusTrap::new(),
                vec![qa_row([
                    input()
                        .placeholder("第一个焦点")
                        .automation_id("component-qa-focus-first")
                        .build(),
                    button("第二个焦点").automation_id("component-qa-focus-second"),
                    button("第三个焦点")
                        .primary()
                        .automation_id("component-qa-focus-third"),
                ])],
            )
            .flex_grow(1.0)])
            .height(52.0),
        ),
        _ => return None,
    };
    Some(view)
}
