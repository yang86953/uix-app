//! Application-level services supplied by component libraries or applications.
use std::{any::TypeId, sync::Arc};
use crate::{core::{Result, WindowId}, draw::FontService, ui::ViewNode};
use super::di::Container;

/// Hooks for an optional application service. The framework owns their lifecycle.
pub trait AppExtension: Send + Sync + 'static {
    fn install(&self, _container: &mut Container) {}
    fn initialize_fonts(&self, _fonts: &mut FontService) -> Result<()> { Ok(()) }
    fn prepare_root(&self, root: ViewNode, _window: WindowId) -> ViewNode { root }
    fn window_closed(&self, _window: WindowId) {}
}

#[derive(Clone, Default)]
pub struct AppExtensions(Vec<(TypeId, Arc<dyn AppExtension>)>);
impl AppExtensions {
    pub fn insert<T: AppExtension>(&mut self, extension: T) {
        let kind = TypeId::of::<T>();
        let value: Arc<dyn AppExtension> = Arc::new(extension);
        if let Some(entry) = self.0.iter_mut().find(|entry| entry.0 == kind) {
            entry.1 = value;
        } else { self.0.push((kind, value)); }
    }
    pub fn initialize_fonts(&self, fonts: &mut FontService) -> Result<()> {
        for (_, extension) in &self.0 { extension.initialize_fonts(fonts)?; }
        Ok(())
    }
    pub(crate) fn prepare_root(&self, mut root: ViewNode, window: WindowId) -> ViewNode {
        for (_, extension) in &self.0 { root = extension.prepare_root(root, window); }
        root
    }
    pub(crate) fn window_closed(&self, window: WindowId) {
        for (_, extension) in &self.0 { extension.window_closed(window); }
    }
}
