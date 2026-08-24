use crate::core::Rect;
use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use crate::ui::view::{View, ViewNode};
use std::rc::Rc;
use std::sync::OnceLock;

// 保存由 UIX 声明的 Tree 固有尺寸、搜索栏与行槽位几何。
#[derive(Debug, Clone, Copy, PartialEq)]
struct TreeGeometryVisual {
    default_width: f32,
    default_height: f32,
    row_height: f32,
    search_height: f32,
    slot_width: f32,
    indent_width: f32,
    min_title_width: f32,
}

// 保存由 UIX 声明的搜索输入与行内容视觉。
#[derive(Debug, Clone, Copy, PartialEq)]
struct TreeChromeVisual {
    search_cursor_char_width: f32,
    search_cursor_base_width: f32,
    search_horizontal_padding: f32,
    search_cursor_y: f32,
    search_cursor_width: f32,
    search_cursor_height: f32,
    search_stroke_width: f32,
    search_font_size: f32,
    search_placeholder: &'static str,
    drop_before_ratio: f32,
    drop_after_ratio: f32,
    center_ratio: f32,
    row_focus_inset: f32,
    row_focus_stroke: f32,
    checked_icon: &'static str,
    unchecked_icon: &'static str,
    check_icon_size: f32,
    check_icon_height_ratio: f32,
    expanded_icon: &'static str,
    collapsed_icon: &'static str,
    branch_icon_size: f32,
    branch_icon_height_ratio: f32,
    node_icon_size: f32,
    node_icon_height_ratio: f32,
    title_font_size: f32,
    title_height_ratio: f32,
    focus_inset: f32,
    focus_stroke: f32,
}

// 保存由 UIX 声明的 Tree 主题语义角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TreePaletteVisual {
    primary: ColorValue,
    text: ColorValue,
    text_secondary: ColorValue,
    selected_fill: ColorValue,
    hover_fill: ColorValue,
    pressed_fill: ColorValue,
    search_background: ColorValue,
    search_border: ColorValue,
}

// 全部 Tree 实例共享的完整静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TreeVisual {
    geometry: TreeGeometryVisual,
    chrome: TreeChromeVisual,
    palette: TreePaletteVisual,
}

// 保存 Tree 每帧只解析一次的主题颜色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedTreeVisual {
    pub(crate) primary: Color,
    pub(crate) text: Color,
    pub(crate) text_secondary: Color,
    pub(crate) selected_fill: Color,
    pub(crate) hover_fill: Color,
    pub(crate) pressed_fill: Color,
    pub(crate) search_background: Color,
    pub(crate) search_border: Color,
}

impl TreeVisual {
    pub(crate) fn resolve(self, tokens: &dyn ThemeTokens) -> ResolvedTreeVisual {
        ResolvedTreeVisual {
            primary: self.palette.primary.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            text_secondary: self.palette.text_secondary.resolve(tokens),
            selected_fill: self.palette.selected_fill.resolve(tokens),
            hover_fill: self.palette.hover_fill.resolve(tokens),
            pressed_fill: self.palette.pressed_fill.resolve(tokens),
            search_background: self.palette.search_background.resolve(tokens),
            search_border: self.palette.search_border.resolve(tokens),
        }
    }
}

const fn tree_geometry_visual(
    default_width: f32,
    default_height: f32,
    row_height: f32,
    search_height: f32,
    slot_width: f32,
    indent_width: f32,
    min_title_width: f32,
) -> TreeGeometryVisual {
    TreeGeometryVisual {
        default_width,
        default_height,
        row_height,
        search_height,
        slot_width,
        indent_width,
        min_title_width,
    }
}

// 组合 UIX 声明的搜索栏、图标、焦点和行内容视觉。
#[allow(clippy::too_many_arguments)]
const fn tree_chrome_visual(
    search_cursor_char_width: f32,
    search_cursor_base_width: f32,
    search_horizontal_padding: f32,
    search_cursor_y: f32,
    search_cursor_width: f32,
    search_cursor_height: f32,
    search_stroke_width: f32,
    search_font_size: f32,
    search_placeholder: &'static str,
    drop_before_ratio: f32,
    drop_after_ratio: f32,
    center_ratio: f32,
    row_focus_inset: f32,
    row_focus_stroke: f32,
    checked_icon: &'static str,
    unchecked_icon: &'static str,
    check_icon_size: f32,
    check_icon_height_ratio: f32,
    expanded_icon: &'static str,
    collapsed_icon: &'static str,
    branch_icon_size: f32,
    branch_icon_height_ratio: f32,
    node_icon_size: f32,
    node_icon_height_ratio: f32,
    title_font_size: f32,
    title_height_ratio: f32,
    focus_inset: f32,
    focus_stroke: f32,
) -> TreeChromeVisual {
    TreeChromeVisual {
        search_cursor_char_width,
        search_cursor_base_width,
        search_horizontal_padding,
        search_cursor_y,
        search_cursor_width,
        search_cursor_height,
        search_stroke_width,
        search_font_size,
        search_placeholder,
        drop_before_ratio,
        drop_after_ratio,
        center_ratio,
        row_focus_inset,
        row_focus_stroke,
        checked_icon,
        unchecked_icon,
        check_icon_size,
        check_icon_height_ratio,
        expanded_icon,
        collapsed_icon,
        branch_icon_size,
        branch_icon_height_ratio,
        node_icon_size,
        node_icon_height_ratio,
        title_font_size,
        title_height_ratio,
        focus_inset,
        focus_stroke,
    }
}

