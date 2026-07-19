//! 逐组件视觉验收页：一项库存对应一个隔离场景。

mod display_feedback;
mod general_layout;
mod input;
pub mod manifest;
mod navigation_other;

use uix::prelude::*;

use crate::demos::context::DemoCtx;

pub use manifest::{ComponentVisualCase, COMPONENT_VISUAL_CASES};

const CASE_VIEWPORT_WIDTH: f32 = 620.0;
const CASE_VIEWPORT_HEIGHT: f32 = 390.0;

pub(super) fn qa_target(node: impl IntoWidgetNode) -> ViewNode {
    embed(node).automation_id("component-qa-target")
}

pub(super) fn qa_target_view(node: ViewNode) -> ViewNode {
    node.automation_id("component-qa-target")
}

pub(super) fn qa_variant(label_text: &str, node: ViewNode) -> ViewNode {
    column_fit([
        label(label_text)
            .font_size(11.0)
            .color(ColorValue::Neutral(NeutralRole::TextTertiary)),
        node,
    ])
    .gap(6.0)
}

pub(super) fn qa_row(children: impl IntoViewChildren) -> ViewNode {
    row(children).align(AlignItems::Center).gap(16.0)
}

fn build_case(case: &ComponentVisualCase, tk: &DesignTokens) -> ViewNode {
    general_layout::build(case.id, tk)
        .or_else(|| input::build(case.id, tk))
        .or_else(|| display_feedback::build(case.id, tk))
        .or_else(|| navigation_other::build(case.id, tk))
        .unwrap_or_else(|| {
            embed(Alert::new(format!("组件验收场景缺失：{}", case.name)).type_(StatusLevel::Error))
        })
}

pub fn page_component_qa(ctx: &DemoCtx<'_>) -> ViewNode {
    let case_state = ctx.component_case();
    let total = COMPONENT_VISUAL_CASES.len();
    let index = case_state.get().min(total.saturating_sub(1));
    let case = &COMPONENT_VISUAL_CASES[index];
    let qa_tokens = if ctx.theme_control().is_some_and(|control| control.is_dark()) {
        DesignTokens::antd_dark()
    } else {
        DesignTokens::antd_light()
    };

    let reset_state = case_state.clone();
    let previous_state = case_state.clone();
    let next_state = case_state.clone();
    let controls = qa_row([
        button("首个")
            .on_click(&reset_state, |state| state.set(0))
            .automation_id("component-qa-reset"),
        button("上一个")
            .on_click(&previous_state, move |state| {
                let current = state.get();
                state.set(if current == 0 { total - 1 } else { current - 1 });
            })
            .automation_id("component-qa-previous"),
        button("下一个")
            .primary()
            .on_click(&next_state, move |state| {
                state.set((state.get() + 1) % total)
            })
            .automation_id("component-qa-next"),
    ]);

    let metadata = column_fit([
        qa_row([
            label(format!("{}/{}", index + 1, total))
                .font_size(13.0)
                .color(ColorValue::Palette(PaletteColor::Primary))
                .automation_id("component-qa-position"),
            label(case.category)
                .font_size(12.0)
                .color(ColorValue::Neutral(NeutralRole::TextSecondary)),
            label(case.name)
                .font_size(20.0)
                .color(ColorValue::Neutral(NeutralRole::Text))
                .automation_id("component-qa-current"),
        ]),
        label(case.id)
            .font_size(11.0)
            .color(ColorValue::Neutral(NeutralRole::TextTertiary))
            .automation_id("component-qa-id"),
        label(total.to_string())
            .font_size(1.0)
            .color(ColorValue::Neutral(NeutralRole::TextQuaternary))
            .automation_id("component-qa-total"),
        label(format!(
            "全局：light / dark / desktop / compact；本组件：{}",
            case.states.join(" / ")
        ))
        .font_size(11.0)
        .color(ColorValue::Neutral(NeutralRole::TextSecondary))
        .automation_id("component-qa-states"),
    ])
    .gap(6.0);

    let stage = column_fit([metadata, controls, build_case(case, &qa_tokens)])
        .key(format!("component-qa-case-{}", case.id))
        .gap(16.0)
        .width(CASE_VIEWPORT_WIDTH)
        .height(CASE_VIEWPORT_HEIGHT)
        .padding(EdgeInsets::uniform(18.0))
        .bg(ColorValue::Neutral(NeutralRole::BgContainer))
        .border(1.0, ColorValue::Neutral(NeutralRole::BorderSecondary))
        .radius(qa_tokens.border_radius_lg)
        .automation_id("component-qa-case");

    scroll(
        column_fit([stage])
            .padding((12.0, 8.0, 20.0, 12.0))
            .overflow_content(),
    )
    .both()
    .flex_grow(1.0)
    .build()
}

