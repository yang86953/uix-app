//! `src/ui/widgets/feedback/notification/layout.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl Notification） ——

impl Notification {
    #[cfg(test)]
    pub(crate) fn interaction_rects_for_test(
        &self,
        frame: Rect,
    ) -> Vec<(Rect, Option<Rect>, Option<Rect>)> {
        let frame = self.remember_frame(frame);
        self.sync_motion();
        let motion = self.motion.borrow();
        self.notification_rects(frame, motion.entries())
            .map(|(rect, entry)| {
                let rect = transitioned_rect(rect, entry.offset(), entry.scale());
                let geometry = self.item_geometry(rect, entry.item());
                (
                    rect,
                    geometry.action,
                    entry.item().closable.then_some(geometry.close),
                )
            })
            .collect()
    }
}
