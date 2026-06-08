/// Manages keyboard focus for a widget.
#[derive(Default)]
pub struct FocusManager {
    focused: bool,
    focusable: bool,
    tab_index: i32,
}

impl FocusManager {
    pub fn new() -> Self { Self::default() }

    pub fn focus(&mut self) { self.focused = true; }
    pub fn blur(&mut self) { self.focused = false; }

    pub fn is_focused(&self) -> bool { self.focused }
    pub fn is_focusable(&self) -> bool { self.focusable }

    pub fn set_focusable(&mut self, v: bool) { self.focusable = v; }

    pub fn tab_index(&self) -> i32 { self.tab_index }
    pub fn set_tab_index(&mut self, idx: i32) { self.tab_index = idx; }
}
