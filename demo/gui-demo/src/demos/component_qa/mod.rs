//! 逐组件视觉测试页：一项清单对应一个隔离场景。

mod display_feedback;
mod general_layout;
mod input;
pub mod manifest;
mod navigation_other;

use uix::prelude::*;

use crate::demos::context::DemoCtx;

pub use manifest::{ComponentVisualCase, COMPONENT_VISUAL_CASES, COMPONENT_VISUAL_CASE_COUNT};

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
            embed(Alert::new(format!("组件测试场景缺失：{}", case.name)).type_(StatusLevel::Error))
        })
}

pub fn page_component_qa(ctx: &DemoCtx<'_>) -> ViewNode {
    let case_state = ctx.component_case();
    let total = COMPONENT_VISUAL_CASE_COUNT;
    let index = case_state.get().min(total.saturating_sub(1));
    let case = COMPONENT_VISUAL_CASES
        .iter()
        .flat_map(|group| group.iter())
        .nth(index)
        .unwrap_or_else(|| panic!("component QA 索引 {index} 必须落在库存内"));
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
