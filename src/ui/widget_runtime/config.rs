//! Theme selection and typed style overrides, independent of component policy.
use crate::ui::{Theme, TokenPatch, Widget};
use std::{any::TypeId, collections::HashMap, sync::Arc};
#[derive(Clone, Default, PartialEq)]
pub struct StyleScope {
    pub theme: Option<Theme>,
    pub widget_tokens: WidgetTokenOverrides,
}
/// 按组件类型索引的设计令牌补丁集合。
#[derive(Clone, Default, PartialEq)]
pub struct WidgetTokenOverrides {
    patches: HashMap<TypeId, Arc<TokenPatch>>,
}

impl WidgetTokenOverrides {
    /// 创建不含任何组件类型补丁的集合。
    pub fn new() -> Self {
        Self::default()
    }

    /// 为指定组件类型插入或替换设计令牌补丁。
    pub fn insert<T: Widget>(&mut self, patch: TokenPatch) {
        self.patches.insert(TypeId::of::<T>(), Arc::new(patch));
    }

    /// 消费集合并为指定组件类型插入或替换设计令牌补丁。
    pub fn with<T: Widget>(mut self, patch: TokenPatch) -> Self {
        self.insert::<T>(patch);
        self
    }

    pub fn get(&self, widget: TypeId) -> Option<Arc<TokenPatch>> {
        self.patches.get(&widget).cloned()
    }

    pub fn extend(&mut self, other: Self) {
        self.patches.extend(other.patches);
    }
}
