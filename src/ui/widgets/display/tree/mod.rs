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

pub struct TreeNode {
    pub title: String,
    pub key: String,
    pub icon: String,
    pub children: Vec<TreeNode>,
    pub disabled: bool,
    pub checkable: bool,
    pub checked: bool,
    pub draggable: bool,
    pub lazy: bool,
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
    Before,
    Inside,
    After,
}

mod component;
mod methods;
// 集中验证树级文档配置与运行时状态所有权。
#[cfg(test)]
mod tests;

pub use component::*;

impl TreeNode {
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

    pub fn icon(mut self, i: &str) -> Self {
        self.icon = i.to_string();
        self
    }

    pub fn children(mut self, c: Vec<TreeNode>) -> Self {
        self.children = c;
        self.is_leaf = false;
        self
    }

    #[allow(
        clippy::should_implement_trait,
        reason = "add is the established fluent builder API, not arithmetic addition"
    )]
    pub fn add(mut self, child: TreeNode) -> Self {
        self.children.push(child);
        self.is_leaf = false;
        self
    }

    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }

    pub fn checkable(mut self, v: bool) -> Self {
        self.checkable = v;
        self
    }

    pub fn draggable(mut self, v: bool) -> Self {
        self.draggable = v;
        self
    }

    /// 标记为展开时由应用异步补充子节点的节点。
    pub fn lazy(mut self, v: bool) -> Self {
        self.lazy = v;
        self
    }

    pub fn filter<F>(mut self, predicate: F) -> Self
    where
        F: Fn(&Self, &str) -> bool + 'static,
    {
        self.filter = Some(Rc::new(predicate));
        self
    }
}
