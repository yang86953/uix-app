//! `src/ui/widgets/navigation/breadcrumb/mod.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl Breadcrumb） ——

impl Breadcrumb {
    // 测试目标保留面包屑可见槽位观测入口，供导航布局测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn visible_slots_for_test(&self) -> Vec<Option<usize>> {
        self.visible_slots()
            .into_iter()
            .map(|slot| match slot {
                BreadcrumbSlot::Item(index) => Some(index),
                BreadcrumbSlot::Overflow => None,
            })
            .collect()
    }

    // 测试目标保留面包屑溢出触发区域观测入口，供导航交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn overflow_trigger_rect_for_test(&self) -> Option<Rect> {
        self.line_layout()
            .into_iter()
            .find_map(|(slot, rect)| (slot == BreadcrumbSlot::Overflow).then_some(rect))
    }

    // 测试目标保留面包屑溢出行观测入口，供导航交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn overflow_rows_for_test(&self) -> Vec<(usize, Rect)> {
        self.overflow_layout()
            .map(|(_, rows)| rows)
            .unwrap_or_default()
    }

    // 测试目标保留面包屑溢出状态观测入口，供导航交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn overflow_state_for_test(&self) -> (bool, Option<usize>) {
        (self.overflow_open, self.overflow_highlighted)
    }

    // 测试目标保留面包屑内容区域观测入口，供导航布局测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn item_content_rects_for_test(
        &self,
        index: usize,
    ) -> Option<(Rect, Option<Rect>, Rect)> {
        let item = self.items.get(index)?;
        let (_, item_rect) = self
            .line_layout()
            .into_iter()
            .find(|(slot, _)| *slot == BreadcrumbSlot::Item(index))?;
        let icon = (!item.icon.is_empty()).then_some(Rect::new(
            item_rect.x,
            item_rect.y,
            self.visual.layout.icon_slot_width,
            item_rect.h,
        ));
        let title_x = item_rect.x
            + if icon.is_some() {
                self.visual.layout.icon_slot_width + self.visual.layout.icon_text_gap
            } else {
                0.0
            };
        let title = Rect::new(
            title_x,
            item_rect.y,
            self.text_width(&item.title, self.visual.layout.title_glyph_width),
            item_rect.h,
        );
        Some((item_rect, icon, title))
    }
}
