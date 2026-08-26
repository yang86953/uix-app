use crate::ui::{AccessibilityRole, AccessibilitySnapshot, AccessibilityState, AriaAttribute};

/// 无障碍快照覆盖项：以可选字段局部覆盖基准快照，未覆盖部分保持原值。
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct AccessibilityOverride {
    replacement: Option<AccessibilitySnapshot>,
    role: Option<AccessibilityRole>,
    name: Option<Option<String>>,
    state: Option<AccessibilityState>,
    attributes: Vec<AriaAttribute>,
}

impl AccessibilityOverride {
    /// 返回覆盖项最终指定的禁用标志；未覆盖状态时保留基准组件决定权。
    pub(crate) fn disabled_override(&self) -> Option<bool> {
        self.state.as_ref().map(|state| state.disabled).or_else(|| {
            self.replacement
                .as_ref()
                .map(|replacement| replacement.state.disabled)
        })
    }

    /// 整体替换基准无障碍快照（覆盖其余所有字段）。
    pub(crate) fn replace(accessibility: AccessibilitySnapshot) -> Self {
        Self {
            replacement: Some(accessibility),
            ..Self::default()
        }
    }

    /// 覆盖角色（role）。
    pub(crate) fn with_role(mut self, role: AccessibilityRole) -> Self {
        self.role = Some(role);
        self
    }

    /// 覆盖名称（name）；空字符串视为未提供，回退到原名称。
    pub(crate) fn with_name(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        self.name = Some((!name.is_empty()).then_some(name));
        self
    }

    /// 覆盖状态（state）。
    pub(crate) fn with_state(mut self, state: AccessibilityState) -> Self {
        self.state = Some(state);
        self
    }

    /// 追加/替换一个 ARIA 属性（同名属性先移除旧值再写入）。
    pub(crate) fn with_attribute(mut self, attribute: AriaAttribute) -> Self {
        // 同名属性去重：先移除旧条目，避免重复。
        self.attributes.retain(|item| item.name != attribute.name);
        self.attributes.push(attribute);
        self
    }

    /// 将本覆盖项应用到基准快照上，返回合并后的结果。
    pub(crate) fn apply(&self, base: AccessibilitySnapshot) -> AccessibilitySnapshot {
        let mut result = self.replacement.clone().unwrap_or(base);
        if let Some(role) = self.role {
            result.role = role;
        }
        if let Some(name) = &self.name {
            result.name = name.clone();
        }
        if let Some(state) = &self.state {
            result.state = state.clone();
        }
        result.with_attributes(self.attributes.iter().cloned())
    }
}
