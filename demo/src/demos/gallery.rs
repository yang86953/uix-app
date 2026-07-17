//! 能力目录页 — 展示框架 widget / 特性覆盖矩阵。

use uix::prelude::*;

use crate::common::page::{page_index_by_label, PageBuilder};
use crate::common::showcase::{backlog_note, info_note};
use crate::demos::context::DemoCtx;

fn coverage_row(
    tk: &DesignTokens,
    active: Option<&State<usize>>,
    name: &str,
    page: &str,
    ok: bool,
) -> ViewNode {
    let mark = if ok { "✓" } else { "—" };
    let line = format!("{mark} {name}  →  {page}");
    let color = if ok {
        tk.color_text
    } else {
        tk.color_text_tertiary
    };

    if ok {
        if let (Some(active_state), Some(target)) = (active, page_index_by_label(page)) {
            let nav = active_state.clone();
            return button(line)
                .on_click(&nav, move |nav| nav.set(target))
                .font_size(12.0)
                .color(color)
                .padding((2.0, 0.0, 2.0, 0.0))
                .width(900.0);
        }
    }

    embed(Label::new(&line).color(color).font_size(12.0))
}

/// 单行覆盖项：(分类, widget/特性, 演示页, 是否已展示)
pub const COVERAGE: &[(&str, &str, &str, bool)] = &[
    // ── 通用 ──
    ("通用", "Button", "通用", true),
    ("通用", "Icon", "通用", true),
    ("通用", "Typography", "通用", true),
    ("通用", "Label", "通用", true),
    ("通用", "Divider", "通用", true),
    ("通用", "Space / SpaceSize", "通用", true),
    ("通用", "FloatButton", "通用", true),
    ("通用", "FloatButtonBackTop", "通用", true),
    // ── 布局 ──
    ("布局", "Container", "布局", true),
    ("布局", "Grid", "布局", true),
    (
        "布局",
        "Layout / Header / Sider / Content / Footer",
        "布局",
        true,
    ),
    ("布局", "Splitter", "布局", true),
    ("布局", "Affix", "布局", true),
    ("布局", "BackTop", "布局", true),
    ("布局", "ScrollView", "布局", true),
    // ── 导航 ──
    ("导航", "Menu / MenuItem / MenuMode", "导航", true),
    ("导航", "Tabs / TabPosition", "导航", true),
    ("导航", "Breadcrumb", "导航", true),
    ("导航", "Pagination", "导航", true),
    ("导航", "Steps / Step / StepStatus", "导航", true),
    ("导航", "Anchor / AnchorItem", "导航", true),
    ("导航", "Dropdown (Overlay)", "导航", true),
    ("导航", "Navigation (builder)", "导航", true),
    ("导航", "NavGroup / NavItem (builder)", "导航", true),
    // ── 输入 ──
    ("输入", "Input", "输入", true),
    ("输入", "InputNumber", "输入", true),
    ("输入", "Select", "输入", true),
    ("输入", "Checkbox / Radio / Switch", "输入", true),
    ("输入", "Slider / Rate", "输入", true),
    ("输入", "Form / FormItem", "输入", true),
    ("输入", "TreeSelect", "输入", true),
    ("输入", "DatePicker / TimePicker", "输入", true),
    ("输入", "ColorPicker", "输入", true),
    ("输入", "Cascader", "输入", true),
    ("输入", "AutoComplete / Mentions", "输入", true),
    ("输入", "Segmented", "输入", true),
    // ── 数据展示 ──
    ("数据", "Card", "数据展示", true),
    ("数据", "List", "数据展示", true),
    ("数据", "Tree / TreeNode", "数据展示", true),
    ("数据", "Carousel", "数据展示", true),
    ("数据", "Collapse / CollapsePanel", "数据展示", true),
    ("数据", "Descriptions", "数据展示", true),
    ("数据", "Avatar / Badge / Tag", "数据展示", true),
    ("数据", "Image / Empty / ResultView", "数据展示", true),
    ("数据", "Skeleton", "数据展示", true),
    ("数据", "Timeline / Calendar", "数据展示", true),
    ("数据", "Table", "数据展示", true),
    ("数据", "SelectableList", "数据展示", true),
    ("数据", "RichText", "数据展示", true),
    // ── 反馈 ──
    ("反馈", "Alert", "反馈", true),
    ("反馈", "Message / Notification", "反馈", true),
    ("反馈", "ProgressBar (线/圆/不确定)", "反馈", true),
    ("反馈", "Spin", "反馈", true),
    ("反馈", "Modal / Drawer (Overlay)", "反馈", true),
    (
        "反馈",
        "Tooltip / Popover / Popconfirm (Overlay)",
        "反馈",
        true,
    ),
    // ── 图表 ──
    ("图表", "BarChart / BarData", "图表", true),
    ("图表", "LineChart / LineData", "图表", true),
    ("图表", "PieChart / PieData", "图表", true),
    // ── 其他 ──
    ("其他", "ScrollView", "其他", true),
    ("其他", "QRCode / Watermark", "其他", true),
    ("其他", "Transfer / Upload", "其他", true),
    ("其他", "ThemeToggle", "其他", true),
    ("其他", "component! 自定义", "其他", true),
    ("其他", "RichText", "数据展示", true),
    // ── App 能力 ──
    ("应用能力", "State / label / on_click", "应用能力", true),
    (
        "应用能力",
        "View DSL / embed / component!",
        "应用能力",
        true,
    ),
    (
        "应用能力",
        "run_after / run_interval (Timer)",
        "应用能力",
        true,
    ),
    (
        "应用能力",
        "Theme light/dark + ThemeToggle",
        "应用能力",
        true,
    ),
    ("应用能力", "follow_system_theme (opt-in)", "应用能力", true),
    ("应用能力", "PicturePolicy (静态 vs 动画)", "应用能力", true),
    ("应用能力", "post_to_ui / on_start 句柄", "应用能力", true),
    ("应用能力", "多窗口 (API 说明)", "覆盖清单", true),
    // ── 框架 Provider 能力 ──
    ("框架能力", "LocaleProvider / Locale", "框架能力", true),
    ("框架能力", "ConfigProvider / ControlSize", "框架能力", true),
    ("框架能力", "ComponentOverrides", "框架能力", true),
    (
        "框架能力",
        "typed component token / render_empty",
        "框架能力",
        true,
    ),
    // ── CLI ──
    (
        "CLI",
        "core / native / draw / ui / app / data",
        "--cli",
        true,
    ),
    // ── 未实现 / 未单独展示 / 待验证（不得混为 Backlog） ──
    ("Backlog", "屏幕阅读器平台桥 (#99)", "—", false),
    ("待真机验证", "macOS AppKit + Metal CpuUpload", "—", false),
    (
        "已实现 / 未单独展示",
        "semantic_handler! fingerprint (#159)",
        "—",
        false,
    ),
    (
        "已实现 / 未单独展示",
        "Computed 独立 StateSlotId",
        "—",
        false,
    ),
    (
        "已实现 / 未单独展示",
        "默认错误 Toast overlay (#89)",
        "—",
        false,
    ),
];

