use crate::ui::{AccessibilityRole, AccessibilitySnapshot, AccessibilityState, AriaAttribute};

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct AccessibilityOverride {
    replacement: Option<AccessibilitySnapshot>,
    role: Option<AccessibilityRole>,
    name: Option<Option<String>>,
    state: Option<AccessibilityState>,
    attributes: Vec<AriaAttribute>,
}

impl AccessibilityOverride {
    pub(crate) fn replace(accessibility: AccessibilitySnapshot) -> Self {
        Self {
            replacement: Some(accessibility),
            ..Self::default()
        }
    }

    pub(crate) fn with_role(mut self, role: AccessibilityRole) -> Self {
        self.role = Some(role);
        self
    }

    pub(crate) fn with_name(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        self.name = Some((!name.is_empty()).then_some(name));
        self
    }

    pub(crate) fn with_state(mut self, state: AccessibilityState) -> Self {
        self.state = Some(state);
        self
    }

    pub(crate) fn with_attribute(mut self, attribute: AriaAttribute) -> Self {
        self.attributes.retain(|item| item.name != attribute.name);
        self.attributes.push(attribute);
        self
    }

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
