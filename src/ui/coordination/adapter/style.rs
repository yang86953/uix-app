//! Dispatch authored styles through the component-owned extension port.
use crate::ui::{Widget, Style, StyleDiff};
use super::ViewAdapter;
impl ViewAdapter {
    pub(crate) fn apply_style(
        mut widget: Box<dyn Widget>,
        style: &Style,
        declared: &StyleDiff,
        flex_grow_override: Option<f32>,
        flex_shrink_override: Option<f32>,
    ) -> Box<dyn Widget> {
        if style != &Style::default() || !declared.is_empty()
            || flex_grow_override.is_some() || flex_shrink_override.is_some()
        {
            widget.apply_declaration_style(style, declared, flex_grow_override, flex_shrink_override);
        }
        widget
    }
}
