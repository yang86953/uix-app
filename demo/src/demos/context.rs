//! Dashboard 共享上下文 — 跨页 State（定时器 tick、动画 time）。

use uix::prelude::*;

/// 传入各分类页的共享运行时 State。
pub struct DemoCtx<'a> {
    pub tk: &'a DesignTokens,
    pub timer_ticks: &'a State<u32>,
    pub anim_time: &'a State<f32>,
    /// 当前页索引；覆盖清单等页用于跳转导航。
    pub active_page: Option<&'a State<usize>>,
    home_count: Option<&'a State<i32>>,
    runtime_count: Option<&'a State<i32>>,
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

    pub fn home_count(&self) -> State<i32> {
        self.home_count.cloned().unwrap_or_else(|| State::new(0))
    }

    pub fn runtime_count(&self) -> State<i32> {
        self.runtime_count.cloned().unwrap_or_else(|| State::new(0))
    }
}
