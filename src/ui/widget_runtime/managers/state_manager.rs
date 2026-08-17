use crate::ui::reactive::state::State;
use std::any::Any;
use std::collections::HashMap;

/// Manages component-local state values.
#[derive(Default)]
pub struct StateManager {
    states: HashMap<String, Box<dyn Any + Send + Sync>>,
}

impl StateManager {
    /// 创建不包含任何具名状态槽的管理器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 按键克隆指定类型的状态句柄；键缺失或类型不匹配时返回 `None`。
    pub fn get_state<T: Clone + Send + Sync + 'static>(&self, key: &str) -> Option<State<T>> {
        self.states
            .get(key)
            .and_then(|any| any.downcast_ref::<State<T>>())
            .cloned()
    }

    /// 写入具名状态句柄，并替换同键的旧状态及其类型。
    pub fn set_state<T: Clone + Send + Sync + 'static>(&mut self, key: &str, state: State<T>) {
        self.states.insert(key.to_string(), Box::new(state));
    }

    /// 返回同键同类型的既有状态，否则以该类型默认值创建并替换状态槽。
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

    /// 移除指定键的状态槽；键不存在时不产生变化。
    pub fn remove(&mut self, key: &str) {
        self.states.remove(key);
    }

    /// 移除管理器中的全部状态槽。
    pub fn clear(&mut self) {
        self.states.clear();
    }
}
