use crate::graphics::geometry::EdgeInsets;
use crate::graphics::Size;
use crate::graphics::{AlignItems, FlexDirection, JustifyContent};

/// Manages layout constraints and flex properties for a widget.
#[derive(Clone)]
pub struct LayoutManager {
    pub direction: FlexDirection,
    pub justify: JustifyContent,
    pub align: AlignItems,
    pub gap: f32,
    pub padding: EdgeInsets,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub min_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_width: Option<f32>,
    pub max_height: Option<f32>,
}

impl Default for LayoutManager {
    fn default() -> Self {
        Self {
            direction: FlexDirection::Row,
            justify: JustifyContent::Start,
            align: AlignItems::Stretch,
            gap: 0.0,
            padding: EdgeInsets::zero(),
            flex_grow: 0.0,
            flex_shrink: 1.0,
            width: None,
            height: None,
            min_width: None,
            min_height: None,
            max_width: None,
            max_height: None,
        }
    }
}

impl LayoutManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Measure this widget's preferred size given child sizes.
    /// For Row direction: main axis = width (sum of child widths + gaps).
    /// For Column direction: main axis = height (sum of child heights + gaps).
    pub fn measure(&self, child_sizes: &[Size], _container: Size) -> Size {
        let is_row = matches!(self.direction, FlexDirection::Row);

        let mut total_w = self.width.unwrap_or(0.0);
        let mut total_h = self.height.unwrap_or(0.0);

        if !child_sizes.is_empty() {
            let gap_total = self.gap * (child_sizes.len() - 1) as f32;

            if total_w == 0.0 && is_row {
                // Row: sum child widths + gaps + horizontal padding
                total_w = child_sizes.iter().map(|s| s.w).sum::<f32>()
                    + gap_total
                    + self.padding.horizontal();
            }
            if total_h == 0.0 && !is_row {
                // Column: sum child heights + gaps + vertical padding
                total_h = child_sizes.iter().map(|s| s.h).sum::<f32>()
                    + gap_total
                    + self.padding.vertical();
            }

            // Cross-axis: take the maximum child size
            if total_w == 0.0 && !is_row {
                total_w = child_sizes
                    .iter()
                    .map(|s| s.w)
                    .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                    .unwrap_or(0.0)
                    + self.padding.horizontal();
            }
            if total_h == 0.0 && is_row {
                total_h = child_sizes
                    .iter()
                    .map(|s| s.h)
                    .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                    .unwrap_or(0.0)
                    + self.padding.vertical();
            }
        }

        // Clamp to min/max
        if let Some(mw) = self.min_width {
            total_w = total_w.max(mw);
        }
        if let Some(mh) = self.min_height {
            total_h = total_h.max(mh);
        }
        if let Some(mw) = self.max_width {
            total_w = total_w.min(mw);
        }
        if let Some(mh) = self.max_height {
            total_h = total_h.min(mh);
        }

        Size::new(total_w, total_h)
    }
}
