//! View 声明样式到具体组件私有配置的适配边界。

// 引入组件公开运行契约。
use crate::ui::widget_runtime::traits::Widget;
// 引入 UI System 拥有的统一样式值。
use crate::ui::theme::style::Style;
// 引入窗口交互区域的专用样式适配入口。
use crate::ui::widgets::window_chrome::WindowInteractionRegion;
// 引入当前已登记消费统一样式的具体组件。
use crate::ui::widgets::{
    Button, Card, Container, Grid, Input, Label, MenuBar, ScrollView, Select, Typography,
};
// VirtualScroll 是布局能力组件而非 widgets 命名空间成员，单独引入。
use crate::ui::virtualization::virtual_scroll::VirtualScroll;
// Modal 只在 feedback capability 启用时进入统一样式桥接。
#[cfg(feature = "feedback")]
use crate::ui::widgets::Modal;
// Tooltip 的触发区显式尺寸同样只在 feedback capability 下桥接。
#[cfg(feature = "feedback")]
use crate::ui::widgets::Tooltip;

// 引入所属适配器类型。
use super::ViewAdapter;

// 集中实现具体组件样式适配，避免主协调文件超过规模上限。
impl ViewAdapter {
    // 把声明样式应用到当前具体组件并保留组件私有语义。
    pub(crate) fn apply_style(
        mut widget: Box<dyn Widget>,
        style: &Style,
        flex_grow_override: Option<f32>,
        flex_shrink_override: Option<f32>,
    ) -> Box<dyn Widget> {
        let style_is_default = style == &Style::default();
        if style_is_default && flex_grow_override.is_none() && flex_shrink_override.is_none() {
            return widget;
        }

        let tid = widget.as_any().type_id();

        if tid == std::any::TypeId::of::<Container>() {
            if let Some(c) = widget.as_any_mut().downcast_mut::<Container>() {
                if !style_is_default {
                    c.style = c.style.clone().apply(style.clone());
                }
                // View DSL 显式 flex 覆盖（含 0.0），Style::apply 无法表达「设为默认值」
                if let Some(g) = flex_grow_override {
                    c.style.flex_grow = g;
                }
                if let Some(s) = flex_shrink_override {
                    c.style.flex_shrink = s;
                }
            }
        } else if tid == std::any::TypeId::of::<Label>() {
            if let Some(l) = widget.as_any_mut().downcast_mut::<Label>() {
                let mut merged = l.style.clone().unwrap_or_default().apply(style.clone());
                if let Some(g) = flex_grow_override {
                    merged.flex_grow = g;
                }
                if let Some(s) = flex_shrink_override {
                    merged.flex_shrink = s;
                }
                // ViewNode width/height → Label 固定尺寸（section 色条等）
                if let Some(w) = style.width {
                    l.fixed_width = Some(w);
                }
                if let Some(h) = style.height {
                    l.fixed_height = Some(h);
                }
                l.style = Some(merged);
            }
        } else if tid == std::any::TypeId::of::<Button>() {
            if let Some(b) = widget.as_any_mut().downcast_mut::<Button>() {
                let mut button_style = style.clone();
                if let Some(g) = flex_grow_override {
                    button_style.flex_grow = g;
                }
                if let Some(s) = flex_shrink_override {
                    button_style.flex_shrink = s;
                }
                b.style = button_style.into();
            }
        } else if tid == std::any::TypeId::of::<Card>() {
            // Card 自己拥有默认尺寸与内容自适应语义，Adapter 只传递显式 View 覆盖。
            if let Some(card) = widget.as_any_mut().downcast_mut::<Card>() {
                // 避免把 Style 默认值误写成 Card 的显式高度。
                card.apply_view_layout_style(style, flex_grow_override);
            }
        } else if tid == std::any::TypeId::of::<ScrollView>() {
            // ScrollView 的 ViewNode 尺寸必须进入组件私有视口配置，不能只停留在声明快照。
            if let Some(scroll_view) = widget.as_any_mut().downcast_mut::<ScrollView>() {
                // 通过窄入口同步尺寸和可表达零值的 Flex 覆盖。
                scroll_view.apply_view_layout_style(
                    // 传递当前声明的统一样式。
                    style,
                    // 传递显式扩张覆盖。
                    flex_grow_override,
                    // 传递显式收缩覆盖。
                    flex_shrink_override,
                );
            }
        } else if tid == std::any::TypeId::of::<VirtualScroll>() {
            // VirtualScroll 与 ScrollView 同语义：尺寸与 Flex 覆盖必须进入组件私有视口配置。
            if let Some(virtual_scroll) = widget.as_any_mut().downcast_mut::<VirtualScroll>() {
                virtual_scroll.apply_view_layout_style(
                    style,
                    flex_grow_override,
                    flex_shrink_override,
                );
            }
        } else if tid == std::any::TypeId::of::<Input>() {
            if let Some(input) = widget.as_any_mut().downcast_mut::<Input>() {
                // 输入组件只接收布局字段；编辑、IME 与视觉状态仍由 Input 私有持有。
                input.apply_view_layout_style(style, flex_grow_override, flex_shrink_override);
            }
        } else if tid == std::any::TypeId::of::<Select>() {
            if let Some(select) = widget.as_any_mut().downcast_mut::<Select>() {
                // 选择器同 Input 一样直接消费叶控件尺寸，避免父容器重新按 hug 测量。
                select.apply_view_layout_style(style, flex_grow_override, flex_shrink_override);
            }
        } else if tid == std::any::TypeId::of::<MenuBar>() {
            if let Some(menu_bar) = widget.as_any_mut().downcast_mut::<MenuBar>() {
                // 菜单栏只消费入口前景/背景覆盖与 Flex 布局字段，弹层视觉保持私有。
                menu_bar.apply_view_layout_style(style);
                if let Some(g) = flex_grow_override {
                    menu_bar.override_flex_grow(g);
                }
                if let Some(s) = flex_shrink_override {
                    menu_bar.override_flex_shrink(s);
                }
            }
        // Typography 只取得自己消费的文本排版字段，不取得 View 生命周期。
        } else if let Some(typography) = widget.as_any_mut().downcast_mut::<Typography>() {
            // 把公开 Style 中排版组件消费的文本字段交给组件。
            typography.apply_view_style(style);
        } else if tid == std::any::TypeId::of::<Grid>() {
            if let Some(g) = widget.as_any_mut().downcast_mut::<Grid>() {
                g.apply_style(style);
            }
        } else if tid
            == std::any::TypeId::of::<crate::ui::widget_runtime::dynamic_label::DynamicLabel>()
        {
            if let Some(dl) = widget
                .as_any_mut()
                .downcast_mut::<crate::ui::widget_runtime::dynamic_label::DynamicLabel>(
            ) {
                dl.set_style(style.clone());
            }
        } else if tid == std::any::TypeId::of::<WindowInteractionRegion>() {
            if let Some(region) = widget
                .as_any_mut()
                .downcast_mut::<WindowInteractionRegion>()
            {
                region.apply_view_style(style, flex_grow_override, flex_shrink_override);
            }
        }

        #[cfg(feature = "feedback")]
        if tid == std::any::TypeId::of::<Modal>() {
            if let Some(modal) = widget.as_any_mut().downcast_mut::<Modal>() {
                // Modal 公共尺寸描述的是对话框而非零尺寸 overlay 占位节点。
                modal.apply_view_layout_style(style);
            }
        }

        // Tooltip 不消费样式会停留主题默认触发区尺寸（80×28），横向行内
        // 超宽命中框会遮挡后继兄弟的命中；显式尺寸必须进入组件测量。
        #[cfg(feature = "feedback")]
        if tid == std::any::TypeId::of::<Tooltip>() {
            if let Some(tooltip) = widget.as_any_mut().downcast_mut::<Tooltip>() {
                tooltip.apply_view_layout_style(style);
            }
        }

        widget
    }
}
