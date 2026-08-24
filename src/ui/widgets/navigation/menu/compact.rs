//! Navigation 折叠侧栏复用 Menu 时需要的紧凑呈现。

// 引入菜单组件及数据项。
use super::{Menu, MenuItem, ResolvedMenuVisual};
// 引入菜单项绘制矩形。
use crate::core::Rect;
// 引入主题解析后的颜色值。
use crate::draw::Color;
// 引入绘制上下文。
use crate::ui::widget_runtime::paint_context::PaintContext;
// 引入响应式状态读取能力。
use crate::ui::State;

// 扩展 Menu 的整栏紧凑呈现，但不改变选择或展开事实所有权。
impl Menu {
    /// 绑定只影响 Navigation 整栏呈现的紧凑状态。
    pub fn compact_when(mut self, collapsed: &State<bool>) -> Self {
        // Menu 借用调用方拥有的折叠状态句柄，不取得其事实所有权。
        self.compact_binding = Some(collapsed.clone());
        // 返回完成组合配置的 Menu。
        self
    }

    /// 返回当前整栏紧凑呈现状态。
    pub(super) fn is_compact(&self) -> bool {
        // 未绑定时保持普通 Menu 的展开呈现。
        self.compact_binding.as_ref().is_some_and(State::get)
    }

    /// 尝试绘制一个紧凑菜单项，并报告是否已处理。
    pub(super) fn paint_compact_item(
        &self,
        ctx: &mut PaintContext,
        item: &MenuItem,
        item_rect: Rect,
        item_color: Color,
        visual: &ResolvedMenuVisual,
    ) -> bool {
        // 普通 Menu 继续使用完整标签绘制路径。
        if !self.is_compact() {
            // 返回未处理以继续主渲染流程。
            return false;
        }
        // 图标存在时在整行居中绘制。
        if !item.icon.is_empty() {
            // 紧凑图标使用完整行作为居中区域。
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                &item.icon,
                item_rect,
                item_color,
                self.visual.typography.compact_icon,
            );
        } else if let Some(display) = item.label.chars().next() {
            // 无图标项以首字符提供可识别的紧凑回退。
            ctx.text_center(
                &display.to_string(),
                item_rect,
                item_color,
                visual.compact_fallback_font_size,
            );
        }
        // 紧凑呈现已经处理该菜单项。
        true
    }
}
