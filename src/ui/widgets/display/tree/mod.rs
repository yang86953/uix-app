use crate::core::Rect;
use std::rc::Rc;

pub(crate) const TREE_ROW_HEIGHT: f32 = 28.0;
const TREE_SLOT_WIDTH: f32 = 20.0;
const TREE_INDENT_WIDTH: f32 = 20.0;
const TREE_MIN_TITLE_WIDTH: f32 = 24.0;

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

mod component;
mod methods;
// 集中验证树级文档配置与运行时状态所有权。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/display/tree/tests.rs"]
mod tests;

pub use component::*;

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