#[cfg(all(test, feature = "test-harness"))]
mod tests {
    use super::*;
    use regex::Regex;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;
    use uix::core::Rect;
    use uix::ui::test_harness::{ViewAdapter, WidgetCore};

    fn collect_rs_files(path: &Path, output: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = fs::read_dir(path) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_rs_files(&path, output);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                output.push(path);
            }
        }
    }

    #[test]
    fn component_visual_inventory_is_unique_and_covers_public_widgets() {
        let ids: BTreeSet<_> = COMPONENT_VISUAL_CASES.iter().map(|case| case.id).collect();
        let names: BTreeSet<_> = COMPONENT_VISUAL_CASES
            .iter()
            .map(|case| case.name)
            .collect();
        assert_eq!(ids.len(), COMPONENT_VISUAL_CASES.len(), "duplicate case id");
        assert_eq!(
            names.len(),
            COMPONENT_VISUAL_CASES.len(),
            "duplicate case name"
        );
        assert!(COMPONENT_VISUAL_CASES
            .iter()
            .all(|case| !case.states.is_empty()));

        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut files = Vec::new();
        collect_rs_files(&root.join("src/ui/widgets"), &mut files);
        files.push(root.join("src/ui/foundation/focus_trap.rs"));
        files.push(root.join("src/ui/foundation/virtual_scroll.rs"));

        let pattern = match Regex::new(r"(?s)component!\s*\{.*?pub struct\s+([A-Z][A-Za-z0-9_]*)") {
            Ok(pattern) => pattern,
            Err(error) => panic!("component declaration regex: {error}"),
        };
        let mut declared = BTreeSet::new();
        for file in files {
            let source = fs::read_to_string(&file)
                .unwrap_or_else(|error| panic!("read {}: {error}", file.display()));
            for captures in pattern.captures_iter(&source) {
                declared.insert(captures[1].to_string());
            }
        }
        declared.insert("Button".to_string());

        let inventoried: BTreeSet<_> = COMPONENT_VISUAL_CASES
            .iter()
            .filter(|case| case.kind == manifest::CaseKind::Widget)
            .map(|case| case.name.to_string())
            .collect();
        assert_eq!(inventoried, declared);

        let extras: BTreeSet<_> = COMPONENT_VISUAL_CASES
            .iter()
            .filter(|case| case.kind != manifest::CaseKind::Widget)
            .map(|case| case.name)
            .collect();
        assert_eq!(
            extras,
            BTreeSet::from([
                "ConfigProvider",
                "FloatButtonBackTop",
                "LocaleProvider",
                "NavGroup",
                "Navigation",
            ])
        );
    }

    #[test]
    fn every_component_visual_case_has_a_render_builder() {
        let tokens = DesignTokens::antd_light();
        for case in COMPONENT_VISUAL_CASES {
            let rendered = general_layout::build(case.id, &tokens)
                .or_else(|| input::build(case.id, &tokens))
                .or_else(|| display_feedback::build(case.id, &tokens))
                .or_else(|| navigation_other::build(case.id, &tokens));
            assert!(rendered.is_some(), "missing builder for {}", case.name);
        }
    }

    #[test]
    fn every_component_visual_page_case_has_a_non_zero_target() {
        let tokens = DesignTokens::antd_light();
        let ticks = State::new(0u32);
        let active = State::new(crate::common::page::PAGE_COMPONENT_QA);
        let component_case = State::new(0usize);
        for (index, case) in COMPONENT_VISUAL_CASES.iter().enumerate() {
            component_case.set(index);
            let ctx =
                DemoCtx::new(&tokens, &ticks, Some(&active)).with_component_case(&component_case);
            let mut tree = ViewAdapter::build(page_component_qa(&ctx));
            if let Some(root) = tree.root_mut() {
                root.set_frame(Rect::new(0.0, 0.0, 900.0, 640.0));
            }
            tree.layout();
            let target = tree
                .traverse()
                .iter()
                .copied()
                .find(|id| {
                    tree.get(*id)
                        .and_then(|node| node.automation_id())
                        .is_some_and(|automation_id| automation_id == "component-qa-target")
                })
                .unwrap_or_else(|| panic!("{} must expose a QA target", case.name));
            let frame = tree
                .get(target)
                .unwrap_or_else(|| panic!("{} QA target node disappeared", case.name))
                .frame();
            assert!(
                frame.w > 0.0 && frame.h > 0.0,
                "{} target must have a non-zero frame: {frame:?}",
                case.name
            );
        }
    }
}