// 组合 UIX 声明的主题语义角色。
const fn tree_palette_visual(
    primary: ColorValue,
    text: ColorValue,
    text_secondary: ColorValue,
    selected_fill: ColorValue,
    hover_fill: ColorValue,
    pressed_fill: ColorValue,
    search_background: ColorValue,
    search_border: ColorValue,
) -> TreePaletteVisual {
    TreePaletteVisual {
        primary,
        text,
        text_secondary,
        selected_fill,
        hover_fill,
        pressed_fill,
        search_background,
        search_border,
    }
}

const fn tree_visual(
    geometry: TreeGeometryVisual,
    chrome: TreeChromeVisual,
    palette: TreePaletteVisual,
) -> TreeVisual {
    TreeVisual {
        geometry,
        chrome,
        palette,
    }
}

// 向 UIX 提供受限表达式不能直接写入的静态文案、图标和主题角色。
const fn tree_search_placeholder() -> &'static str {
    "搜索"
}
const fn tree_checked_icon() -> &'static str {
    "check-square"
}
const fn tree_unchecked_icon() -> &'static str {
    "square"
}
const fn tree_expanded_icon() -> &'static str {
    "chevron-down"
}
const fn tree_collapsed_icon() -> &'static str {
    "chevron-right"
}
const fn tree_primary_color() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
const fn tree_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
const fn tree_secondary_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
const fn tree_selected_fill_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}
const fn tree_hover_fill_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}
const fn tree_pressed_fill_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillSecondary)
}
const fn tree_search_background_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}
const fn tree_search_border_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BorderSecondary)
}

// Rust 直接构造时保持既有视觉；正常 View 构建切换到 UIX 静态配置。
pub(crate) static DEFAULT_TREE_VISUAL: TreeVisual = tree_visual(
    tree_geometry_visual(200.0, 300.0, 28.0, 32.0, 20.0, 20.0, 24.0),
    tree_chrome_visual(
        8.0,
        8.0,
        8.0,
        6.0,
        1.0,
        20.0,
        1.0,
        13.0,
        "搜索",
        1.0 / 3.0,
        2.0 / 3.0,
        0.5,
        0.5,
        1.0,
        "check-square",
        "square",
        13.0,
        0.65,
        "chevron-down",
        "chevron-right",
        12.0,
        0.6,
        12.0,
        0.6,
        13.0,
        0.65,
        0.75,
        1.5,
    ),
    tree_palette_visual(
        ColorValue::Palette(PaletteColor::Primary),
        ColorValue::Neutral(NeutralRole::Text),
        ColorValue::Neutral(NeutralRole::TextSecondary),
        ColorValue::Neutral(NeutralRole::FillTertiary),
        ColorValue::Neutral(NeutralRole::FillTertiary),
        ColorValue::Neutral(NeutralRole::FillSecondary),
        ColorValue::Neutral(NeutralRole::BgContainer),
        ColorValue::Neutral(NeutralRole::BorderSecondary),
    ),
);

// 首次 UIX 构建固化声明值，全部 Tree 实例共享一份视觉表。
static UIX_TREE_VISUAL: OnceLock<TreeVisual> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TreePointerAction {
    Check(String),
    Toggle(String),
    Select(String),
}

pub(crate) struct TreeRowGeometry {
    row: Rect,
    check: Option<Rect>,
    toggle: Option<Rect>,
    icon: Option<Rect>,
    title: Rect,
}

/// 树组件使用的递归节点声明。
pub struct TreeNode {
    /// 节点显示的标题。
    pub title: String,
    /// 标识选择、展开和拖拽目标的稳定键。
    pub key: String,
    /// 显示在标题前方的可选 Lucide 图标名称。
    pub icon: String,
    /// 按声明顺序排列的直接子节点。
    pub children: Vec<TreeNode>,
    /// 是否拒绝节点选择、检查和拖拽交互。
    pub disabled: bool,
    /// 是否为节点显示并启用检查框。
    pub checkable: bool,
    /// 节点声明或运行时保留的检查状态。
    pub checked: bool,
    /// 是否允许将节点作为拖拽源。
    pub draggable: bool,
    /// 是否在展开时由应用异步补充子节点。
    pub lazy: bool,
    /// 是否将节点视为不含子节点的叶节点。
    pub is_leaf: bool,
    filter: Option<TreeFilter>,
}

