use super::*;

impl SnapshotSource for Input {
    fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Input {
            placeholder: self.placeholder.clone(),
            input_size: self.input_size,
            disabled: self.disabled,
            prefix: self.prefix.clone(),
            suffix: self.suffix.clone(),
            addon_before: self.addon_before.clone(),
            addon_after: self.addon_after.clone(),
            password: self.password,
            password_visible: self.password_visible,
            clearable: self.clearable,
            search: self.search,
            status: self.status,
            status_message: self.status_message.clone(),
            textarea: self.textarea,
            textarea_rows: self.textarea_rows,
            max_length: self.max_length,
        }
    }
}

/// Compatibility extension for the former `.search(bool)` builder.
pub trait InputSearchExt: Sized {
    fn search(self, enabled: bool) -> Self;
}

impl InputSearchExt for Input {
    fn search(self, enabled: bool) -> Self {
        self.search_enabled(enabled)
    }
}
