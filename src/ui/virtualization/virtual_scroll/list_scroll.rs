//! Table、Tree 与 VirtualScroll 共用的等高列表滚动状态。

// 复用父模块的有限值归一化与索引窗口计算规则。
use super::{
    finite_max_scroll_offset, finite_scroll_offset, scroll_offset_after_delta,
    virtual_list_index_range,
};

/// 绘制型大列表与虚拟列表共用的等高滚动状态。
#[derive(Debug, Clone)]
pub struct VirtualListScroll {
    scroll_offset: f32,
    overscan: usize,
    wheel_step: f32,
}

impl Default for VirtualListScroll {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualListScroll {
    /// 创建零偏移、前后预渲染两个项目的滚动状态。
    pub fn new() -> Self {
        Self {
            scroll_offset: 0.0,
            overscan: 2,
            wheel_step: 40.0,
        }
    }

    /// 设置可见区前后额外纳入窗口的项目数量。
    pub fn overscan(mut self, n: usize) -> Self {
        self.overscan = n;
        self
    }

    /// 返回可见区前后额外纳入窗口的项目数量。
    pub fn overscan_count(&self) -> usize {
        self.overscan
    }

    /// 返回归一化后的有限非负滚动偏移。
    pub fn scroll_offset(&self) -> f32 {
        // 对外只暴露有限非负的共享滚动状态。
        finite_scroll_offset(self.scroll_offset)
    }

    /// 恢复滚动偏移，并将非法输入归一为有限非负值。
    pub fn set_scroll_offset(&mut self, offset: f32) {
        // 外部恢复状态必须先满足有限非负坐标不变量。
        self.scroll_offset = finite_scroll_offset(offset);
    }

    /// 返回内容高度超出视口的最大有限滚动偏移。
    pub fn max_scroll_offset(
        &self,
        item_count: usize,
        item_height: f32,
        viewport_height: f32,
    ) -> f32 {
        // 复用安全总高度与视口归一规则，避免乘法溢出。
        finite_max_scroll_offset(item_count, item_height, viewport_height)
    }

    /// 返回当前偏移、视口与预渲染配置对应的半开项目索引范围。
    pub fn scroll_range(
        &self,
        item_count: usize,
        item_height: f32,
        viewport_height: f32,
    ) -> (usize, usize) {
        virtual_list_index_range(
            item_count,
            item_height,
            self.scroll_offset,
            viewport_height,
            self.overscan,
        )
    }

    /// 应用滚动距离并返回边界夹取后实际生效的距离。
    pub fn scroll_by(
        &mut self,
        dy: f32,
        item_count: usize,
        item_height: f32,
        viewport_height: f32,
    ) -> f32 {
        // 内容边界始终保持有限。
        let max = self.max_scroll_offset(item_count, item_height, viewport_height);
        // 非数增量被忽略，无穷增量被夹到对应有限边界。
        let (new, applied) = scroll_offset_after_delta(self.scroll_offset, dy, max);
        // 保存已经归一化的新状态。
        self.scroll_offset = new;
        // 返回真正生效且有限的滚动距离。
        applied
    }

    /// 应用归一化滚轮增量；正值令视口向下移动。
    pub fn scroll_by_wheel(
        &mut self,
        wheel_delta_y: f32,
        item_count: usize,
        item_height: f32,
        viewport_height: f32,
    ) -> f32 {
        self.scroll_by(
            wheel_delta_y * self.wheel_step,
            item_count,
            item_height,
            viewport_height,
        )
    }

    /// 将当前偏移夹取到最新内容与视口形成的有效范围。
    pub fn clamp_to_content(&mut self, item_count: usize, item_height: f32, viewport_height: f32) {
        // 先计算有限内容边界。
        let max = self.max_scroll_offset(item_count, item_height, viewport_height);
        // 恢复状态可能为非有限值，因此不能直接调用 min。
        self.scroll_offset = finite_scroll_offset(self.scroll_offset).min(max);
    }
}
