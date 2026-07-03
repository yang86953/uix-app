//! Fake 键盘 — 可手动设置按键状态，支持 &self 访问。

use crate::api::traits::IKeyboard;
use crate::types::KeyCode;
use std::cell::Cell;
use std::collections::HashSet;

#[derive(Debug)]
pub struct FakeKeyboard {
    /// 当前按下的键
    pub keys_down: HashSet<KeyCode>,
    /// 自上次输入以来的空闲毫秒数
    pub idle_ms: Cell<u32>,
    /// 双击判定时间（毫秒）
    pub double_click_ms: Cell<u32>,
}

impl FakeKeyboard {
    pub fn new() -> Self {
        Self {
            keys_down: HashSet::new(),
            idle_ms: Cell::new(0),
            double_click_ms: Cell::new(500),
        }
    }

    /// 模拟按下按键
    pub fn press(&mut self, key: KeyCode) {
        self.keys_down.insert(key);
    }

    /// 模拟释放按键
    pub fn release(&mut self, key: KeyCode) {
        self.keys_down.remove(&key);
    }

    /// 模拟按下后立即释放（一次点击）
    pub fn tap(&mut self, key: KeyCode) {
        self.keys_down.insert(key);
        self.keys_down.remove(&key);
    }

    /// 清空所有按键状态
    pub fn release_all(&mut self) {
        self.keys_down.clear();
    }
}

impl IKeyboard for FakeKeyboard {
    fn is_down(&self, key: KeyCode) -> bool {
        self.keys_down.contains(&key)
    }

    fn idle_ms(&self) -> u32 {
        self.idle_ms.get()
    }

    fn double_click_ms(&self) -> u32 {
        self.double_click_ms.get()
    }
}
