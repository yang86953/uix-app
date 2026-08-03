use crate::core::Rect;
use crate::draw::Color;
use crate::ui::reactive::state::State;
use std::collections::HashSet;

mod render;
mod search;

use self::search::VisibleRow;

const DROPDOWN_ROW_HEIGHT: f32 = 28.0;
const MAX_DROPDOWN_VIEWPORT_HEIGHT: f32 = 280.0;

pub(crate) type SelectOptionRenderer = Box<dyn Fn(&str) -> crate::ui::view::ViewNode>;


/// 选项组。
#[derive(Debug, Clone, PartialEq)]
pub struct OptGroup {
    pub label: String,
    pub options: Vec<String>,
}

impl OptGroup {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            options: Vec::new(),
        }
    }

    #[allow(
        clippy::should_implement_trait,
        reason = "add is the established fluent builder API, not arithmetic addition"
    )]
    pub fn add(mut self, opt: &str) -> Self {
        self.options.push(opt.to_string());
        self
    }
}

/// 使用一次性选项集合构造的 `Select` 分组。
///
/// 这是推荐的公开入口；`OptGroup` 保留给已有的逐项 `.add(...)` 写法。
#[derive(Debug, Clone, PartialEq)]
pub struct SelectOptionGroup {
    pub label: String,
    pub options: Vec<String>,
}

impl SelectOptionGroup {
    pub fn new<I, S>(label: impl Into<String>, options: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self {
            label: label.into(),
            options: options
                .into_iter()
                .map(|option| option.as_ref().to_owned())
                .collect(),
        }
    }
}

impl From<SelectOptionGroup> for OptGroup {
    fn from(group: SelectOptionGroup) -> Self {
        Self {
            label: group.label,
            options: group.options,
        }
    }
}

/// 可绑定到 `Select` 的外部值类型。
pub trait SelectValue: Clone + PartialEq + Send + Sync + 'static {
    const MULTIPLE: bool;

    fn selected_indices(&self, options: &[&str]) -> Vec<usize>;
    fn from_selected_indices(options: &[&str], indices: &[usize]) -> Self;
}

impl SelectValue for String {
    const MULTIPLE: bool = false;

    fn selected_indices(&self, options: &[&str]) -> Vec<usize> {
        options
            .iter()
            .position(|option| *option == self)
            .into_iter()
            .collect()
    }

    fn from_selected_indices(options: &[&str], indices: &[usize]) -> Self {
        indices
            .first()
            .and_then(|index| options.get(*index))
            .copied()
            .unwrap_or_default()
            .to_owned()
    }
}

impl SelectValue for HashSet<String> {
    const MULTIPLE: bool = true;

    fn selected_indices(&self, options: &[&str]) -> Vec<usize> {
        options
            .iter()
            .enumerate()
            .filter_map(|(index, option)| self.contains(*option).then_some(index))
            .collect()
    }

    fn from_selected_indices(options: &[&str], indices: &[usize]) -> Self {
        indices
            .iter()
            .filter_map(|index| options.get(*index))
            .map(|option| (*option).to_owned())
            .collect()
    }
}

type ReadSelection = Box<dyn Fn(&[&str]) -> Vec<usize> + Send + Sync>;
type WriteSelection = Box<dyn Fn(&[&str], &[usize]) + Send + Sync>;

pub(crate) struct SelectValueBinding {
    multiple: bool,
    read: ReadSelection,
    write: WriteSelection,
    capture: Box<dyn Fn() + Send + Sync>,
}

impl SelectValueBinding {
    fn new<T: SelectValue>(state: &State<T>) -> Self {
        let read_state = state.clone();
        let write_state = state.clone();
        let capture_state = state.clone();
        Self {
            multiple: T::MULTIPLE,
            read: Box::new(move |options| read_state.get().selected_indices(options)),
            write: Box::new(move |options, indices| {
                let value = T::from_selected_indices(options, indices);
                if write_state.get() != value {
                    write_state.set(value);
                }
            }),
            capture: Box::new(move || {
                let _ = capture_state.get();
            }),
        }
    }
}

mod builder;
mod component;
mod methods;

pub use component::*;


pub struct SelectOptionView {
    select: Select,
    renderer: SelectOptionRenderer,
}

impl crate::ui::view::View for SelectOptionView {
    fn build(self) -> crate::ui::view::ViewNode {
        let mut node = crate::ui::view::ViewNode::leaf(self.select);
        node.render_handlers.push(
            crate::ui::render_handler::RenderHandlerRegistration::SelectOptions(self.renderer),
        );
        node
    }
}

impl From<SelectOptionView> for crate::ui::view::ViewNode {
    fn from(view: SelectOptionView) -> Self {
        crate::ui::view::View::build(view)
    }
}

impl crate::ui::IntoWidgetNode for SelectOptionView {
    fn into_node(self) -> crate::ui::component::widget::WidgetNode {
        crate::ui::IntoWidgetNode::into_node(crate::ui::view::View::build(self))
    }
}

fn select_dirty_rect(frame: Rect, popup: Rect) -> Rect {
    let list = select_popup_rect(frame, popup);
    let expanded = frame.union(&list);
    let expand = 8.0;
    Rect::new(
        expanded.x - expand,
        expanded.y - expand,
        expanded.w + expand * 2.0,
        expanded.h + expand * 2.0,
    )
}

fn select_popup_rect(frame: Rect, popup: Rect) -> Rect {
    Rect::new(frame.x + popup.x, frame.y + popup.y, popup.w, popup.h)
}

fn fade_color(color: Color, opacity: f32) -> Color {
    let alpha = (color.a as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    color.with_alpha(alpha)
}

