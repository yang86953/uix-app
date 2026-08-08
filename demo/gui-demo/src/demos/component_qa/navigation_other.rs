use uix::prelude::*;

use super::{qa_row, qa_target, qa_target_view, qa_variant, COMPONENT_VISUAL_CASE_COUNT};

fn menu_item(key: &str, label: &str, icon: &str, disabled: bool) -> MenuItem {
    MenuItem {
        key: key.into(),
        label: label.into(),
        icon: icon.into(),
        children: Vec::new(),
        disabled,
    }
}

pub fn build(id: &str, tk: &DesignTokens) -> Option<ViewNode> {
    let view = match id {
        "anchor" => qa_target(
            Anchor::new(vec![
                AnchorItem::new("组件库存", "#inventory"),
                AnchorItem::new("视觉矩阵", "#visual"),
                AnchorItem::new("回归结论", "#report"),
            ])
            .set_offset_top(8.0),
        ),
        "breadcrumb" => qa_target(
            Breadcrumb::new()
                .item(BreadcrumbItem::new("UIX"))
                .item(BreadcrumbItem::new("组件"))
                .item(BreadcrumbItem::new("视觉质量测试").active()),
        ),
        "dropdown" => qa_target(Dropdown::new("测试操作").items(vec![
            "查看结果",
            "重新执行",
            "标记缺陷",
            "导出报告",
        ])),
        "menu" => column_fit([
            qa_variant(
                "Horizontal / target",
                qa_target(
                    Menu::new()
                        .add_item(menu_item("inventory", "库存", "grid", false))
                        .add_item(menu_item("visual", "视觉", "eye", false))
                        .add_item(menu_item("disabled", "禁用", "lock", true))
                        .mode(MenuMode::Horizontal)
                        .active_key("inventory"),
                ),
            ),
            qa_variant(
                "Vertical",
                embed(
                    Menu::new()
                        .add_item(menu_item("base", "基础", "home", false))
                        .add_item(menu_item("state", "状态", "settings", false))
                        .mode(MenuMode::Vertical)
                        .active_key("base"),
                ),
            ),
        ])
        .gap(14.0),
        "nav-item" => {
            let shared = SharedActive::new(std::cell::Cell::new(0));
            qa_row([
                qa_variant(
                    "Default / target",
                    qa_target(NavItem::new("组件测试", 1, shared.clone()).icon("eye")),
                ),
                qa_variant(
                    "Selected",
                    embed(NavItem::new("已选择", 0, shared.clone()).icon("check")),
                ),
                qa_variant(
                    "Compact",
                    embed(NavItem::new("视觉", 2, shared).icon("grid").compact(true)),
                ),
            ])
        }
        "nav-group" => {
            let items = NavGroup::new()
                .item("组件库存", "grid")
                .item("视觉矩阵", "eye")
                .item("质量报告", "file-text")
                .active_index(1)
                .build();
            column(
                items
                    .into_iter()
                    .enumerate()
                    .map(|(index, item)| {
                        if index == 0 {
                            qa_target(item)
                        } else {
                            embed(item)
                        }
                    })
                    .collect::<Vec<_>>(),
            )
            .gap(4.0)
            .width(210.0)
            .height(150.0)
        }
        "navigation" => {
            let mut navigation = Navigation::new("UIX Quality")
                .item_with_icon("组件库存", "inventory", "grid")
                .item_with_icon("视觉矩阵", "visual", "eye")
                .item_with_icon("质量报告", "report", "file-text")
                .active_index(1)
                .width(220.0)
                .height(190.0)
                .show_version(false)
                .build(tk);
            // 标题和分隔线占用前两个槽位，测试目标选择第一个未选中的 NavItem。
            // 该目标直接覆盖 hover/focus 可见状态，不落到外层布局容器。
            if let Some(target) = navigation.children.get_mut(2) {
                target.automation_id = Some("component-qa-target".into());
            }
            qa_row([
                qa_variant("Default / target", embed(navigation)),
                qa_variant(
                    "Compact",
                    embed(
                        Navigation::new("UIX")
                            .item_with_icon("库存", "inventory", "grid")
                            .item_with_icon("视觉", "visual", "eye")
                            .item_with_icon("报告", "report", "file-text")
                            .active_index(1)
                            .width(44.0)
                            .height(190.0)
                            .show_title(false)
                            .show_version(false)
                            .compact(true)
                            .build(tk),
                    ),
                ),
            ])
        }
        "pagination" => column_fit([
            qa_variant(
                "Middle / target",
                qa_target(Pagination::new(185, 10).current(7).show_total(true)),
            ),
            qa_variant(
                "Compact",
                embed(Pagination::new(48, 10).current(1).item_size(24.0)),
            ),
        ])
        .gap(14.0),
        "steps" => qa_target(
            Steps::new(vec![
                Step::new("库存").status(StepStatus::Finish),
                Step::new("执行")
                    .description("真实窗口")
                    .status(StepStatus::Process),
                Step::new("修复").status(StepStatus::Wait),
                Step::new("报告").status(StepStatus::Error),
            ])
            .current(1),
        ),
        "tabs" => qa_row([
            qa_variant(
                "Top / target",
                qa_target_view(embed(tree! {
                    Tabs::new()
                        .tab("组件", "components")
                        .tab("状态", "states")
                        .tab("结果", "results")
                        .active(1)
                        .position(TabPosition::Top)
                        .size(300.0, 150.0) => [
                            label(format!("{} 个组件", COMPONENT_VISUAL_CASE_COUNT)),
                            label("逐组件适用状态矩阵"),
                            label("Light / Dark / Compact"),
                        ]
                })),
            ),
            qa_variant(
                "Bottom",
                embed(tree! {
                    Tabs::new()
                        .tab("组件", "components")
                        .tab("状态", "states")
                        .tab("结果", "results")
                        .active(0)
                        .position(TabPosition::Bottom)
                        .size(240.0, 150.0) => [
                            label("组件"),
                            label("状态"),
                            label("结果"),
                        ]
                }),
            ),
        ]),
        "chart-placeholder" => qa_target(
            ChartPlaceholder::new()
                .width(560.0)
                .height(190.0)
                .title("组件图表容器")
                .subtitle("空数据状态与响应式尺寸契约")
                .responsive(true),
        ),
        "bar-chart" => qa_target(
            BarChart::new()
                .width(560.0)
                .height(190.0)
                .show_value(true)
                .data(vec![
                    BarData::new("通用", 9.0, tk.color_primary),
                    BarData::new("输入", 19.0, tk.color_success),
                    BarData::new("展示", 18.0, tk.color_warning),
                    BarData::new("反馈", 11.0, tk.color_error),
                    BarData::new("导航", 10.0, tk.color_info),
                ]),
        ),
        "line-chart" => qa_target(
            LineChart::new()
                .width(560.0)
                .height(190.0)
                .line_color(tk.color_primary)
                .show_dots(true)
                .show_grid(true)
                .data(vec![
                    LineData::new("库存", COMPONENT_VISUAL_CASE_COUNT as f32),
                    LineData::new("基础", COMPONENT_VISUAL_CASE_COUNT as f32),
                    LineData::new("暗色", COMPONENT_VISUAL_CASE_COUNT as f32),
                    LineData::new("紧凑", COMPONENT_VISUAL_CASE_COUNT as f32),
                ]),
        ),
        "pie-chart" => qa_row([
            qa_variant(
                "Pie / target",
                qa_target(PieChart::new().size(180.0).data(vec![
                    PieData::new("通过", 74.0, tk.color_success),
                    PieData::new("复核", 10.0, tk.color_warning),
                    PieData::new("失败", 4.0, tk.color_error),
                ])),
            ),
            qa_variant(
                "Donut",
                embed(PieChart::new().size(180.0).donut(0.48).data(vec![
                    PieData::new("Light", 1.0, tk.color_primary),
                    PieData::new("Dark", 1.0, tk.color_success),
                    PieData::new("Compact", 1.0, tk.color_warning),
                ])),
            ),
        ]),
        "qr-code" => qa_row([
            qa_variant(
                "Default / target",
                qa_target(QRCode::new("https://uix.dev/quality").size(128.0)),
            ),
            qa_variant(
                "Dense",
                embed(QRCode::new("UIX-COMPONENT-VISUAL-QUALITY-GATE-2026-07-17").size(128.0)),
            ),
        ]),
        "transfer" => qa_row([
            qa_variant(
                "Data / target",
                qa_target(
                    Transfer::new()
                        .source(vec![
                            TransferItem {
                                key: "button".into(),
                                title: "Button".into(),
                                selected: false,
                            },
                            TransferItem {
                                key: "input".into(),
                                title: "Input".into(),
                                selected: true,
                            },
                            TransferItem {
                                key: "table".into(),
                                title: "Table".into(),
                                selected: false,
                            },
                        ])
                        .target(vec![
                            TransferItem {
                                key: "modal".into(),
                                title: "Modal".into(),
                                selected: false,
                            },
                            TransferItem {
                                key: "tabs".into(),
                                title: "Tabs".into(),
                                selected: false,
                            },
                        ]),
                )
                .width(380.0),
            ),
            qa_variant("Empty", embed(Transfer::new()).width(160.0)),
        ]),
        "upload" => {
            let mut upload = Upload::dragger()
                .accept(".png,.jpg")
                .multiple(true)
                .max_count(4)
                .show_upload_list(true)
                .preview_image(true)
                .manual(true);
            upload.add_file("button-light.png");
            upload.add_file("assets/images/demo.png");
            upload.add_file("broken.png");
            upload.add_file("pending.jpg");
            upload.update_progress(0, 100.0);
            upload.complete_file(0, true);
            upload.update_progress(1, 62.0);
            upload.complete_file(2, false);
            qa_target(upload)
        }
        "watermark" => qa_target_view(
            column([ViewNode::new(
                Watermark::new("UIX QUALITY")
                    // 大字号叠加旋转会稳定进入通用 MSDF lowering，作为真窗视觉矩阵样本。
                    .font_size(64.0)
                    .opacity(0.14)
                    .rotate(-22.0)
                    .gap(120.0, 72.0),
                vec![column_fit([
                    label("组件视觉质量报告").font_size(20.0),
                    label("每个组件都有独立状态矩阵与真实窗口测试。"),
                    label("Light / Dark / Desktop / Compact"),
                ])
                .gap(12.0)
                .padding(EdgeInsets::uniform(24.0))
                .width(560.0)
                .height(170.0)],
            )
            .flex_grow(1.0)])
            .height(170.0),
        ),
        "config-provider" => {
            let mut overrides = ComponentOverrides::default();
            overrides.input.prefix = Some("QA".into());
            overrides.input.suffix = Some("PASS".into());
            qa_target_view(
                ConfigProvider::new()
                    .component_size(ControlSize::Large)
                    .overrides(overrides)
                    .render_empty(|context| {
                        label(format!("{} / 自定义空状态", context.component_name()))
                            .color(ColorValue::Palette(PaletteColor::Primary))
                    })
                    .child(|| {
                        column_fit([
                            qa_row([
                                button("继承 Large").primary().build(),
                                ConfigProvider::new()
                                    .disabled(true)
                                    .child(|| button("嵌套禁用"))
                                    .build(),
                            ]),
                            input().placeholder("构造覆盖：prefix / suffix").build(),
                            List::new().build(),
                        ])
                        .gap(10.0)
                    })
                    .build()
                    .automation_id("component-qa-target"),
            )
        }
        "locale-provider" => qa_target_view(
            column_fit([
                qa_row([
                    qa_variant(
                        "zh-CN",
                        LocaleProvider::zh_cn()
                            .child(|| {
                                column_fit([
                                    label(format!("empty: {}", use_locale().empty_description)),
                                    label(format!("ok: {}", use_locale().ok_text)),
                                ])
                                .gap(6.0)
                            })
                            .build(),
                    ),
                    qa_variant(
                        "en-US",
                        LocaleProvider::en_us()
                            .child(|| {
                                column_fit([
                                    label(format!("empty: {}", use_locale().empty_description)),
                                    label(format!("ok: {}", use_locale().ok_text)),
                                ])
                                .gap(6.0)
                            })
                            .build(),
                    ),
                ]),
                qa_variant(
                    "Locale::default fallback",
                    LocaleProvider::new(Locale::default())
                        .child(|| {
                            label(format!(
                                "fallback empty: {}",
                                use_locale().empty_description
                            ))
                        })
                        .build(),
                ),
            ])
            .gap(14.0)
            .automation_id("component-qa-target"),
        ),
        _ => return None,
    };
    Some(view)
}
