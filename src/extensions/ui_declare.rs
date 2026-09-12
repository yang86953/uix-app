//! Library-owned immutable declaration payload crossing the worker boundary.
use std::{any::Any, fmt::Debug};

trait Payload: Debug + Send + Sync {
    fn cloned(&self) -> Box<dyn Payload>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn equals(&self, other: &dyn Payload) -> bool;
}
impl<T: Any + Clone + Debug + PartialEq + Send + Sync> Payload for T {
    fn cloned(&self) -> Box<dyn Payload> { Box::new(self.clone()) }
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
    fn equals(&self, other: &dyn Payload) -> bool { other.as_any().downcast_ref::<T>().is_some_and(|other| self == other) }
}

/// A validated declaration. Only the owning library interprets its contents.
#[derive(Debug)]
pub struct UiNode(Box<dyn Payload>);
impl Clone for UiNode { fn clone(&self) -> Self { Self(self.0.cloned()) } }
impl PartialEq for UiNode { fn eq(&self, other: &Self) -> bool { self.0.equals(other.0.as_ref()) } }
impl UiNode {
    pub fn new<T: Any + Clone + Debug + PartialEq + Send + Sync>(value: T) -> Self { Self(Box::new(value)) }
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> { self.0.as_any().downcast_ref() }
    pub fn downcast_mut<T: Any>(&mut self) -> Option<&mut T> { self.0.as_any_mut().downcast_mut() }
}
