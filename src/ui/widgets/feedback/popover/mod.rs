use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::component;

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::{Color, FillRule, PathBuilder, Radius};
use crate::ui::SnapshotFields;
use crate::ui::animation::{AnimationConfig, TransitionPlayer, presets};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{EventResult, KeyCode, MouseButton, State, SystemEvent, WidgetTree};

mod geometry;

use self::geometry::*;

const POPOVER_WIDTH: f32 = 220.0;
const POPOVER_HEIGHT: f32 = 100.0;
const TRIGGER_WIDTH: f32 = 80.0;
// Popover 触发器高度（28.0）；cascader 触发器为 32.0，组件独立设计。
const TRIGGER_HEIGHT: f32 = 28.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PopoverPressTarget {
    Trigger,
}

/// Popover placement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PopoverPlacement {
    /// 在触发器上方居中显示。
    Top,
    /// 在触发器上方显示并对齐左边缘。
    TopLeft,
    /// 在触发器上方显示并对齐右边缘。
    TopRight,
    /// 在触发器下方居中显示。
    Bottom,
    /// 在触发器下方显示并对齐左边缘。
    BottomLeft,
    /// 在触发器下方显示并对齐右边缘。
    BottomRight,
    /// 在触发器左侧居中显示。
    Left,
    /// 在触发器左侧显示并对齐顶部。
    LeftTop,
    /// 在触发器左侧显示并对齐底部。
    LeftBottom,
    /// 在触发器右侧居中显示。
    Right,
    /// 在触发器右侧显示并对齐顶部。
    RightTop,
    /// 在触发器右侧显示并对齐底部。
    RightBottom,
}

/// Popover trigger mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PopoverTrigger {
    /// 点击触发器时切换显示状态。
    Click,
    /// 指针悬停于触发器时显示。
    Hover,
    /// 触发器获得键盘焦点时显示。
    Focus,
}

