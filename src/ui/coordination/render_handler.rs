//! Tree-owned, type-erased component render registrations.
use crate::core::WidgetId;
use crate::ui::view::ViewNode;
use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

pub struct RenderHandlerRegistration {
    kind: TypeId,
    value: Box<dyn Any>,
}
impl RenderHandlerRegistration {
    pub fn new<T: Any>(value: T) -> Self {
        Self {
            kind: TypeId::of::<T>(),
            value: Box::new(value),
        }
    }
}
#[derive(Default)]
pub struct RenderHandlerTable {
    handlers: HashMap<WidgetId, HashMap<TypeId, Box<dyn Any>>>,
}
impl RenderHandlerTable {
    pub(crate) fn replace_widget(
        &mut self,
        owner: WidgetId,
        handlers: Vec<RenderHandlerRegistration>,
    ) {
        if handlers.is_empty() {
            self.handlers.remove(&owner);
            return;
        }
        self.handlers.insert(
            owner,
            handlers
                .into_iter()
                .map(|handler| (handler.kind, handler.value))
                .collect(),
        );
    }
    pub fn get<T: Any>(&self, owner: WidgetId) -> Option<&T> {
        self.handlers
            .get(&owner)?
            .get(&TypeId::of::<T>())?
            .downcast_ref()
    }
    pub fn get_mut<T: Any>(&mut self, owner: WidgetId) -> Option<&mut T> {
        self.handlers
            .get_mut(&owner)?
            .get_mut(&TypeId::of::<T>())?
            .downcast_mut()
    }
    pub(crate) fn clear_widget(&mut self, owner: WidgetId) {
        self.handlers.remove(&owner);
    }
    pub(crate) fn clear(&mut self) {
        self.handlers.clear();
    }
}
