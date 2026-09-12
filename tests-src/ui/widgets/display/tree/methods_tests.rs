//! `src/ui/widgets/display/tree/methods.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl Tree） ——

impl Tree {
    // 测试目标保留树可见 key 观测入口，供树过滤与展开测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn visible_keys_for_test(&self) -> Vec<String> {
        self.flat.iter().map(|node| node.key.clone()).collect()
    }

    // 测试目标观察 UIX 声明的关键视觉契约，不扩大公开 API。
    #[cfg(test)]
    pub(crate) fn visual_contract_for_test(&self) -> (f32, f32, f32, f32, f32, f32) {
        (
            self.visual.geometry.default_width,
            self.visual.geometry.default_height,
            self.visual.geometry.row_height,
            self.visual.geometry.search_height,
            self.visual.geometry.slot_width,
            self.visual.chrome.title_font_size,
        )
    }

    // 测试目标确认实例共享同一份 UIX 视觉表。
    #[cfg(test)]
    pub(crate) fn shares_visual_with_for_test(&self, other: &Self) -> bool {
        std::ptr::eq(self.visual, other.visual)
    }
}