component! {
    /// 拥有唯一触发器并按配置交互显示窗口内内容气泡的组合组件。
    pub struct Popover {
        title: String,
        content: String,
        visible: bool,
        placement: PopoverPlacement,
        trigger: PopoverTrigger,
        arrow: bool,
        background: Option<Color>,
        custom_trigger: bool,
        #[snapshot(skip)]
        custom_trigger_view:
            Option<Rc<RefCell<Option<crate::ui::view::ViewNode>>>>,
        #[snapshot(skip)]
        open_binding: Option<State<bool>>,
        timer: f32,
        enter_animation: AnimationConfig,
        leave_animation: AnimationConfig,
        transition: TransitionPlayer,
        closing: bool,
        transition_dirty: bool,
        focused: bool,
        hovered: bool,
        pressed_target: Option<PopoverPressTarget>,
        pressed_key: Option<KeyCode>,
        last_frame: Cell<Rect>,
        popup_rect: Cell<Rect>,
        surface_rect: Cell<Rect>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    hit_test_children => (&self) -> bool { false }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(crate::ui::ComponentId, Rect)>
    {
        children.iter().map(|child| (child.id, Self::normalize_frame(frame))).collect()
    }

    build_view_children => (&self) -> Vec<crate::ui::view::ViewNode> {
        self.custom_trigger_view
            .as_ref()
            .and_then(|view| view.borrow_mut().take())
            .into_iter()
            .collect()
    }

    tab_index => (&self) -> i32 { 1 }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        self.sync_bound_open();
        match event {
            SystemEvent::FocusIn => {
                self.focused = true;
                return EventResult::Handled;
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.cancel_pending_activation();
                // 焦点离开即关闭弹层（与 Dropdown 的 FocusOut 语义对齐）。
                // hover 触发的打开不依赖焦点：hover 路径由 PointerEnter/Leave
                // 驱动，FocusOut 只发生在组件曾获得焦点的场景，不会误关
                // 纯 hover 打开的弹层。
                self.close();
                return EventResult::Handled;
            }
            SystemEvent::KeyDown { key: key @ (KeyCode::Enter | KeyCode::Space), .. }
                if self.trigger == PopoverTrigger::Click =>
            {
                self.pressed_key = Some(*key);
                return EventResult::Handled;
            }
            SystemEvent::KeyUp { key: key @ (KeyCode::Enter | KeyCode::Space), .. }
                if self.trigger == PopoverTrigger::Click =>
            {
                let matches = self.pressed_key.take() == Some(*key);
                if matches {
                    self.toggle();
                }
                return EventResult::Handled;
            }
            SystemEvent::KeyDown { key: KeyCode::Escape, .. } if self.is_present() => {
                self.cancel_pending_activation();
                self.close();
                return EventResult::Handled;
            }
            SystemEvent::PointerMove { pos, .. } => {
                let hovered = self.trigger_rect().contains(*pos);
                if self.hovered != hovered {
                    self.hovered = hovered;
                    return EventResult::Handled;
                }
            }
            SystemEvent::PointerLeave => {
                let changed = self.hovered || self.pressed_target.is_some();
                self.hovered = false;
                self.pressed_target = None;
                if self.trigger == PopoverTrigger::Hover && self.is_present() {
                    self.close();
                    return EventResult::Handled;
                }
                if changed {
                    return EventResult::Handled;
                }
            }
            _ => {}
        }
        match self.trigger {
            PopoverTrigger::Click => {
                match event {
                    SystemEvent::PointerDown {
                        pos,
                        button: MouseButton::Left,
                        ..
                    } => {
                        if self.trigger_rect().contains(*pos) {
                            self.focused = true;
                            self.pressed_target = Some(PopoverPressTarget::Trigger);
                            return EventResult::Handled;
                        }
                        if self.is_present() && self.popup_rect.get().contains(*pos) {
                            return EventResult::Handled;
                        }
                        if self.is_present() {
                            self.cancel_pending_activation();
                            self.close();
                            return EventResult::Handled;
                        }
                    }
                    SystemEvent::PointerUp {
                        pos,
                        button: MouseButton::Left,
                        ..
                    } => {
                        let pressed = self.pressed_target.take();
                        if pressed == Some(PopoverPressTarget::Trigger)
                            && self.trigger_rect().contains(*pos)
                        {
                            self.toggle();
                        }
                        return EventResult::Handled;
                    }
                    _ => {}
                }
            }
            PopoverTrigger::Hover => {
                if let SystemEvent::PointerEnter = event {
                    self.hovered = true;
                    self.open();
                    self.timer = 0.0;
                    return EventResult::Handled;
                }
            }
            PopoverTrigger::Focus => {}
        }
        EventResult::NotHandled
    }

    on_focus_within => (&mut self, focused: bool) -> EventResult {
        if self.trigger != PopoverTrigger::Focus {
            return EventResult::NotHandled;
        }
        if focused {
            self.open();
        } else {
            self.close();
        }
        EventResult::Handled
    }


    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let frame = Self::normalize_frame(frame);
        let surface_size = ctx.logical_surface_size();
        let surface = Self::normalize_frame(Rect::new(0.0, 0.0, surface_size.w, surface_size.h));
        self.last_frame.set(frame);
        self.surface_rect.set(surface);
        let popup_geometry = resolve_popover_geometry(
            frame,
            surface,
            self.placement,
            self.arrow,
            POPOVER_WIDTH,
            POPOVER_HEIGHT,
        );
        self.popup_rect.set(Rect::new(
            popup_geometry.popup.x - frame.x,
            popup_geometry.popup.y - frame.y,
            popup_geometry.popup.w,
            popup_geometry.popup.h,
        ));

        let bg = self
            .background
            .unwrap_or_else(|| ctx.tokens().color_bg_elevated());
        let border = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));

        ctx.push_clip(surface);
        if self.hovered || self.pressed_target.is_some() {
            ctx.fill_rect(
                frame,
                if self.pressed_target.is_some() {
                    ctx.tokens().color_fill_secondary()
                } else {
                    ctx.tokens().color_fill_tertiary()
                },
                r,
            );
        }
        ctx.stroke_rect(
            frame,
            if self.focused && tree.keyboard_focus_visible() {
                ctx.tokens().color_primary()
            } else {
                border
            },
            if self.focused && tree.keyboard_focus_visible() { 2.0 } else { 1.0 },
            r,
        );
        if !self.custom_trigger {
            Self::paint_elided_text(ctx, "Popover", frame, text_secondary, ctx.tokens().font_size_sm(), true);
        }

        if self.is_present() && popup_geometry.popup.w > 0.0 && popup_geometry.popup.h > 0.0 {
            let opacity = self.transition.opacity_progress.clamp(0.0, 1.0);
            let popup_bg = fade_color(bg, opacity);
            let popup_border = fade_color(border, opacity);
            let popup_text = fade_color(text_color, opacity);
            let popup_secondary = fade_color(text_secondary, opacity);
            let pop_rect = self.transitioned_rect(popup_geometry.popup);
            let shadow = ctx.tokens().box_shadow_secondary();
            ctx.draw_box_shadow(
                pop_rect,
                shadow.layer_1.2,
                shadow.layer_1.0,
                shadow.layer_1.1,
                fade_color(shadow.layer_1.3, opacity),
                r,
            );
            ctx.fill_rect(pop_rect, popup_bg, r);
            ctx.stroke_rect(pop_rect, popup_border, 1.0, r);

            if self.arrow {
                draw_popover_arrow(
                    ctx,
                    frame,
                    pop_rect,
                    popup_geometry.placement,
                    popup_bg,
                );
            }

            let inset = 12.0_f32.min(pop_rect.w * 0.5);
            let content_width = (pop_rect.w - inset * 2.0).max(0.0);
            let title_height = 32.0_f32.min(pop_rect.h);
            if !self.title.is_empty() && content_width > 0.0 {
                let title_rect = Rect::new(
                    pop_rect.x + inset,
                    pop_rect.y,
                    content_width,
                    title_height,
                );
                Self::paint_elided_text(ctx, &self.title, title_rect, popup_text, ctx.tokens().font_size(), false);
                if pop_rect.h > title_height {
                    ctx.fill_rect(
                        Rect::new(
                            pop_rect.x + inset,
                            pop_rect.y + title_height,
                            content_width,
                            1.0,
                        ),
                        popup_border,
                        None,
                    );
                }
            }
            let content_top = if self.title.is_empty() {
                pop_rect.y
            } else {
                (pop_rect.y + title_height + 1.0).min(pop_rect.y + pop_rect.h)
            };
            let content_rect = Rect::new(
                pop_rect.x + inset,
                content_top,
                content_width,
                (pop_rect.y + pop_rect.h - content_top).max(0.0),
            );
            Self::paint_elided_text(
                ctx,
                &self.content,
                content_rect,
                popup_secondary,
                12.0,
                false,
            );
        }
        ctx.pop_clip();
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        self.transition_dirty_rect(frame)
    }

    overlay_entry => (&self, id: crate::ui::ComponentId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        if !self.is_present() {
            return None;
        }

        let frame = Self::normalize_frame(frame);
        let popup = expand_popover_rect(self.absolute_popup_rect(frame), 12.0);
        let popup = self
            .transition_sweep_rect(popup)
            .intersect(&self.surface_or_fallback(frame))
            .unwrap_or_default();
        Some(
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Popover)
                .bounds(popup)
                .z_index(900)
                // 外部点击由树的 System 私有取消端口回调 owner 关闭。
                .dismiss_on_outside(true),
        )
    }

    // 布局阶段以当前逻辑表面刷新气泡几何，再沿用既有登记策略。
    overlay_entry_for_surface => (&self, id: crate::ui::ComponentId, frame: Rect, surface: Rect) -> Option<crate::ui::OverlayEntry> {
        // 记录与本次 OverlayStack 重建一致的表面边界。
        self.surface_rect.set(Self::normalize_frame(surface));
        // 复用统一的浮层登记与动画扫掠逻辑。
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
            self.visible = false;
            self.closing = false;
        }

        self.is_present() && !self.transition.finished
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.transition_dirty {
            self.transition_dirty_rect(frame)
        } else {
            Rect::zero()
        }
    }
}

impl Default for Popover {
    fn default() -> Self {
        Self::new("")
    }
}
