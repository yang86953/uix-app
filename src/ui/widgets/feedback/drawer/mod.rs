use std::cell::Cell;

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::{Color, Radius};
use crate::platform::windowing::ControlSize;
use crate::ui::SnapshotFields;
use crate::ui::animation::{AnimationConfig, TransitionPlayer};
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::widget::WidgetCore;
use crate::widget;
// 引入受控状态句柄与 Drawer 既有事件、树契约。
use crate::ui::{EventResult, MouseButton, State, SystemEvent, WidgetTree};

mod presentation;
use self::presentation::*;

mod methods;

/// 抽屉面板进入窗口的边缘方向。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DrawerPlacement {
    /// 从窗口右侧进入。
    Right,
    /// 从窗口左侧进入。
    Left,
    /// 从窗口顶部进入。
    Top,
    /// 从窗口底部进入。
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DrawerPointerTarget {
    Trigger,
    Close,
    Mask,
}

// 保存 Drawer 受控打开状态的唯一外部事实源句柄。
pub(crate) struct ControlledDrawerOpen {
    // 克隆 State 句柄，不复制其中的布尔事实。
    state: State<bool>,
}

// 为受控句柄提供窄构造与读取契约。
impl ControlledDrawerOpen {
    // 从声明端状态建立共享句柄。
    fn new(state: &State<bool>) -> Self {
        // 克隆共享句柄以跨声明重建保留同一事实源。
        Self {
            // 保存共享状态句柄。
            state: state.clone(),
        }
    }

    // 读取声明端当前期望的打开状态。
    fn want_open(&self) -> bool {
        // 返回 State 中的唯一业务事实。
        self.state.get()
    }
}