type TreeFilter = Rc<dyn Fn(&TreeNode, &str) -> bool>;

impl Clone for TreeNode {
    fn clone(&self) -> Self {
        Self {
            title: self.title.clone(),
            key: self.key.clone(),
            icon: self.icon.clone(),
            children: self.children.clone(),
            disabled: self.disabled,
            checkable: self.checkable,
            checked: self.checked,
            draggable: self.draggable,
            lazy: self.lazy,
            is_leaf: self.is_leaf,
            filter: self.filter.clone(),
        }
    }
}

impl std::fmt::Debug for TreeNode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TreeNode")
            .field("title", &self.title)
            .field("key", &self.key)
            .field("icon", &self.icon)
            .field("children", &self.children)
            .field("disabled", &self.disabled)
            .field("checkable", &self.checkable)
            .field("checked", &self.checked)
            .field("draggable", &self.draggable)
            .field("lazy", &self.lazy)
            .field("is_leaf", &self.is_leaf)
            .field("has_filter", &self.filter.is_some())
            .finish()
    }
}

impl PartialEq for TreeNode {
    fn eq(&self, other: &Self) -> bool {
        self.title == other.title
            && self.key == other.key
            && self.icon == other.icon
            && self.children == other.children
            && self.disabled == other.disabled
            && self.checkable == other.checkable
            && self.checked == other.checked
            && self.draggable == other.draggable
            && self.lazy == other.lazy
            && self.is_leaf == other.is_leaf
            && self.filter.is_some() == other.filter.is_some()
    }
}

/// 树节点拖拽放置位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropPosition {
    /// 将拖拽节点放在目标节点之前。
    Before,
    /// 将拖拽节点放入目标节点内部成为子节点。
    Inside,
    /// 将拖拽节点放在目标节点之后。
    After,
}

mod methods;
mod widget;
// 集中验证树级文档配置与运行时状态所有权。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/display/tree/tests.rs"]
mod tests;

pub use widget::*;

// 把节点数据、交互状态与 UIX 静态视觉融合为单一根节点。
fn build_tree_view(mut kernel: Tree, declared_visual: TreeVisual) -> ViewNode {
    kernel.visual = UIX_TREE_VISUAL.get_or_init(|| declared_visual);
    ViewNode::leaf(kernel)
}

impl View for Tree {
    fn build(self) -> ViewNode {
        // UIX 拥有公开根与静态视觉；Rust 保留稳定 key、输入、虚拟化和树算法。
        let kernel = self;
        crate::uix!("src/ui/widgets/display/tree/tree.uix")
    }
}

impl TreeNode {
    /// 使用标题与稳定键创建默认叶节点。
    pub fn new(title: &str, key: &str) -> Self {
        Self {
            title: title.to_string(),
            key: key.to_string(),
            icon: String::new(),
            children: Vec::new(),
            disabled: false,
            checkable: false,
            checked: false,
            draggable: false,
            lazy: false,
            is_leaf: true,
            filter: None,
        }
    }

    /// 设置显示在标题前方的 Lucide 图标名称。
    pub fn icon(mut self, i: &str) -> Self {
        self.icon = i.to_string();
        self
    }

    /// 替换全部直接子节点并将当前节点标记为非叶节点。
    pub fn children(mut self, c: Vec<TreeNode>) -> Self {
        self.children = c;
        self.is_leaf = false;
        self
    }

    #[allow(
        clippy::should_implement_trait,
        reason = "add is the established fluent builder API, not arithmetic addition"
    )]
    /// 追加一个直接子节点并将当前节点标记为非叶节点。
    pub fn add(mut self, child: TreeNode) -> Self {
        self.children.push(child);
        self.is_leaf = false;
        self
    }

    /// 设置节点是否拒绝选择、检查和拖拽交互。
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }

    /// 设置是否为节点显示并启用检查框。
    pub fn checkable(mut self, v: bool) -> Self {
        self.checkable = v;
        self
    }

    /// 设置是否允许将节点作为拖拽源。
    pub fn draggable(mut self, v: bool) -> Self {
        self.draggable = v;
        self
    }

    /// 标记为展开时由应用异步补充子节点的节点。
    pub fn lazy(mut self, v: bool) -> Self {
        self.lazy = v;
        self
    }

    /// 设置接收节点与规范化查询文本的自定义筛选谓词。
    pub fn filter<F>(mut self, predicate: F) -> Self
    where
        F: Fn(&Self, &str) -> bool + 'static,
    {
        self.filter = Some(Rc::new(predicate));
        self
    }
}
