//! Generic context provider. Libraries own the values and their defaults.
use crate::ui::{View, ViewNode, with_context};
pub struct ContextProvider<T, F> {
    value: T,
    child: F,
}
impl<T> ContextProvider<T, ()> {
    pub fn new(value: T) -> Self {
        Self { value, child: () }
    }
}
impl<T, F> ContextProvider<T, F> {
    pub fn child<G, V>(self, child: G) -> ContextProvider<T, impl FnOnce() -> ViewNode>
    where
        G: FnOnce() -> V,
        V: View,
    {
        ContextProvider {
            value: self.value,
            child: move || child().build(),
        }
    }
}
impl<T, F> View for ContextProvider<T, F>
where
    T: Clone + PartialEq + Send + Sync + 'static,
    F: FnOnce() -> ViewNode + 'static,
{
    fn build(self) -> ViewNode {
        with_context(&self.value, self.child)
    }
}
