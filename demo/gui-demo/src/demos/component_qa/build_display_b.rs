//! 展示与反馈类测试场景：descriptions/empty/image/list/result/selectable-list/skeleton/table/tag/timeline/tree/rich-text。
//! 由 display_feedback.rs 以 `#[path]` 引入，build 在父模块按 id 分发。

use uix::prelude::*;

use super::super::{qa_row, qa_target, qa_variant};
// 复用主文件的 frame 辅助与样例数据。
use super::{descriptions_frame, empty_frame, image_frame, inventory_count_label, list_frame, result_frame, rich_text_frame, sample_tree, selectable_list_frame, skeleton_frame, tag_frame, timeline_frame, tree_frame};

// 当前分组不依赖主题令牌，但保留统一构建签名。
pub(super) fn build(id: &str, _tk: &DesignTokens) -> Option<ViewNode> {
    let view = match id {
        "descriptions" => qa_row([
            qa_variant(
                "Bordered / target",
                descriptions_frame(
                    Descriptions::new()
                        .title("组件测试信息")
                        .column(2)
                        .bordered(true)
                        .add(DescriptionsItem::new("状态", "执行中"))
                        .add(DescriptionsItem::new("平台", "Windows 原生窗口"))
                        .add(
                            DescriptionsItem::new(
                                "结果",
                                "Light / Dark / Compact visual quality gate",
                            )
                            .span(2),
                        ),
                    360.0,
                    130.0,
                    "component-qa-target",
                ),
            ),
            qa_variant(
                "220px / responsive",
                descriptions_frame(
                    Descriptions::new()
                        .title("很长的描述列表标题需要省略并保持在窄边界内")
                        .column(3)
                        .bordered(true)
                        .add(DescriptionsItem::new(
                            "负责人和联系方式",
                            "贝露丹迪 · owner@example.com",
                        ))
                        .add(DescriptionsItem::new(
                            "说明",
                            "窄宽度下自动改为单列并换行，不覆盖相邻内容。",
                        )),
                    220.0,
                    180.0,
                    "component-qa-descriptions-constrained",
                ),
            ),
        ]),
        "empty" => qa_row([
            qa_variant(
                "Default / target",
                empty_frame(Empty::new(), 160.0, 100.0, "component-qa-target"),
            ),
            qa_variant(
                "Custom icon",
                empty_frame(
                    Empty::new()
                        .icon("inbox")
                        .description("当前筛选条件下没有组件"),
                    160.0,
                    120.0,
                    "component-qa-empty-icon",
                ),
            ),
            qa_variant(
                "120px / image preset",
                empty_frame(
                    Empty::new()
                        .image("search")
                        .description("没有找到匹配结果，请调整关键词或筛选条件后重试。"),
                    120.0,
                    150.0,
                    "component-qa-empty-constrained",
                ),
            ),
        ]),
        "image" => qa_row([
            qa_variant(
                "Loaded / target",
                image_frame(
                    Image::new(128.0, 88.0)
                        .src("assets/images/demo.png")
                        .alt("UIX demo"),
                    128.0,
                    88.0,
                    "component-qa-target",
                ),
            ),
            qa_variant(
                "Load fallback",
                image_frame(
                    Image::new(128.0, 88.0)
                        .src("assets/images/missing.png")
                        .fallback("图片加载失败，请检查网络后重试"),
                    128.0,
                    88.0,
                    "component-qa-image-fallback",
                ),
            ),
            qa_variant(
                "72×40 / static",
                image_frame(
                    Image::new(128.0, 88.0)
                        .src("assets/images/demo.png")
                        .alt("受限图片")
                        .radius(10.0)
                        .fit(false)
                        .preview(false),
                    72.0,
                    40.0,
                    "component-qa-image-constrained",
                ),
            ),
        ]),
        "image-group" => qa_target(
            ImageGroup::new()
                .images([
                    "assets/images/demo.png",
                    "assets/images/demo.png",
                    "assets/images/demo.png",
                ])
                .start_index(1),
        ),
        "list" => qa_row([
            qa_variant(
                "Data / target",
                list_frame(
                    List::new()
                        .header("质量清单")
                        .items(vec!["视觉基线", "语义断言", "交互回归"])
                        .footer("共 3 项"),
                    260.0,
                    200.0,
                    "component-qa-target",
                ),
            ),
            qa_variant(
                "Empty",
                List::new().build().automation_id("component-qa-list-empty"),
            ),
            qa_variant(
                "150×120 / constrained",
                list_frame(
                    List::new()
                        .header("这是一段很长的质量核验清单标题")
                        .items(vec![
                            "第一项包含很长的中英文 mixed content and identifier",
                            "第二项继续验证窄宽度下不会覆盖相邻内容",
                        ])
                        .footer("长页脚也必须保持在边界内")
                        .load_more("加载更多质量检查项"),
                    150.0,
                    120.0,
                    "component-qa-list-constrained",
                ),
            ),
        ]),
        "result-view" => qa_row([
            qa_variant(
                "Success / target",
                result_frame(
                    ResultView::new(ResultType::Success)
                        .title("测试通过")
                        .subtitle("所有关键检查已通过")
                        .extra_text("查看结果"),
                    172.0,
                    190.0,
                    "component-qa-target",
                ),
            ),
            qa_variant(
                "Warning",
                result_frame(
                    ResultView::new(ResultType::Warning).title("需要复核"),
                    172.0,
                    190.0,
                    "component-qa-result-warning",
                ),
            ),
            qa_variant(
                "Error",
                result_frame(
                    ResultView::new(ResultType::Error).title("存在缺陷"),
                    172.0,
                    190.0,
                    "component-qa-result-error",
                ),
            ),
            qa_variant(
                "140×180 / constrained",
                result_frame(
                    ResultView::new(ResultType::Warning)
                        .title("当前发布仍有需要人工确认的质量风险")
                        .subtitle("请检查兼容性、可访问性与回归测试后再继续。")
                        .extra_text("查看完整质量核验报告"),
                    140.0,
                    180.0,
                    "component-qa-result-constrained",
                ),
            ),
        ]),
        "selectable-list" => qa_row([
            qa_variant(
                "Selected / target",
                selectable_list_frame(
                    SelectableList::new()
                        .items(vec![
                            SelectableItem::new("visual", "视觉质量").icon("eye"),
                            SelectableItem::new("semantic", "语义质量").icon("file-text"),
                            SelectableItem::new("interaction", "交互质量").icon("mouse-pointer"),
                            SelectableItem::new("layout", "布局质量"),
                            SelectableItem::new("theme", "主题质量"),
                            SelectableItem::new("performance", "性能质量"),
                            SelectableItem::new("recovery", "恢复质量"),
                            SelectableItem::new("release", "发布质量"),
                        ])
                        .active(1)
                        .header_button("全选")
                        .footer("3 个测试维度"),
                    220.0,
                    190.0,
                    "component-qa-target",
                ),
            ),
            qa_variant(
                "132×120 / constrained",
                selectable_list_frame(
                    SelectableList::new()
                        .items(vec![
                            SelectableItem::new("visual", "视觉质量与主题一致性测试").icon("eye"),
                            SelectableItem::new("semantic", "Semantic accessibility regression")
                                .icon("file-text"),
                        ])
                        .header_button("选择全部质量检查项")
                        .footer("共 2 个超长测试维度"),
                    132.0,
                    120.0,
                    "component-qa-selectable-constrained",
                ),
            ),
            qa_variant(
                "Long / scroll",
                selectable_list_frame(
                    SelectableList::new()
                        .items(
                            (0..12)
                                .map(|index| {
                                    SelectableItem::new(
                                        format!("check-{index}"),
                                        format!("质量检查项 {index:02}"),
                                    )
                                })
                                .collect(),
                        )
                        .footer("12 个检查项"),
                    180.0,
                    150.0,
                    "component-qa-selectable-scroll",
                ),
            ),
        ]),
        "skeleton" => qa_row([
            qa_variant(
                "Rect / target",
                skeleton_frame(
                    Skeleton::new().shape(SkeletonShape::Rect).size(240.0, 18.0),
                    240.0,
                    18.0,
                    "component-qa-target",
                ),
            ),
            qa_variant(
                "Circle",
                skeleton_frame(
                    Skeleton::new()
                        .shape(SkeletonShape::Circle)
                        .size(48.0, 48.0),
                    48.0,
                    48.0,
                    "component-qa-skeleton-circle",
                ),
            ),
            qa_variant(
                "Text",
                skeleton_frame(
                    Skeleton::new().shape(SkeletonShape::Text).size(200.0, 48.0),
                    200.0,
                    48.0,
                    "component-qa-skeleton-text",
                ),
            ),
            qa_variant(
                "72×12 / constrained",
                skeleton_frame(
                    Skeleton::new().shape(SkeletonShape::Text).size(240.0, 80.0),
                    72.0,
                    12.0,
                    "component-qa-skeleton-constrained",
                ),
            ),
        ]),
        "table" => qa_row([
            qa_variant(
                "Data / target",
                qa_target(
                    Table::new()
                        .columns(vec![
                            TableColumn::new("组件", 140.0)
                                .sortable(true)
                                .resizable(true),
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
                        .empty_text("当前没有符合质量条件的数据")
                        .bordered(true)
                        .size(135.0, 190.0),
                )
                .automation_id("component-qa-table-empty"),
            ),
            qa_variant(
                "150×90 / constrained",
                embed(
                    Table::new()
                        .columns(vec![
                            TableColumn::new("超长可排序组件标题", 74.0).sortable(true),
                            TableColumn::new("Semantic identifier heading", 76.0),
                        ])
                        .rows(vec![
                            vec![
                                "中英文混合单元格 mixed value".into(),
                                "long semantic value".into(),
                            ],
                            vec!["第二个质量检查项".into(), "another long value".into()],
                        ])
                        .sortable(true)
                        .bordered(true)
                        .size(150.0, 90.0),
                )
                .automation_id("component-qa-table-constrained"),
            ),
        ]),
        "tag" => column_fit([
            qa_row([
                qa_variant(
                    "CJK closable / target",
                    row([qa_target(Tag::new("管理员").checkable(true).closable())]),
                ),
                qa_variant(
                    "CJK checked",
                    embed(Tag::new("已选择").default_checked(true))
                        .automation_id("component-qa-tag-checked"),
                ),
                qa_variant(
                    "80×16 / constrained",
                    tag_frame(
                        Tag::new("很长的中英文混合标签 mixed value")
                            .default_checked(true)
                            .closable(),
                        80.0,
                        16.0,
                        "component-qa-tag-constrained",
                    ),
                ),
            ]),
            qa_row([
                qa_variant("Default", embed(Tag::new("默认"))),
                qa_variant("Success", embed(Tag::new("成功").color(TagColor::Success))),
                qa_variant("Warning", embed(Tag::new("警告").color(TagColor::Warning))),
                qa_variant("Error", embed(Tag::new("错误").color(TagColor::Error))),
                qa_variant("Info", embed(Tag::new("信息").color(TagColor::Info))),
                qa_variant(
                    "Light custom",
                    embed(Tag::new("可读").custom_color(Color::white()))
                        .automation_id("component-qa-tag-light-custom"),
                ),
            ]),
            qa_row([qa_variant(
                "Custom 20px",
                embed(Tag::new("重要").font_size(20.0)),
            )]),
        ])
        .gap(14.0),
        "timeline" => column_fit([
            qa_row([
                qa_variant(
                    "220×150 / target",
                    timeline_frame(
                        Timeline::new()
                            .add(
                                TimelineItem::new("库存完成").description(&inventory_count_label()),
                            )
                            .add(
                                TimelineItem::new("视觉执行")
                                    .description("真实原生窗口")
                                    .color(Color::green()),
                            )
                            .add(TimelineItem::new("质量门禁").description("逐组件可追溯")),
                        220.0,
                        150.0,
                        "component-qa-target",
                    ),
                ),
                qa_variant(
                    "132×120 / constrained",
                    timeline_frame(
                        Timeline::new()
                            .add(
                                TimelineItem::new("第一条很长的中英文时间轴标题 mixed value")
                                    .description("第一条很长的说明 description"),
                            )
                            .add(
                                TimelineItem::new("第二条很长的时间轴标题")
                                    .description("第二条很长的说明"),
                            )
                            .add(TimelineItem::new("第三条很长的时间轴标题"))
                            .pending(true),
                        132.0,
                        120.0,
                        "component-qa-timeline-constrained",
                    ),
                ),
            ]),
            qa_row([qa_variant(
                "Reverse + pending",
                timeline_frame(
                    Timeline::new()
                        .add(TimelineItem::new("第一步").description("准备"))
                        .add(TimelineItem::new("第二步").description("执行"))
                        .add(TimelineItem::new("第三步").description("验证"))
                        .pending(true)
                        .reverse(true),
                    220.0,
                    120.0,
                    "component-qa-timeline-reverse-pending",
                ),
            )]),
        ])
        .gap(14.0),
        "tree" => qa_row([
            qa_variant(
                "Scrollable hierarchy",
                tree_frame(
                    Tree::new(sample_tree()),
                    390.0,
                    190.0,
                    "component-qa-target",
                ),
            ),
            qa_variant(
                "Constrained + long title",
                tree_frame(
                    Tree::new(vec![
                        TreeNode::new(
                            "很长的中英文根节点标题 mixed identifier",
                            "constrained-root",
                        )
                        .checkable(true),
                        TreeNode::new("禁用节点", "disabled").disabled(true),
                    ]),
                    132.0,
                    84.0,
                    "component-qa-tree-constrained",
                ),
            ),
        ]),
        "rich-text" => column_fit([
            qa_variant(
                "Interactive link / target",
                rich_text_frame(
                    RichText::new().content(vec![RichTextSegment::Link {
                        content: "质量报告 interactive link target 中英文交互区域".to_string(),
                        url: "https://uix.dev/quality".to_string(),
                    }]),
                    320.0,
                    48.0,
                    "component-qa-target",
                ),
            ),
            qa_variant(
                "Wrapped styles + code",
                rich_text_frame(
                    RichText::new().content(vec![
                        RichTextSegment::Link {
                            content: "质量报告".to_string(),
                            url: "https://uix.dev/quality".to_string(),
                        },
                        RichTextSegment::Text {
                            content: " 展示中英文折行 mixed content 与 ".to_string(),
                            style: RichTextStyle {
                                bold: true,
                                ..RichTextStyle::default()
                            },
                        },
                        RichTextSegment::Link {
                            content: "使用指南".to_string(),
                            url: "https://uix.dev/guide".to_string(),
                        },
                        RichTextSegment::NewLine,
                        RichTextSegment::Code {
                            content: "cargo test --features test-harness".to_string(),
                        },
                    ]),
                    320.0,
                    82.0,
                    "component-qa-rich-text-wrapped",
                ),
            ),
            qa_variant(
                "Constrained clipping",
                rich_text_frame(
                    RichText::new().content(vec![
                        RichTextSegment::Text {
                            content: "极窄富文本 mixed ".to_string(),
                            style: RichTextStyle::default(),
                        },
                        RichTextSegment::Link {
                            content: "可访问链接".to_string(),
                            url: "https://uix.dev/constrained".to_string(),
                        },
                        RichTextSegment::NewLine,
                        RichTextSegment::Code {
                            content: "cargo test --lib".to_string(),
                        },
                    ]),
                    132.0,
                    64.0,
                    "component-qa-rich-text-constrained",
                ),
            ),
        ])
        .gap(14.0),
        _ => return None,
    };
    Some(view)
}