widget! {
    /// Sliding drawer panel.
    pub struct Drawer {
        // 精确容量文本避免为不可变标题保留 String capacity 字段。
        title: Box<str>,
        visible: bool,
        width: f32,
        height: f32,
        drawer_size: ControlSize,
        placement: DrawerPlacement,
        closable: bool,
        mask_closable: bool,
        mask: bool,
        // 保存可选的统一 overlay backdrop blur 请求。
        backdrop_blur: Option<crate::ui::OverlayBackdropBlur>,
        footer_visible: bool,
        // 精确容量文本避免为不可变附加文案保留 String capacity 字段。
        extra: Box<str>,
        enter_animation: Option<AnimationConfig>,
        leave_animation: Option<AnimationConfig>,
        // 保存可选受控打开绑定，生命周期仍由 Drawer 自身拥有。
        controlled: Option<ControlledDrawerOpen>,
        pub(crate) transition: TransitionPlayer,
        closing: bool,
        pub(crate) transition_dirty: bool,
        layout_requested: Cell<bool>,
        last_frame: Cell<Rect>,
        last_surface_w: Cell<f32>,
        last_surface_h: Cell<f32>,
        last_trigger_rect: Cell<Rect>,
        last_panel_rect: Cell<Rect>,
        close_hovered: Cell<bool>,
        pressed_target: Cell<Option<DrawerPointerTarget>>,
        activation_key: Cell<Option<crate::ui::KeyCode>>,
        // 全部实例只保存指向 UIX 唯一视觉静态项的共享引用。
        #[snapshot(skip)]
        visual: &'static DrawerVisual,
    }

    // Closed drawers still render and receive input through their trigger.
    visible => (&self) -> bool { true }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    tab_index => (&self) -> i32 { i32::from(!self.is_present()) }

    hit_test_frame => (&self, actual_frame: Rect) -> Rect {
        self.last_frame.set(Self::normalize_frame(actual_frame));
        if self.mask && self.is_present() {
            Rect::new(0.0, 0.0, self.last_surface_w.get(), self.last_surface_h.get())
        } else if !self.is_present() {
            let trigger = self.trigger_rect_for_size(actual_frame.w, actual_frame.h);
            self.last_trigger_rect.set(trigger);
            Rect::new(
                actual_frame.x + trigger.x,
                actual_frame.y + trigger.y,
                trigger.w,
                trigger.h,
            )
        } else {
            actual_frame
        }
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        // 离场期间吞掉输入，防止关闭动作重入并重新启动动画。
        if self.closing {
            // 清理残留按压状态并保持当前离场生命周期。
            self.cancel_interaction();
            // 覆盖层离场时继续阻断下层输入。
            return EventResult::Handled;
        }
        if !self.is_present() {
            let trigger = self.trigger_rect_local();
            return match event {
                SystemEvent::PointerDown {
                    pos,
                    button: MouseButton::Left,
                    ..
                } if trigger.contains(*pos) => {
                    self.close_hovered.set(true);
                    self.pressed_target.set(Some(DrawerPointerTarget::Trigger));
                    EventResult::Handled
                }
                SystemEvent::PointerUp {
                    pos,
                    button: MouseButton::Left,
                    ..
                } if self.pressed_target.replace(None) == Some(DrawerPointerTarget::Trigger) => {
                    let released_inside = trigger.contains(*pos);
                    self.close_hovered.set(released_inside);
                    if released_inside {
                        self.open();
                    }
                    EventResult::Handled
                }
                SystemEvent::PointerMove { pos, .. } => {
                    let hovered = trigger.contains(*pos);
                    if self.close_hovered.replace(hovered) != hovered {
                        EventResult::Handled
                    } else {
                        EventResult::NotHandled
                    }
                }
                SystemEvent::PointerLeave | SystemEvent::FocusOut => {
                    self.cancel_interaction();
                    EventResult::Handled
                }
                SystemEvent::KeyDown {
                    key: key @ (crate::ui::KeyCode::Enter | crate::ui::KeyCode::Space),
                    ..
                } => {
                    self.activation_key.set(Some(*key));
                    EventResult::Handled
                }
                SystemEvent::KeyUp {
                    key: key @ (crate::ui::KeyCode::Enter | crate::ui::KeyCode::Space),
                    ..
                } if self.activation_key.replace(None) == Some(*key) => {
                    self.open();
                    EventResult::Handled
                }
                SystemEvent::FocusIn => EventResult::Handled,
                _ => EventResult::NotHandled,
            };
        }

        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let target = self.pointer_target_at(*pos);
                self.close_hovered
                    .set(target == Some(DrawerPointerTarget::Close));
                self.pressed_target.set(target);
                if target.is_some() {
                    return EventResult::Handled;
                }
                EventResult::Handled
            }
            SystemEvent::PointerUp {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let armed = self.pressed_target.replace(None);
                let target = self.pointer_target_at(*pos);
                self.close_hovered
                    .set(target == Some(DrawerPointerTarget::Close));
                if armed.is_some() && armed == target {
                    self.close();
                }
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                let hovered = self.pointer_target_at(*pos) == Some(DrawerPointerTarget::Close);
                self.close_hovered.set(hovered);
                EventResult::Handled
            }
            SystemEvent::PointerLeave | SystemEvent::FocusOut => {
                self.cancel_interaction();
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } => {
                // Escape 与 Modal 对齐：始终采用关闭语义，closable 只控制
                // 关闭按钮可见性，不约束键盘关闭路径。
                if *key == crate::ui::KeyCode::Escape {
                    self.close();
                    return EventResult::Handled;
                }
                EventResult::Handled
            }
            _ => EventResult::Handled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.last_frame.set(Self::normalize_frame(frame));
        let loc = crate::ui::widget_runtime::locale::use_locale();
        let visual = self.visual;
        if !self.is_present() {
            let local_trigger = self.trigger_rect_for_size(frame.w, frame.h);
            self.last_trigger_rect.set(local_trigger);
            // 关闭态只解析触发器实际使用的主题角色。
            let resolved = visual.resolve_trigger(ctx.tokens());
            let primary = if self.pressed_target.get() == Some(DrawerPointerTarget::Trigger)
                || self.activation_key.get().is_some()
            {
                resolved.primary_active
            } else if self.close_hovered.get() {
                resolved.primary_hover
            } else {
                resolved.primary
            };
            let frame_x = if frame.x.is_finite() { frame.x } else { 0.0 };
            let frame_y = if frame.y.is_finite() { frame.y } else { 0.0 };
            let trigger = Rect::new(
                frame_x + local_trigger.x,
                frame_y + local_trigger.y,
                local_trigger.w,
                local_trigger.h,
            );
            if trigger.w <= 0.0 || trigger.h <= 0.0 {
                return;
            }
            ctx.push_clip(trigger);
            ctx.fill_rect(trigger, primary, Some(Radius::uniform(resolved.radius)));
            ctx.text_center(
                visual.chrome.trigger_label,
                trigger,
                resolved.white,
                resolved.font_size,
            );
            ctx.pop_clip();
            return;
        }

        // 打开态按真实可见分支一次解析主题，避免同一帧重复虚调用。
        let resolved = visual.resolve_panel(
            ctx.tokens(),
            self.mask,
            self.closable,
            !self.extra.is_empty(),
            self.footer_visible,
        );
        // 保留主题遮罩色的基础 alpha，并按当前进出场进度衰减。
        let mask = super::fade_token_color(
            // 使用本帧已经解析的遮罩角色。
            resolved.mask,
            // 使用 Drawer 生命周期拥有的过渡不透明度。
            self.transition_opacity(),
        );
        let surface_extent = self.mask.then(|| {
            let surface_size = ctx.surface_size();
            (surface_size.w, surface_size.h)
        });
        if let Some((surface_w, surface_h)) = surface_extent {
            self.last_surface_w.set(surface_w);
            self.last_surface_h.set(surface_h);
            ctx.fill_rect(
                Rect::new(0.0, 0.0, surface_w, surface_h),
                // 绘制已经解析主题与动画的遮罩色。
                mask,
                None,
            );
        }

        let bg = resolved.background;
        let border = resolved.border;
        let text = resolved.text;
        let text_sec = resolved.text_secondary;
        let r = Radius::uniform(resolved.radius);
        let layout = &visual.layout;

        let drawer_rect = if let Some((surface_w, surface_h)) = surface_extent {
            self.overlay_rect_for_surface(surface_w, surface_h)
        } else {
            match self.placement {
                DrawerPlacement::Right | DrawerPlacement::Left => {
                    Rect::new(frame.x, frame.y, self.width, frame.h)
                }
                DrawerPlacement::Top | DrawerPlacement::Bottom => {
                    Rect::new(frame.x, frame.y, frame.w, self.height)
                }
            }
        };
        let drawer_rect = self.apply_transition_to_rect(drawer_rect);
        self.last_panel_rect.set(drawer_rect);
        let drawer_x = drawer_rect.x;
        let drawer_y = drawer_rect.y;
        let drawer_w = drawer_rect.w;
        let drawer_h = drawer_rect.h;
        let corner = match self.placement {
            DrawerPlacement::Right => Some(Radius { tl: r.tl, tr: 0.0, br: 0.0, bl: r.br }),
            DrawerPlacement::Left => Some(Radius { tl: 0.0, tr: r.tr, br: r.bl, bl: 0.0 }),
            DrawerPlacement::Top => Some(Radius { tl: 0.0, tr: 0.0, br: r.bl, bl: r.br }),
            DrawerPlacement::Bottom => Some(Radius { tl: r.tl, tr: r.tr, br: 0.0, bl: 0.0 }),
        };
        ctx.push_clip(drawer_rect);
        ctx.fill_rect(drawer_rect, bg, corner);
        ctx.stroke_rect(drawer_rect, border, visual.chrome.panel_stroke, corner);

        let header_rect = Rect::new(
            drawer_x,
            drawer_y,
            drawer_w,
            drawer_h.min(layout.header_height),
        );
        let close_w = if self.closable {
            drawer_w.min(layout.close_width)
        } else {
            0.0
        };
        let extra_w = if self.extra.is_empty() {
            0.0
        } else {
            (drawer_w - close_w).clamp(0.0, layout.extra_max_width)
        };
        let title_x = drawer_x + layout.title_inset.min(drawer_w);
        let title_rect = Rect::new(
            title_x,
            drawer_y,
            (drawer_x + drawer_w - close_w - extra_w - title_x).max(0.0),
            header_rect.h,
        );
        Self::paint_elided_text(ctx, &self.title, title_rect, text, resolved.title_font_size);

        if !self.extra.is_empty() {
            let extra_rect = Rect::new(
                drawer_x + drawer_w - close_w - extra_w,
                drawer_y,
                extra_w,
                header_rect.h,
            );
            Self::paint_elided_text(ctx, &self.extra, extra_rect, text_sec, resolved.extra_font_size);
        }
        if self.closable {
            let close_rect = Rect::new(
                drawer_x + drawer_w - close_w,
                drawer_y,
                close_w,
                header_rect.h,
            );
            let inset_x = layout.close_inset.min(close_rect.w * 0.5);
            let inset_y = layout.close_inset.min(close_rect.h * 0.5);
            let close_button = Rect::new(
                close_rect.x + inset_x,
                close_rect.y + inset_y,
                (close_rect.w - inset_x * 2.0).max(0.0),
                (close_rect.h - inset_y * 2.0).max(0.0),
            );
            if self.pressed_target.get() == Some(DrawerPointerTarget::Close) {
                ctx.fill_rect(
                    close_button,
                    resolved.fill_secondary,
                    Some(Radius::uniform(visual.chrome.control_radius)),
                );
            } else if self.close_hovered.get() {
                ctx.fill_rect(
                    close_button,
                    resolved.fill_tertiary,
                    Some(Radius::uniform(visual.chrome.control_radius)),
                );
            }
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                visual.icons.close,
                close_button,
                text_sec,
                visual.typography.close_icon,
            );
        }
        ctx.fill_rect(
            Rect::new(
                drawer_x,
                drawer_y + header_rect.h,
                drawer_w,
                layout.divider_thickness,
            ),
            border,
            None,
        );

        let footer_h = if self.footer_visible {
            (drawer_h - header_rect.h).clamp(0.0, layout.footer_height)
        } else {
            0.0
        };
        if self.footer_visible {
            let footer_y = drawer_y + drawer_h - footer_h;
            ctx.fill_rect(
                Rect::new(
                    drawer_x,
                    footer_y,
                    drawer_w,
                    layout.divider_thickness,
                ),
                border,
                None,
            );
            let btn_r = Some(Radius::uniform(visual.chrome.control_radius));
            let ok_w = (drawer_w - layout.footer_side_inset * 2.0)
                .clamp(0.0, layout.footer_button_max_width);
            let ok_h = (footer_h - layout.footer_vertical_padding)
                .clamp(0.0, layout.footer_button_max_height);
            if ok_w > 0.0 && ok_h > 0.0 {
                let ok_rect = Rect::new(
                    drawer_x + drawer_w - layout.footer_side_inset.min(drawer_w) - ok_w,
                    footer_y + (footer_h - ok_h) * 0.5,
                    ok_w,
                    ok_h,
                );
                ctx.fill_rect(ok_rect, resolved.primary, btn_r);
                ctx.text_center(
                    loc.drawer_ok,
                    ok_rect,
                    resolved.white,
                    resolved.footer_font_size,
                );
            }
        }
        ctx.pop_clip();
    }

    overlay_entry => (&self, id: crate::ui::WidgetId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        if !self.is_present() {
            return None;
        }

        let bounds = if self.mask {
            Rect::new(
                self.visual.layout.overlay_fallback_origin,
                self.visual.layout.overlay_fallback_origin,
                self.visual.layout.overlay_fallback_extent,
                self.visual.layout.overlay_fallback_extent,
            )
        } else {
            match self.placement {
                DrawerPlacement::Right | DrawerPlacement::Left => {
                    self.apply_transition_to_rect(Rect::new(frame.x, frame.y, self.width, frame.h))
                }
                DrawerPlacement::Top | DrawerPlacement::Bottom => {
                    self.apply_transition_to_rect(Rect::new(frame.x, frame.y, frame.w, self.height))
                }
            }
        };

        Some(
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Drawer)
                .bounds(bounds)
                .z_index(self.visual.chrome.overlay_z),
        )
    }

    // 使用真实表面解析 masked Drawer，非 mask Drawer 则保持面板命中区域。
    overlay_entry_for_surface => (&self, id: crate::ui::WidgetId, frame: Rect, surface: Rect) -> Option<crate::ui::OverlayEntry> {
        // 不在呈现生命周期时不登记 overlay。
        if !self.is_present() {
            // 关闭态只保留普通触发器。
            return None;
        }
        // mask Drawer 使用完整逻辑表面；无 mask 时使用当前动画后的面板区域。
        let bounds = if self.mask {
            // mask、命中与默认 blur 区域保持同一几何事实。
            surface
        } else {
            // 非 mask Drawer 仅覆盖其真实面板。
            match self.placement {
                // 横向面板按当前动画位置解析。
                DrawerPlacement::Right | DrawerPlacement::Left => {
                    // 使用布局高度与声明宽度。
                    self.apply_transition_to_rect(Rect::new(frame.x, frame.y, self.width, frame.h))
                }
                // 纵向面板按当前动画位置解析。
                DrawerPlacement::Top | DrawerPlacement::Bottom => {
                    // 使用布局宽度与声明高度。
                    self.apply_transition_to_rect(Rect::new(frame.x, frame.y, frame.w, self.height))
                }
            }
        };
        // 创建统一 overlay 登记。
        let mut entry = crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Drawer)
            // 同一 bounds 同时服务命中、mask 与默认 blur。
            .bounds(bounds)
            // 使用 UIX 声明的 Drawer 层级。
            .z_index(self.visual.chrome.overlay_z);
        // 离场开始立即停止 backdrop blur。
        if !self.closing {
            // 默认关闭，仅投影显式请求。
            if let Some(blur) = self.backdrop_blur {
                // 复用统一 OverlayEntry 契约。
                entry = entry.backdrop_blur(blur);
            }
        }
        // 返回当前表面下的完整登记。
        Some(entry)
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], tree: &WidgetTree)
        -> Vec<(crate::ui::WidgetId, Rect)>
    {
        if !self.is_present() {
            return Vec::new();
        }
        let frame = Self::normalize_frame(frame);
        self.last_frame.set(frame);
        let drawer_rect = if self.mask {
            let (surface_w, surface_h) = tree
                .root_id()
                .and_then(|root_id| tree.get(root_id))
                .map(|root| (root.frame().w, root.frame().h))
                .filter(|(w, h)| *w > 0.0 && *h > 0.0)
                .map(|(w, h)| (Self::normalize_dimension(w), Self::normalize_dimension(h)))
                .unwrap_or((
                    self.visual.layout.surface_fallback_width,
                    self.visual.layout.surface_fallback_height,
                ));
            self.last_surface_w.set(surface_w);
            self.last_surface_h.set(surface_h);
            self.overlay_rect_for_surface(surface_w, surface_h)
        } else {
            match self.placement {
                DrawerPlacement::Right | DrawerPlacement::Left => {
                    Rect::new(frame.x, frame.y, self.width, frame.h)
                }
                DrawerPlacement::Top | DrawerPlacement::Bottom => {
                    Rect::new(frame.x, frame.y, frame.w, self.height)
                }
            }
        };
        self.last_panel_rect.set(drawer_rect);
        if children.is_empty() {
            return Vec::new();
        }
        let drawer_rect = self.apply_transition_to_rect(drawer_rect);
        self.last_panel_rect.set(drawer_rect);
        let drawer_x = drawer_rect.x;
        let drawer_y = drawer_rect.y;
        let drawer_w = drawer_rect.w;
        let drawer_h = drawer_rect.h;
        let header_h = drawer_h.min(self.visual.layout.header_height);
        let footer_h = if self.footer_visible {
            (drawer_h - header_h).clamp(0.0, self.visual.layout.footer_height)
        } else {
            0.0
        };
        let body_y = drawer_y + header_h;
        let body_h = (drawer_h - header_h - footer_h).max(0.0);
        let pad = self.visual.layout.body_padding;
        children
            .iter()
            .map(|child| {
                (
                    child.id,
                    Rect::new(
                        drawer_x + pad,
                        body_y + pad,
                        (drawer_w - pad * 2.0).max(0.0),
                        (body_h - pad * 2.0).max(0.0),
                    ),
                )
            })
            .collect()
    }

    children_clip => (&self, _frame: Rect) -> Option<Rect> {
        if self.is_present() {
            Some(self.body_rect(self.last_panel_rect.get()))
        } else {
            Some(Rect::zero())
        }
    }

    take_layout_request => (&mut self) -> bool {
        self.layout_requested.replace(false)
    }

    update_animation => (&mut self, dt: f64) -> bool {
        // 每帧先把外部受控事实同步到 Drawer 的唯一运行生命周期。
        self.controlled_sync();
        if !self.is_present() || self.transition.finished {
            self.transition_dirty = false;
            return false;
        }

        self.transition.update(dt);
        self.transition_dirty = true;

        if self.closing && self.transition.finished {
            self.visible = false;
            self.closing = false;
            self.layout_requested.set(true);
        }

        self.is_present() && !self.transition.finished
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.transition_dirty {
            if self.mask {
                let surface_w = self.last_surface_w.get();
                let surface_h = self.last_surface_h.get();
                if surface_w > 0.0 && surface_h > 0.0 {
                    return Rect::new(0.0, 0.0, surface_w, surface_h);
                }
            }
            frame
        } else {
            Rect::zero()
        }
    }
}

// 把 Drawer 行为内核与 UIX 唯一视觉静态项融合为单一节点。
fn build_drawer_view(mut kernel: Drawer, visual: &'static DrawerVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

// 让声明式 View 构建统一进入同目录 UIX 根。
fn build_drawer_uix_root(kernel: Drawer) -> ViewNode {
    crate::uix!("src/ui/widgets/feedback/drawer/drawer.uix")
}

impl View for Drawer {
    fn build(self) -> ViewNode {
        build_drawer_uix_root(self)
    }
}
