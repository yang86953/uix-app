use crate::ui::view::ViewNode;

pub type WindowRootFactory = Box<dyn Fn() -> ViewNode + Send + Sync + 'static>;

/// Runtime configuration for a secondary application window.
pub struct WindowConfig {
    pub title: String,
    pub width: i32,
    pub height: i32,
    pub root: WindowRootFactory,
}

impl WindowConfig {
    pub fn new<F>(title: impl Into<String>, width: i32, height: i32, root: F) -> Self
    where
        F: Fn() -> ViewNode + Send + Sync + 'static,
    {
        Self {
            title: title.into(),
            width,
            height,
            root: Box::new(root),
        }
    }
}
