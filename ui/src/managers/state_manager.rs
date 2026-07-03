use crate::state::State;
use std::any::Any;
use std::collections::HashMap;

/// Manages widget-local state values.
#[derive(Default)]
pub struct StateManager {
    states: HashMap<String, Box<dyn Any + Send + Sync>>,
}

impl StateManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_state<T: Clone + Send + Sync + 'static>(&self, key: &str) -> Option<State<T>> {
        self.states
            .get(key)
            .and_then(|any| any.downcast_ref::<State<T>>())
            .cloned()
    }

    pub fn set_state<T: Clone + Send + Sync + 'static>(&mut self, key: &str, state: State<T>) {
        self.states.insert(key.to_string(), Box::new(state));
    }

    pub fn ensure_state<T: Clone + Default + Send + Sync + 'static>(
        &mut self,
        key: &str,
    ) -> State<T> {
        let key_str = key.to_string();
        if let Some(existing) = self.states.get(&key_str) {
            if let Some(state) = existing.downcast_ref::<State<T>>() {
                return state.clone();
            }
        }
        let state = State::<T>::new(T::default());
        self.states.insert(key_str, Box::new(state.clone()));
        state
    }

    pub fn remove(&mut self, key: &str) {
        self.states.remove(key);
    }

    pub fn clear(&mut self) {
        self.states.clear();
    }
}