pub fn covered_count() -> (usize, usize) {
    let shown = COVERAGE.iter().filter(|(_, _, _, ok)| *ok).count();
    (shown, COVERAGE.len())
}

pub fn page_gallery(ctx: &DemoCtx<'_>) -> ViewNode {
    let tk = ctx.tk;
    let (shown, total) = covered_count();
    let mut builder = PageBuilder::new(tk).gap();

    builder = builder
        .section("覆盖总览")
        .push(info_note(
            tk,
            &format!(
                "已展示 {shown}/{total} 项；点击 ✓ 行可跳转到对应演示页。CLI 域：`cargo run --bin uix-demo -- --cli`。"
            ),
        ))
        .push(backlog_note(
            tk,
            "本页列出的实现 backlog 仅含屏幕阅读器平台桥（#99）；静态 ARIA、键盘导航与焦点链已落地。",
        ))
        .push(info_note(
            tk,
            "已实现但未单独展示：semantic_handler! fingerprint、Computed StateSlotId、默认错误 Toast overlay；macOS AppKit + Metal CpuUpload 已编码，仍需真机验证。",
        ));

    let active = ctx.active_page;
    let mut last_cat = "";
    for (cat, name, page, ok) in COVERAGE {
        if *cat != last_cat {
            builder = builder.section(&format!("{cat}"));
            last_cat = cat;
        }
        builder = builder.push(coverage_row(tk, active, name, page, *ok));
    }

    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coverage_has_entries() {
        assert!(COVERAGE.len() >= 60);
        let (shown, total) = covered_count();
        assert_eq!(shown + (total - shown), total);
        assert!(
            shown > 50,
            "expected most widgets covered, got {shown}/{total}"
        );
    }

    #[test]
    fn coverage_distinguishes_backlog_from_implemented_and_unverified_items() {
        let backlog: Vec<_> = COVERAGE
            .iter()
            .filter(|(category, _, _, _)| *category == "Backlog")
            .map(|(_, name, _, _)| *name)
            .collect();
        assert_eq!(backlog, vec!["屏幕阅读器平台桥 (#99)"]);

        for implemented in [
            "semantic_handler! fingerprint (#159)",
            "Computed 独立 StateSlotId",
            "默认错误 Toast overlay (#89)",
        ] {
            let category = COVERAGE
                .iter()
                .find(|(_, name, _, _)| *name == implemented)
                .map(|entry| entry.0);
            assert_eq!(category, Some("已实现 / 未单独展示"));
        }

        let macos_category = COVERAGE
            .iter()
            .find(|(_, name, _, _)| *name == "macOS AppKit + Metal CpuUpload")
            .map(|entry| entry.0);
        assert_eq!(macos_category, Some("待真机验证"));
    }
}
