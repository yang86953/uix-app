//! `src/ui/widgets/feedback/message/mod.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl Message） ——

impl Message {
    // 测试目标保留消息交互区域观测入口，供反馈组件命中测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn interaction_rects_for_test(
        &self,
        frame: Rect,
    ) -> Vec<(Rect, Option<Rect>, Option<Rect>)> {
        let frame = self.remember_frame(frame);
        self.sync_motion();
        let motion = self.motion.borrow();
        self.message_rects(frame, motion.entries())
            .map(|(rect, entry)| {
                let rect = transitioned_rect(rect, entry.offset(), entry.scale());
                let geometry = self.item_geometry(rect, entry.item().closable);
                (
                    rect,
                    geometry.action,
                    entry.item().closable.then_some(geometry.close),
                )
            })
            .collect()
    }
}
