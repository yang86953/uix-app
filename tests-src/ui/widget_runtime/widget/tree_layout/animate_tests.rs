//! `src/ui/widget_runtime/widget/tree_layout/animate.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl WidgetTree） ——

impl WidgetTree {
    // 测试目标保留活动过渡 id 观测入口，供动画生命周期测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn active_view_transition_ids(&self) -> Vec<WidgetId> {
        self.traverse()
            .iter()
            .copied()
            .filter(|&id| {
                self.get(id)
                    .is_some_and(BoxedWidget::view_transition_active)
                    && (self.get(id).is_some_and(BoxedWidget::pending_removal)
                        || self.is_effectively_visible(id))
            })
            .collect()
    }

    // 测试目标保留过渡注册观测入口，供动画生命周期测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn view_transition_registrations(&self) -> Vec<(WidgetId, Option<Instant>)> {
        let mut registrations = Vec::new();
        self.extend_view_transition_registrations(&mut registrations);
        registrations
    }

    // 测试目标保留动画节点更新便捷入口，供时间推进测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn update_animation_nodes(
        &mut self,
        ids: &[WidgetId],
        dt: f64,
    ) -> Vec<(WidgetId, bool)> {
        self.update_animation_nodes_at(ids, Instant::now(), dt)
    }
}
