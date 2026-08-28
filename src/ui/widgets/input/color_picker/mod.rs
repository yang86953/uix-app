//! ColorPicker widget — 颜色选择器，Ant Design 风格。
//!
//! 预设色板选择，点击触发弹出面板。

use crate::core::{Constraints, Rect, Size};
use crate::draw::{Color, Radius};
use crate::platform::windowing::ControlSize;
use crate::ui::animation::{AnimationConfig, TransitionPlayer};
use crate::ui::reactive::state::State;
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widgets::binding::write_if_changed;
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent, WidgetId,
    WidgetTree,
};
use crate::widget;
use std::cell::Cell;

// 复用反馈层共享的颜色衰减辅助。
use crate::ui::widgets::feedback::fade_token_color;

// 声明颜色面板的私有表面几何模块。
mod geometry;
// 声明颜色选择器的私有弹层缓存方法模块。
mod methods;
// 声明颜色选择器的 UIX 静态视觉与主题解析模块。
mod presentation;

use presentation::*;

// 引入颜色面板的绝对坐标转换、表面裁剪与共享网格指标。
use geometry::{ColorPanelGeometry, absolute_color_popup_rect, color_surface_rect};

// ColorPicker — 颜色选择器。
widget! {
    /// 通过窗口内预设色面板选择并提交颜色值的组件。
    pub struct ColorPicker {
        value: Cell<Color>,
        value_binding: Option<State<Color>>,
        open: bool,
        transition: TransitionPlayer,
        closing: bool,
        transition_dirty: bool,
        preset_colors: Vec<Color>,
        hovered: bool,
        highlighted_idx: Option<usize>,
        focused: bool,
        picker_size: ControlSize,
        last_frame: Cell<Option<Rect>>,
        pending_change: Cell<Option<Color>>,
        // 缓存相对触发器原点的最终颜色面板矩形。
        popup_rect: Cell<Rect>,
        // 缓存最近登记或绘制使用的逻辑表面。
        surface_rect: Cell<Option<Rect>>,
        // 缓存最近登记使用的绝对触发器锚点。
        popup_anchor_frame: Cell<Option<Rect>>,
        // 累计当前呈现周期内的绝对颜色面板脏区。
        popup_damage_rect: Cell<Rect>,
        // 同目录 UIX 生成的唯一静态视觉表。
        #[snapshot(skip)]
        visual: &'static ColorPickerVisual,
    }


    tab_index => (&self) -> i32 { 1 }
    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        self.sync_bound_value();
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let frame = self.interaction_frame();
                if frame.contains(*pos) {
                    if self.open {
                        self.close();
                    } else {
                        self.open();
                    }
                    self.focused = true;
                    return EventResult::Handled;
                }
                if self.open {
                    // 读取登记、绘制与命中共享的实际颜色面板。
                    let popup = self.interaction_popup_rect(frame);
                    // 使用实际缩放网格解析指针命中的颜色索引。
                    if let Some(index) =
                        // 构造与绘制共享的颜色面板指标。
                        ColorPanelGeometry::new(popup, self.preset_colors.len()).index_at(*pos)
                    {
                        self.highlighted_idx = Some(index);
                        self.commit_value(self.preset_colors[index]);
                        self.close();
                        return EventResult::Handled;
                    }
                    // 面板空白区域仍会关闭当前选择器。
                    if popup.contains(*pos) {
                        self.close();
                        return EventResult::Handled;
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                let frame = self.interaction_frame();
                let old_hovered = self.hovered;
                let old_highlight = self.highlighted_idx;
                if self.open {
                    self.hovered = frame.contains(*pos);
                    // 读取登记、绘制与悬停共享的实际颜色面板。
                    let popup = self.interaction_popup_rect(frame);
                    // 使用实际缩放网格解析悬停颜色。
                    self.highlighted_idx = ColorPanelGeometry::new(
                        // 传入当前面板矩形。
                        popup,
                        // 传入当前预设颜色数量。
                        self.preset_colors.len(),
                    )
                    // 解析当前指针位置。
                    .index_at(*pos)
                    .or_else(|| self.default_highlight());
                } else {
                    self.hovered = frame.contains(*pos);
                    self.highlighted_idx = None;
                }
                if self.hovered != old_hovered || self.highlighted_idx != old_highlight {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerLeave => {
                let old_hovered = self.hovered;
                let old_highlight = self.highlighted_idx;
                self.hovered = false;
                self.highlighted_idx = if self.open {
                    self.default_highlight()
                } else {
                    None
                };
                if self.hovered != old_hovered || self.highlighted_idx != old_highlight {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::FocusIn => { self.focused = true; EventResult::Handled }
            SystemEvent::FocusOut => { self.close(); self.focused = false; EventResult::Handled }
            SystemEvent::KeyDown { key, .. } => {
                match key {
                    KeyCode::Escape if self.is_present() => {
                        self.close();
                        EventResult::Handled
                    }
                    KeyCode::Space | KeyCode::Enter if self.open => {
                        if let Some(index) = self.highlighted_idx {
                            self.commit_value(self.preset_colors[index]);
                        }
                        self.close();
                        EventResult::Handled
                    }
                    KeyCode::Space | KeyCode::Enter => {
                        self.open();
                        self.focused = true;
                        EventResult::Handled
                    }
                    KeyCode::Left if self.open => {
                        self.move_highlight(ColorMove::Previous);
                        EventResult::Handled
                    }
                    KeyCode::Right if self.open => {
                        self.move_highlight(ColorMove::Next);
                        EventResult::Handled
                    }
                    KeyCode::Up if self.open => {
                        self.move_highlight(ColorMove::PreviousRow);
                        EventResult::Handled
                    }
                    KeyCode::Down if self.open => {
                        self.move_highlight(ColorMove::NextRow);
                        EventResult::Handled
                    }
                    KeyCode::Home if self.open && !self.preset_colors.is_empty() => {
                        self.highlighted_idx = Some(0);
                        EventResult::Handled
                    }
                    KeyCode::End if self.open && !self.preset_colors.is_empty() => {
                        self.highlighted_idx = Some(self.preset_colors.len() - 1);
                        EventResult::Handled
                    }
                    _ => EventResult::NotHandled,
                }
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .take()
            .map(|value| SemanticEvent::change(id, value.to_string()))
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        if self.is_present() {
            // 读取最近登记或首帧有限回退表面。
            let surface = self.surface_or_fallback(frame);
            // 解析并缓存当前实际颜色面板。
            let popup = self.remember_popup_rect(frame, surface);
            // 将相对颜色面板转换为窗口绝对坐标。
            let popup = absolute_color_popup_rect(frame, popup);
            // 触发器与当前颜色面板命中框共同收敛到表面内。
            color_surface_rect(frame, popup, surface)
        } else {
            frame
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.sync_bound_value();
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        // 触发色块、棋盘格与弹层共享一次 UIX 主题解析。
        let visual = self.visual.resolve(ctx.tokens());
        let layout = self.visual.layout;
        let nominal_height = self.control_height();
        let scale = if nominal_height > 0.0 {
            (frame.h.min(frame.w) / nominal_height).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let available = frame.w.min(frame.h).max(0.0);
        let swatch_inset = (layout.swatch_inset * scale).min(available * 0.5);
        let swatch_size = (available - swatch_inset * 2.0).max(0.0);

        ctx.push_clip(frame);
        if swatch_size > 0.0 {
            let swatch = Rect::new(
                frame.x + swatch_inset,
                frame.y + swatch_inset,
                swatch_size,
                swatch_size,
            );
            let radius = Some(Radius::uniform(
                (visual.swatch_radius * scale).min(swatch_size * 0.5),
            ));
            paint_transparency_checkerboard(
                ctx,
                swatch,
                scale,
                layout,
                visual.checker_light,
                visual.checker_dark,
            );
            ctx.fill_rect(swatch, self.value.get(), radius);
            let border_c = if self.hovered || self.focused {
                visual.primary
            } else {
                visual.border
            };
            ctx.stroke_rect(
                swatch,
                border_c,
                layout.swatch_border_width * scale,
                radius,
            );

            if self.focused {
                let focus_outset = (layout.focus_outset * scale).min(swatch_inset);
                ctx.stroke_rect(
                    Rect::new(
                        swatch.x - focus_outset,
                        swatch.y - focus_outset,
                        swatch.w + focus_outset * 2.0,
                        swatch.h + focus_outset * 2.0,
                    ),
                    visual.primary,
                    layout.focus_border_width * scale,
                    radius,
                );
            }
        }
        ctx.pop_clip();

        if self.is_present() {
            // 从绘制上下文读取当前逻辑表面尺寸。
            let surface_size = ctx.logical_surface_size();
            // 将窗口原点与逻辑尺寸组合为当前表面矩形。
            let surface = Rect::new(0.0, 0.0, surface_size.w, surface_size.h);
            // 在绘制前解析并缓存同帧最终颜色面板几何。
            let popup = self.remember_popup_rect(frame, surface);
            // 将相对触发器缓存转换为窗口绝对颜色面板。
            let panel_rect = absolute_color_popup_rect(frame, popup);
            // 构造绘制与命中共享的缩放网格指标。
            let panel_geometry =
                // 使用实际颜色面板和当前预设数量。
                ColorPanelGeometry::new(panel_rect, self.preset_colors.len());
            let opacity = self.transition.opacity_progress.clamp(0.0, 1.0);
            let bg = fade_token_color(visual.popup_background, opacity);
            let border = fade_token_color(visual.border, opacity);
            let panel_radius = Some(Radius::uniform(visual.panel_radius));
            // 将整个弹层绘制限制在当前逻辑表面内。
            ctx.push_clip(surface);
            ctx.push_clip(panel_rect);
            ctx.fill_rect(panel_rect, bg, panel_radius);
            ctx.stroke_rect(
                panel_rect,
                border,
                self.visual.chrome.panel_border_width,
                panel_radius,
            );

            for (i, c) in self.preset_colors.iter().enumerate() {
                // 从共享网格读取当前色块的实际缩放矩形。
                let Some(cell_rect) = panel_geometry.cell_rect(i) else {
                    // 无效或不可见网格不生成色块绘制命令。
                    continue;
                };
                // 使用较小轴比例缩放圆角、描边和选中图标。
                let visual_scale = panel_geometry.visual_scale();
                // 构造与实际色块尺寸一致的圆角。
                let cell_radius = Some(Radius::uniform(layout.cell_radius * visual_scale));
                ctx.fill_rect(cell_rect, fade_token_color(*c, opacity), cell_radius);
                if self.highlighted_idx == Some(i) {
                    // 高亮描边：按色块亮度取黑白 token 对比色。
                    let highlight_color = if c.is_light() {
                        visual.contrast_light
                    } else {
                        visual.contrast_dark
                    };
                    ctx.stroke_rect(
                        cell_rect,
                        fade_token_color(highlight_color, opacity),
                        // 缩放描边以避免窄色块被边框完全覆盖。
                        layout.highlight_border_width * visual_scale,
                        // 复用当前色块圆角。
                        cell_radius,
                    );
                }
                if self.value.get() == *c {
                    // 选中图标色：按色块亮度取黑白 token 对比色。
                    let icon_color = if c.is_light() {
                        visual.contrast_light
                    } else {
                        visual.contrast_dark
                    };
                    crate::ui::widgets::icon::Icon::paint_in_frame(
                        ctx,
                        self.visual.icons.selected,
                        cell_rect,
                        fade_token_color(icon_color, opacity),
                        // 缩放选中图标以保持在实际色块内。
                        layout.selected_icon_size * visual_scale,
                    );
                }
            }
            ctx.pop_clip();
            // 恢复颜色面板外层的逻辑表面裁剪。
            ctx.pop_clip();
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        // 合并当前与本呈现周期历史颜色面板并裁剪到表面。
        self.presentation_dirty_rect(frame)
    }

    overlay_entry => (&self, id: WidgetId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        self.is_present().then(|| {
            // 读取最近登记的表面或首帧有限回退。
            let surface = self.surface_or_fallback(frame);
            // 解析并缓存当前实际颜色面板。
            let popup = self.remember_popup_rect(frame, surface);
            // 将相对面板转换为窗口绝对登记边界。
            let bounds = absolute_color_popup_rect(frame, popup);
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Popover)
                // OverlayStack 只登记实际颜色面板，不并入触发器。
                .bounds(bounds)
                .z_index(self.visual.chrome.overlay_z)
        })
    }

    // 使用组件树提供的同帧表面创建颜色面板登记。
    overlay_entry_for_surface => (&self, id: WidgetId, frame: Rect, surface: Rect) -> Option<crate::ui::OverlayEntry> {
        // 在旧登记入口执行前刷新表面与实际面板缓存。
        self.remember_popup_rect(frame, surface);
        // 复用统一的颜色面板登记逻辑。
        self.overlay_entry(id, frame)
    }

    update_animation => (&mut self, dt: f64) -> bool {
        if !self.is_present() || self.transition.finished {
            self.transition_dirty = false;
            return false;
        }

        self.transition.update(dt);
        self.transition_dirty = true;

        if self.closing && self.transition.finished {
            self.open = false;
            self.closing = false;
            self.highlighted_idx = None;
        }

        self.is_present() && !self.transition.finished
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.transition_dirty {
            // 动画脏区复用当前呈现周期的完整表面约束结果。
            self.presentation_dirty_rect(frame)
        } else {
            Rect::zero()
        }
    }
}
impl ColorPicker {
    fn intrinsic_size(&self) -> Size {
        let height = self.control_height();
        Size::new(height, height)
    }

    /// 创建使用内置色板、当前配置尺寸且未展开的颜色选择器。
    pub fn new() -> Self {
        let config = crate::ui::widget_runtime::config::use_config();
        let visual = COLOR_PICKER_VISUAL_REF;
        Self {
            value: Cell::new(Color::default()),
            value_binding: None,
            open: false,
            transition: TransitionPlayer::new(AnimationConfig::fade_in(
                visual.motion.enter_duration,
            )),
            closing: false,
            transition_dirty: false,
            // 预设色板从 UIX 展示数据解包构造，实例继续拥有可协调的 Vec。
            preset_colors: visual
                .presets
                .values()
                .into_iter()
                .map(|c| {
                    Color::from_rgba(
                        ((c >> 16) & 0xFF) as u8,
                        ((c >> 8) & 0xFF) as u8,
                        (c & 0xFF) as u8,
                        255,
                    )
                })
                .collect(),
            hovered: false,
            highlighted_idx: None,
            focused: false,
            picker_size: config.size,
            last_frame: Cell::new(None),
            pending_change: Cell::new(None),
            // 初始尚无最终颜色面板。
            popup_rect: Cell::new(Rect::zero()),
            // 初始尚未记录逻辑表面。
            surface_rect: Cell::new(None),
            // 初始尚未记录绝对触发器锚点。
            popup_anchor_frame: Cell::new(None),
            // 初始呈现周期没有历史颜色面板脏区。
            popup_damage_rect: Cell::new(Rect::zero()),
            // 默认实例直接引用 UIX 生成的唯一静态视觉表。
            visual,
        }
    }
    /// 将颜色绑定到外部 `State<Color>`。
    pub fn value(mut self, state: &State<Color>) -> Self {
        self.value_binding = Some(state.clone());
        self.value.set(state.get());
        self
    }

    /// 设置非受控颜色选择器的初始值。
    pub fn default_value(mut self, value: Color) -> Self {
        self.value_binding = None;
        self.value.set(value);
        self
    }

    /// 返回组件当前缓存值；controlled 用法应以绑定的 `State` 为真值来源。
    pub fn current_value(&self) -> Color {
        self.value.get()
    }

    /// 设置颜色触发器采用的控件尺寸规格。
    pub fn size(mut self, size: ControlSize) -> Self {
        self.picker_size = size;
        self
    }

    /// 返回颜色面板当前是否处于逻辑展开状态。
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// 返回颜色面板当前是否仍需呈现，包括退出过渡阶段。
    pub fn is_present(&self) -> bool {
        self.open || self.closing
    }

    /// 展开颜色面板，并高亮当前颜色或首个预设色。
    pub fn open(&mut self) {
        // 仅从完全未呈现状态进入时开始新的面板呈现周期。
        if !self.is_present() {
            // 清除上一次完整关闭后留下的表面与脏区缓存。
            self.reset_popup_presentation();
        }
        self.open = true;
        self.closing = false;
        self.highlighted_idx = self.default_highlight();
        self.transition =
            TransitionPlayer::new(AnimationConfig::fade_in(self.visual.motion.enter_duration));
        self.transition_dirty = true;
    }

    /// 关闭颜色面板并在已呈现时启动退出过渡。
    pub fn close(&mut self) {
        if !self.is_present() {
            self.open = false;
            self.closing = false;
            self.transition_dirty = false;
            return;
        }

        self.open = false;
        self.closing = true;
        self.highlighted_idx = self.selected_index();
        self.transition =
            TransitionPlayer::new(AnimationConfig::fade_out(self.visual.motion.exit_duration));
        self.transition_dirty = true;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::ColorPicker {
            value: self.value.get(),
            preset_colors: self.preset_colors.clone(),
            open: self.open,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let controlled_value = next.value_binding.as_ref().map(|_| next.value.get());
        let visual_changed = !std::ptr::eq(self.visual, next.visual);
        self.value_binding = next.value_binding;
        self.preset_colors = next.preset_colors;
        self.picker_size = next.picker_size;
        self.visual = next.visual;
        if visual_changed {
            // 静态视觉几何变化后丢弃上一呈现周期的表面缓存。
            self.reset_popup_presentation();
        }
        if let Some(value) = controlled_value {
            self.value.set(value);
        }
        if self.open {
            self.highlighted_idx = self.default_highlight();
        }
    }

    fn sync_bound_value(&self) {
        if let Some(state) = self.value_binding.as_ref() {
            self.value.set(state.get());
        }
    }

    fn commit_value(&self, value: Color) {
        if self.value.get() == value {
            return;
        }
        self.value.set(value);
        write_if_changed(self.value_binding.as_ref(), value);
        self.pending_change.set(Some(value));
    }

    fn control_height(&self) -> f32 {
        crate::ui::widget_runtime::config::control_height(self.picker_size)
    }

    fn interaction_frame(&self) -> Rect {
        self.last_frame.get().unwrap_or_else(|| {
            Rect::new(0.0, 0.0, self.intrinsic_size().w, self.intrinsic_size().h)
        })
    }

    fn selected_index(&self) -> Option<usize> {
        let value = self.value.get();
        self.preset_colors.iter().position(|color| *color == value)
    }

    fn default_highlight(&self) -> Option<usize> {
        self.selected_index()
            .or_else(|| (!self.preset_colors.is_empty()).then_some(0))
    }

    fn move_highlight(&mut self, direction: ColorMove) {
        let Some(last) = self.preset_colors.len().checked_sub(1) else {
            self.highlighted_idx = None;
            return;
        };
        let current = self.highlighted_idx.unwrap_or(0).min(last);
        let next = match direction {
            ColorMove::Previous => current.saturating_sub(1),
            ColorMove::Next => (current + 1).min(last),
            ColorMove::PreviousRow => {
                current.saturating_sub(self.visual.layout.panel_columns.max(1))
            }
            ColorMove::NextRow => (current + self.visual.layout.panel_columns.max(1)).min(last),
        };
        self.highlighted_idx = Some(next);
    }
}

impl Default for ColorPicker {
    fn default() -> Self {
        Self::new()
    }
}

fn paint_transparency_checkerboard(
    ctx: &mut PaintContext,
    frame: Rect,
    scale: f32,
    layout: ColorPickerLayoutVisual,
    light: Color,
    dark: Color,
) {
    let tile = (layout.checker_tile * scale).max(layout.checker_tile_min);
    let columns = (frame.w / tile).ceil() as usize;
    let rows = (frame.h / tile).ceil() as usize;
    ctx.push_clip(frame);
    // 棋盘格明暗色均来自 UIX 视觉角色。
    ctx.fill_rect(frame, light, None);
    for row in 0..rows {
        for column in 0..columns {
            if (row + column) % 2 == 0 {
                ctx.fill_rect(
                    Rect::new(
                        frame.x + column as f32 * tile,
                        frame.y + row as f32 * tile,
                        tile,
                        tile,
                    ),
                    dark,
                    None,
                );
            }
        }
    }
    ctx.pop_clip();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColorMove {
    Previous,
    Next,
    PreviousRow,
    NextRow,
}

// 把 ColorPicker Rust 状态内核与 UIX 静态视觉组合为单一组件节点。
fn build_color_picker_view(
    mut kernel: ColorPicker,
    visual: &'static ColorPickerVisual,
) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

// 为 UIX 根提供稳定的 Rust 内核绑定名称。
fn build_color_picker_uix_root(kernel: ColorPicker) -> ViewNode {
    crate::uix!("src/ui/widgets/input/color_picker/color_picker.uix")
}

impl View for ColorPicker {
    fn build(self) -> ViewNode {
        build_color_picker_uix_root(self)
    }
}
