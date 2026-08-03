//! Cascader widget - linked multi-level popup selection.

use crate::core::{Point, Rect};
use crate::draw::Color;
use crate::ui::component::paint_context::PaintContext;
use std::collections::HashSet;

const TRIGGER_HEIGHT: f32 = 32.0;
const POPUP_GAP: f32 = 2.0;
const POPUP_COLUMN_MIN_WIDTH: f32 = 200.0;
const POPUP_HEIGHT: f32 = 200.0;
const ITEM_HEIGHT: f32 = 32.0;

#[derive(Debug, Clone, PartialEq)]
pub struct CascaderOption {
    pub label: String,
    pub value: String,
    pub children: Vec<CascaderOption>,
    pub disabled: bool,
}

impl CascaderOption {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            children: vec![],
            disabled: false,
        }
    }

    pub fn children(mut self, children: Vec<CascaderOption>) -> Self {
        self.children = children;
        self
    }

    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CascaderValue {
    pub labels: Vec<String>,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CascaderSearchResult {
    value: CascaderValue,
    disabled: bool,
    loading: bool,
}


mod component;
mod methods;

pub use component::*;


fn collect_search_results(
    options: &[CascaderOption],
    loading_children: &HashSet<String>,
    query: &str,
    path: &mut CascaderValue,
    ancestor_disabled: bool,
    results: &mut Vec<CascaderSearchResult>,
) {
    for option in options {
        path.labels.push(option.label.clone());
        path.values.push(option.value.clone());
        let disabled = ancestor_disabled || option.disabled;
        let loading = loading_children.contains(&option.value);
        if loading || option.children.is_empty() {
            let searchable_path = path.labels.join(" / ").to_lowercase();
            if searchable_path.contains(query) {
                results.push(CascaderSearchResult {
                    value: path.clone(),
                    disabled,
                    loading,
                });
            }
        } else {
            collect_search_results(
                &option.children,
                loading_children,
                query,
                path,
                disabled,
                results,
            );
        }
        path.labels.pop();
        path.values.pop();
    }
}

fn byte_index_for_char(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map(|(index, _)| index)
        .unwrap_or(text.len())
}

fn paint_loading_spinner(ctx: &mut PaintContext, row: Rect, phase: f32, color: Color) {
    let slot = Rect::new(row.x + row.w - 24.0, row.y, 24.0, row.h);
    let radius = 4.5_f32.min(slot.w.min(slot.h) * 0.25);
    if radius > 0.0 {
        ctx.stroke_arc(
            slot.x + slot.w * 0.5,
            slot.y + slot.h * 0.5,
            radius,
            phase,
            phase + std::f32::consts::PI * 1.45,
            color,
            1.6,
        );
    }
}

fn first_enabled_index(options: &[CascaderOption]) -> Option<usize> {
    options.iter().position(|option| !option.disabled)
}

fn next_enabled_index(options: &[CascaderOption], current: usize, forward: bool) -> Option<usize> {
    let len = options.len();
    if len == 0 {
        return None;
    }

    (1..=len)
        .map(|step| {
            if forward {
                (current + step) % len
            } else {
                (current + len - (step % len)) % len
            }
        })
        .find(|index| !options[*index].disabled)
}

fn cascader_column_width(frame: Rect) -> f32 {
    frame.w.max(POPUP_COLUMN_MIN_WIDTH)
}

fn cascader_popup_rect(frame: Rect, level_count: usize) -> Rect {
    Rect::new(
        frame.x,
        frame.y + frame.h + POPUP_GAP,
        cascader_column_width(frame) * level_count.max(1) as f32,
        POPUP_HEIGHT,
    )
}

fn cascader_dirty_rect(frame: Rect, level_count: usize) -> Rect {
    frame.union(&cascader_popup_rect(frame, level_count))
}

fn point_in_half_open_rect(rect: Rect, point: Point) -> bool {
    point.x >= rect.x && point.x < rect.x + rect.w && point.y >= rect.y && point.y < rect.y + rect.h
}

fn fade_color(color: Color, opacity: f32) -> Color {
    let alpha = (color.a as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    color.with_alpha(alpha)
}

