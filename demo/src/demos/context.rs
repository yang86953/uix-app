//! Dashboard 共享上下文 — 跨页 State（定时器 tick、动画 time）。
//! 使用 [`使用.md`](../../docs/使用.md) 风格。

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use uix::prelude::*;

#[derive(Clone, Default)]
pub struct ThemeControl {
    handle: Arc<Mutex<Option<AppHandle>>>,
    dark: Arc<AtomicBool>,
}

impl ThemeControl {
    pub fn set_handle(&self, handle: AppHandle) {
        *self.handle.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);
    }

    pub fn handle(&self) -> Option<AppHandle> {
        self.handle
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn is_dark(&self) -> bool {
        self.dark.load(Ordering::Acquire)
    }

    pub fn toggle(&self) {
        let next = !self.is_dark();
        let handle = self
            .handle
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if let Some(handle) = handle {
            let _ = handle.set_theme(if next {
                Theme::antd_dark()
            } else {
                Theme::antd_light()
            });
            self.dark.store(next, Ordering::Release);
        }
    }
}

/// 传入各分类页的共享运行时 State。
pub struct DemoCtx<'a> {
    pub tk: &'a DesignTokens,
    pub timer_ticks: &'a State<u32>,
    pub anim_time: &'a State<f32>,
    pub active_page: Option<&'a State<usize>>,
    home_count: Option<&'a State<i32>>,
    runtime_count: Option<&'a State<i32>>,
    theme_control: Option<&'a ThemeControl>,
}

impl<'a> DemoCtx<'a> {
    pub fn new(
        tk: &'a DesignTokens,
        timer_ticks: &'a State<u32>,
        anim_time: &'a State<f32>,
        active_page: Option<&'a State<usize>>,
    ) -> Self {
        Self {
            tk,
            timer_ticks,
            anim_time,
            active_page,
            home_count: None,
            runtime_count: None,
            theme_control: None,
        }
    }

    pub fn with_counters(
        mut self,
        home_count: &'a State<i32>,
        runtime_count: &'a State<i32>,
    ) -> Self {
        self.home_count = Some(home_count);
        self.runtime_count = Some(runtime_count);
        self
    }

    pub fn with_theme_control(mut self, theme_control: &'a ThemeControl) -> Self {
        self.theme_control = Some(theme_control);
        self
    }

    pub fn home_count(&self) -> State<i32> {
        self.home_count.cloned().unwrap_or_else(|| State::new(0))
    }

    pub fn runtime_count(&self) -> State<i32> {
        self.runtime_count.cloned().unwrap_or_else(|| State::new(0))
    }

    pub fn theme_control(&self) -> Option<&ThemeControl> {
        self.theme_control
    }
}
