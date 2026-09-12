//! 声明式 View 布局样式到 ScrollView 私有配置的适配。

// 引入 UI System 统一拥有的声明样式值。
use crate::ui::theme::style::Style;

// 引入父模块定义的滚动组件。
use super::ScrollView;

// 为 ScrollView 提供 Adapter 可调用的窄布局样式入口。
impl ScrollView {
    // 判断下一份受控声明是否改变了当前 live 滚动位置。
    pub(crate) fn controlled_offset_changed(&self, next: &Self) -> bool {
        // 只有下一声明绑定外部 State 时才把坐标视为受控运行态。
        next.scroll_binding.is_some()
            // 任一轴变化都必须进入统一 Paint 或 Composite 失效入口。
            && (self.scroll_x() != next.scroll_x || self.scroll_y() != next.scroll_y)
    }

    // 将下一次声明组件同步到 live 实例，并保留受控滚动的局部合成差量。
    pub(crate) fn sync_from(&mut self, next: Self) {
        // 保存受控同步前的水平偏移，供统一滚动合成契约计算真实位移。
        let old_scroll_x = self.scroll_x();
        // 保存受控同步前的垂直偏移，避免声明式状态更新绕过滚动搬移。
        let old_scroll_y = self.scroll_y();
        // 只把带绑定的下一声明解释为受控偏移更新。
        let controlled_offset = next
            // 借用下一声明的状态绑定。
            .scroll_binding
            // 无绑定时保留 live 组件自己的运行态偏移。
            .as_ref()
            // 冻结下一声明读取到的两个轴偏移。
            .map(|_| (next.scroll_x, next.scroll_y));
        // 替换状态绑定所有权。
        self.scroll_binding = next.scroll_binding;
        // 同步声明滚动方向。
        self.direction = next.direction;
        // 同步声明固定宽度。
        self.fixed_width = next.fixed_width;
        // 同步声明固定高度。
        self.fixed_height = next.fixed_height;
        // 同步父级 Flex 扩张系数。
        self.flex_grow_val = next.flex_grow_val;
        // 同步父级 Flex 收缩系数。
        self.flex_shrink_val = next.flex_shrink_val;
        // 同步纵向滚动条可见配置。
        self.scrollbar_v.show = next.scrollbar_v.show;
        // 同步横向滚动条可见配置。
        self.scrollbar_h.show = next.scrollbar_h.show;
        // 保留 UIX 注入的唯一视口与滚动条视觉引用。
        self.visual = next.visual;
        // 只有受控声明才覆盖 live 偏移。
        if let Some((scroll_x, scroll_y)) = controlled_offset {
            // 将声明水平偏移限制到当前内容范围。
            self.scroll_x = self.clamp_bound_axis(scroll_x, true);
            // 将声明纵向偏移限制到当前内容范围。
            self.scroll_y = self.clamp_bound_axis(scroll_y, false);
            // 受控 State 更新与输入事件共享同一增量记录，由 WidgetTree 决定局部搬移或安全重绘。
            self.push_scroll_delta(
                self.scroll_x() - old_scroll_x,
                self.scroll_y() - old_scroll_y,
            );
        }
    }

    // 应用 ViewNode 显式尺寸与 Flex 覆盖，不接收绘制或平台语义。
    pub(crate) fn apply_view_layout_style(
        // 借用当前运行态组件。
        &mut self,
        // 借用声明节点提供的统一样式。
        style: &Style,
        // 接收可表达显式零值的扩张系数覆盖。
        flex_grow_override: Option<f32>,
        // 接收可表达显式零值的收缩系数覆盖。
        flex_shrink_override: Option<f32>,
    ) {
        // 只在声明显式宽度时覆盖 builder 自有配置。
        if let Some(width) = style.width {
            // 保存与其它组件一致的声明宽度，由共享布局边界统一规范化。
            self.fixed_width = Some(width);
        }
        // 只在声明显式高度时覆盖 builder 自有配置。
        if let Some(height) = style.height {
            // 保存与其它组件一致的声明高度，由共享布局边界统一规范化。
            self.fixed_height = Some(height);
        }
        // ViewNode 覆盖优先于 ScrollBuilder 默认扩张值。
        if let Some(flex_grow) = flex_grow_override {
            // 保留显式零值，使固定视口不会吞掉父级剩余空间。
            self.flex_grow_val = flex_grow;
        }
        // ViewNode 覆盖优先于 ScrollBuilder 默认收缩值。
        if let Some(flex_shrink) = flex_shrink_override {
            // 保存声明式收缩系数供父级统一 Flex 求解。
            self.flex_shrink_val = flex_shrink;
        }
    }
}
