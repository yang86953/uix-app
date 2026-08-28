use crate::core::Rect;
use crate::ui::reactive::state::State;
use crate::ui::view::{View, ViewNode};
// 复用浮层共享几何纯函数：归一化、有限收敛与垂直下拉翻转决策。
use crate::ui::widgets::overlay::{
    finite_nonnegative, normalize_rect, resolve_vertical_dropdown_rect, vertical_fallback_surface,
};
use std::collections::HashSet;

mod presentation;
mod render;
mod search;

use self::presentation::*;
use self::search::VisibleRow;

pub(crate) type SelectOptionRenderer = Box<dyn Fn(&str) -> crate::ui::view::ViewNode>;

/// 为选择器提供相互独立的显示文案与稳定值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectOption {
    /// 供界面绘制和搜索使用的显示文案。
    pub label: String,
    /// 供状态绑定和业务逻辑使用的稳定值。
    pub value: String,
}

impl SelectOption {
    /// 创建一项具备独立显示文案与稳定值的选择项。
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        // 同时固化显示文案与稳定值，避免后续借用输入数据。
        Self {
            // 保存面向用户的显示文案。
            label: label.into(),
            // 保存面向状态绑定的稳定值。
            value: value.into(),
        }
    }
}

/// 选项组。
#[derive(Debug, Clone, PartialEq)]
pub struct OptGroup {
    /// 分组标题。
    pub label: String,
    /// 组内显示文案与稳定值相同的选项。
    pub options: Vec<String>,
}

impl OptGroup {
    /// 创建没有选项的分组。
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
    /// 在分组末尾追加一个选项。
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
    /// 分组标题。
    pub label: String,
    /// 组内显示文案与稳定值相同的选项。
    pub options: Vec<String>,
}

impl SelectOptionGroup {
    /// 使用一次性选项集合创建分组。
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
    /// 该值类型是否表示多选集合。
    const MULTIPLE: bool;

    /// 将当前稳定值映射为给定选项集合中的索引。
    fn selected_indices(&self, options: &[&str]) -> Vec<usize>;
    /// 从给定选项集合及已选索引重建外部值。
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

/// 把声明式单选或多选模式约束到对应的状态值类型。
pub trait SelectValueMode<const MULTIPLE: bool>: SelectValue {}

// 单选模式只接受单字符串状态。
impl SelectValueMode<false> for String {}

// 多选模式只接受字符串集合状态。
impl SelectValueMode<true> for HashSet<String> {}

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
mod methods;
mod widget;

pub use widget::*;

/// 为自定义选项 View 附加选择器交互处理器的构建结果。
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
    fn into_node(self) -> crate::ui::widget_runtime::widget::WidgetNode {
        crate::ui::IntoWidgetNode::into_node(crate::ui::view::View::build(self))
    }
}

// UIX 根把同目录声明的完整视觉表注入 Rust 交互内核。
fn build_select_view(mut kernel: Select, visual: &'static SelectVisual) -> ViewNode {
    kernel.visual = visual;
    kernel.control_rect.set(Rect::new(
        0.0,
        0.0,
        visual.layout.natural_min_width,
        visual.layout.control_height(kernel.select_size),
    ));
    kernel.dropdown_rect.set(Rect::zero());
    kernel.surface_rect.set(None);
    ViewNode::leaf(kernel)
}

impl View for Select {
    fn build(self) -> ViewNode {
        build_select_view(self, SELECT_VISUAL_REF)
    }
}

// 计算控件、弹层与受表面裁剪阴影共同占用的脏区。
fn select_dirty_rect(
    // 接收控件的绝对布局矩形。
    frame: Rect,
    // 接收弹层相对控件的最终矩形。
    popup: Rect,
    // 接收当前逻辑表面。
    surface: Rect,
    // 接收 UIX 声明的阴影扩展。
    visual: &SelectVisual,
    // 返回限制在逻辑表面内的脏区。
) -> Rect {
    // 归一化控件矩形以阻止非有限值扩散。
    let frame = normalize_select_rect(frame);
    // 归一化逻辑表面以保证裁剪区有限非负。
    let surface = normalize_select_rect(surface);
    // 将相对弹层转换为绝对窗口坐标。
    let list = select_popup_rect(frame, popup);
    // 合并控件与弹层本体。
    let expanded = frame.union(&list);
    // 为 UIX 声明的下拉阴影预留扩展。
    let expand = visual.chrome.shadow_expand;
    // 扩展阴影后收敛到当前逻辑表面。
    Rect::new(
        // 向左扩展阴影。
        expanded.x - expand,
        // 向上扩展阴影。
        expanded.y - expand,
        // 同时覆盖左右阴影。
        expanded.w + expand * 2.0,
        // 同时覆盖上下阴影。
        expanded.h + expand * 2.0,
    )
    // 裁掉逻辑表面外不可见的重绘范围。
    .intersect(&surface)
    // 控件与表面完全不相交时返回空脏区。
    .unwrap_or_default()
}

// 将相对控件的弹层矩形转换为窗口绝对坐标。
fn select_popup_rect(frame: Rect, popup: Rect) -> Rect {
    // 叠加控件原点并保留受约束尺寸。
    Rect::new(frame.x + popup.x, frame.y + popup.y, popup.w, popup.h)
}

// 使用当前逻辑表面解析选择弹层相对控件的最终矩形。
fn resolve_select_popup_rect(
    // 接收控件的绝对布局矩形。
    frame: Rect,
    // 接收实际绘制的控件高度。
    control_height: f32,
    // 接收当前选项集合决定的自然弹层高度。
    popup_height: f32,
    // 接收当前逻辑表面。
    surface: Rect,
    // 返回相对控件原点的最终弹层矩形。
) -> Rect {
    // 归一化控件矩形。
    let frame = normalize_select_rect(frame);
    // 归一化逻辑表面。
    let surface = normalize_select_rect(surface);
    // 将实际控件高度限制在布局 frame 内。
    let control_height = finite_nonnegative(control_height).min(frame.h);
    // 将自然弹层高度收敛为有限非负值。
    let popup_height = finite_nonnegative(popup_height);
    // 复用共享垂直下拉解析：优先向下、放不下翻向上、两侧不足取大侧并钳制表面。
    // 弹层锚定控件实际底边，宽度直接沿用控件宽度。
    let absolute = resolve_vertical_dropdown_rect(
        frame,
        surface,
        frame.w,
        popup_height,
        frame.y + control_height,
        0.0,
    );
    // 将受约束的绝对矩形转换回相对控件原点。
    Rect::new(
        // 保存横向钳制产生的相对偏移。
        absolute.x - frame.x,
        // 保存上下方向与缩高产生的相对偏移。
        absolute.y - frame.y,
        // 使用受表面限制的宽度。
        absolute.w,
        // 使用受可用空间限制的高度。
        absolute.h,
    )
}

// 构造尚未取得真实窗口表面时的有限回退表面。
fn select_fallback_surface(frame: Rect, popup_height: f32) -> Rect {
    // 归一化自然弹层高度。
    let popup_height = finite_nonnegative(popup_height);
    // 复用共享回退表面：在控件上下各预留一份自然弹层空间（既有两侧份量固定为 2）。
    vertical_fallback_surface(frame, frame.w, popup_height, 0.0, 2.0)
}

// 归一化选择控件、弹层或逻辑表面矩形。
fn normalize_select_rect(rect: Rect) -> Rect {
    // 复用浮层共享归一化：非有限坐标回退原点、尺寸收敛为有限非负。
    normalize_rect(rect)
}
