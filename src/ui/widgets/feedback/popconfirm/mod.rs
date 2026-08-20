// 组合触发器需要一次性 ViewNode 所有权与同帧尺寸事实。
use std::cell::{Cell, RefCell};
// 同步窄回调与一次性 ViewNode 句柄共享组件生命周期。
use std::rc::Rc;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::{Color, FillRule, PathBuilder, Radius};
use crate::ui::animation::{TransitionPlayer, presets};
use crate::ui::widget_runtime::paint_context::PaintContext;
// 复用组件树唯一的子节点测量入口。
use crate::ui::SnapshotFields;
use crate::ui::component_snapshot::SnapshotPopconfirm;
use crate::ui::widget_runtime::tree_measure::child_from_tree_with_constraints;
use crate::ui::{
    ComponentId, EventResult, KeyCode, LayoutChild, MouseButton, SemanticEvent, SystemEvent,
    WidgetTree,
};

mod geometry;

// 仅在测试构建中编译组合 trigger 的父级测量回归。
#[cfg(test)]
// 将布局回归从已接近规模门禁的几何测试文件中独立出来。
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/feedback/popconfirm/layout_tests.rs"]
mod layout_tests;

use self::geometry::*;

const POPCONFIRM_WIDTH: f32 = 200.0;
const POPCONFIRM_HEIGHT: f32 = 110.0;
// 零子节点 Rust 构造保留兼容触发器宽度。
const FALLBACK_TRIGGER_WIDTH: f32 = 80.0;
// 零子节点 Rust 构造保留兼容触发器高度。
const FALLBACK_TRIGGER_HEIGHT: f32 = 28.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PopconfirmTarget {
    Trigger,
    Confirm,
    Cancel,
}

/// Popconfirm 弹出位置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PopconfirmPlacement {
    /// 将确认气泡放在触发器上方，使用默认水平对齐。
    Top,
    /// 将确认气泡放在触发器上方并与其左边缘对齐。
    TopLeft,
    /// 将确认气泡放在触发器上方并与其右边缘对齐。
    TopRight,
    /// 将确认气泡放在触发器下方，使用默认水平对齐。
    Bottom,
    /// 将确认气泡放在触发器下方并与其左边缘对齐。
    BottomLeft,
    /// 将确认气泡放在触发器下方并与其右边缘对齐。
    BottomRight,
}

component! {
    /// 拥有唯一触发器、确认气泡和一次性确认或取消动作的组合组件。
    pub struct Popconfirm {
        title: String,
        confirm_text: String,
        cancel_text: String,
        visible: bool,
        placement: PopconfirmPlacement,
        arrow: bool,
        icon: bool,
        /// UIX 组合模式由唯一直接子 View 绘制并拥有业务交互。
        custom_trigger: bool,
        // Popconfirm 在声明期拥有完整 trigger ViewNode 子树。
        #[snapshot(skip)]
        custom_trigger_view: Option<Rc<RefCell<Option<crate::ui::view::ViewNode>>>>,
        /// 保存组件树实际登记的直接 trigger 数量。
        trigger_child_count: usize,
        /// 保存本轮布局测得的实际 trigger border-box 尺寸。
        trigger_size: Cell<Size>,
        // 保存确认动作的同步窄回调。
        #[snapshot(skip)]
        confirm_callback: Option<Rc<dyn Fn()>>,
        // 保存所有用户取消入口共用的同步窄回调。
        #[snapshot(skip)]
        cancel_callback: Option<Rc<dyn Fn()>>,
        /// 防止同一稳定打开周期重复提交用户动作。
        action_committed: bool,
        transition: TransitionPlayer,
        closing: bool,
        transition_dirty: bool,
        focused: bool,
        focused_action: usize,
        pending_submit: Cell<bool>,
        hovered_target: Option<PopconfirmTarget>,
        pressed_target: Option<PopconfirmTarget>,
        pressed_key: Option<KeyCode>,
        last_frame: Cell<Rect>,
        popup_rect: Cell<Rect>,
        surface_rect: Cell<Rect>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        // 组合模式由实际子树在同一布局收敛周期撑开包装节点。
        let desired = if self.custom_trigger { Size::zero() } else { self.intrinsic_size() };
        // 尊重父级给出的最小与最大约束。
        constraints.clamp(desired)
    }

    // 组合模式在父级测量本轮直接读取唯一 trigger 的自然尺寸。
    measure_from_children => (&self, constraints: Constraints, children: &[ComponentId], tree: &WidgetTree)
        -> Option<Size>
    {
        // 兼容自绘模式继续使用普通 intrinsic measure。
        if !self.custom_trigger {
            // 空值让统一入口回退到 measure。
            return None;
        }
        // UIX 契约只允许一个直接 trigger，运行时异常结构不伪造尺寸。
        let [child_id] = children else {
            // 让异常结构继续走零尺寸并由既有门禁诊断。
            return Some(Size::zero());
        };
        // 使用与 arrange 相同的无约束自然测量，避免零高 bootstrap 截断按钮。
        let child = child_from_tree_with_constraints(*child_id, tree, Constraints::unconstrained());
        // 将自然 border-box 收敛到父级给出的有限边界。
        let measured = constraints.clamp(child.measured_size);
        // 记录本轮真实锚点尺寸，供同帧命中、绘制与浮层定位共享。
        self.trigger_size.set(measured);
        // 把唯一 trigger 的同轮尺寸返回给父容器。
        Some(measured)
    }

    // 组合模式必须让完整 trigger 子树成为真实指针目标。
    hit_test_children => (&self) -> bool { self.custom_trigger && self.trigger_child_count == 1 }

    // 记录唯一 trigger 子树的挂载与卸载变化。
    on_children_changed => (&mut self, child_count: usize) {
        // 组件树通知成为 trigger 生命周期的当前事实。
        self.trigger_child_count = child_count;
        // 子树变化后清除旧布局尺寸，禁止跨身份猜测。
        self.trigger_size.set(Size::zero());
    }

    // 将声明期拥有的 trigger ViewNode 一次性交给组件树物化。
    build_view_children => (&self) -> Vec<crate::ui::view::ViewNode> {
        // 取走待物化子树，避免同一声明重复挂载。
        self.custom_trigger_view
            // 组合模式才持有一次性建造句柄。
            .as_ref()
            // 从共享单元取走完整 ViewNode。
            .and_then(|view| view.borrow_mut().take())
            // 零或一个 trigger 统一转成迭代器。
            .into_iter()
            // 返回组件树可消费的直接子节点集合。
            .collect()
    }

    // 使用唯一 trigger 子节点的自然尺寸完成本轮测量。
    measure_children => (&self, _frame: Rect, children: &[ComponentId], tree: &WidgetTree)
        -> Vec<LayoutChild>
    {
        // UIX 门禁保证唯一直接子树；运行时只消费第一项以保持确定性。
        children
            // 借用唯一直接子节点身份。
            .first()
            // 使用无约束测量取得 trigger 自然 border-box。
            .map(|id| child_from_tree_with_constraints(*id, tree, Constraints::unconstrained()))
            // 把可选测量结果物化为布局集合。
            .into_iter()
            // 返回零或一个 trigger 布局快照。
            .collect()
    }

    // 以本轮自然尺寸安排唯一 trigger，并记录同帧锚点事实。
    layout_children => (&self, frame: Rect, children: &[LayoutChild], _tree: &WidgetTree)
        -> Vec<(ComponentId, Rect)>
    {
        // 没有组合 trigger 时清除尺寸并保持零子布局。
        let Some(child) = children.first() else {
            // 禁止沿用上一帧已经卸载的 trigger 尺寸。
            self.trigger_size.set(Size::zero());
            // 返回空布局集合。
            return Vec::new();
        };
        // 归一化本轮测量出的自然尺寸。
        let size = Size::new(
            // 宽度不得为负或非有限值。
            child.measured_size.w.max(0.0),
            // 高度不得为负或非有限值。
            child.measured_size.h.max(0.0),
        );
        // 保存当前 arrange 周期的实际 trigger border-box。
        self.trigger_size.set(size);
        // trigger 与包装节点共享原点并保持自己的自然尺寸。
        vec![(child.id, Rect::new(frame.x, frame.y, size.w, size.h))]
    }

    // 组合模式把 Tab 焦点交给 trigger 子树；缺失子树时保留恢复入口。
    tab_index => (&self) -> i32 { i32::from(!self.custom_trigger || self.trigger_child_count == 0) }

    // 组合模式需要在子 trigger 前观察同一输入序列而不吞掉业务事件。
    wants_capture_phase => (&self) -> bool { self.custom_trigger && self.trigger_child_count == 1 }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(target) = self.target_at(*pos) {
                    self.focused = true;
                    self.pressed_target = Some(target);
                    self.focused_action = match target {
                        PopconfirmTarget::Cancel => 1,
                        _ => 0,
                    };
                    // trigger 捕获阶段只记录手势，继续交付给真实子 View。
                    return if self.custom_trigger && target == PopconfirmTarget::Trigger {
                        EventResult::NotHandled
                    } else {
                        EventResult::Handled
                    };
                }
                if self.is_present() && self.popup_rect.get().contains(*pos) {
                    return EventResult::Handled;
                }
                if self.is_present() {
                    self.cancel_pending_activation();
                    // 任何落入 owner 但不命中气泡的用户点击都属于取消。
                    self.cancel_action();
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerUp {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let pressed = self.pressed_target.take();
                if let Some(pressed) = pressed.filter(|pressed| Some(*pressed) == self.target_at(*pos)) {
                    match pressed {
                        PopconfirmTarget::Trigger => {
                            if self.is_present() && !self.closing {
                                // 打开态重新激活 trigger 属于一次用户取消。
                                self.cancel_action();
                            } else {
                                self.open();
                            }
                        }
                        PopconfirmTarget::Confirm => self.confirm_action(),
                        PopconfirmTarget::Cancel => self.cancel_action(),
                    }
                }
                // trigger 捕获阶段完成 owner 状态切换后仍让子 View 结束原手势。
                if self.custom_trigger && pressed == Some(PopconfirmTarget::Trigger) {
                    EventResult::NotHandled
                } else {
                    EventResult::Handled
                }
            }
            SystemEvent::PointerMove { pos, .. } => {
                let hovered = self.target_at(*pos);
                if self.hovered_target != hovered {
                    self.hovered_target = hovered;
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerLeave => {
                let changed = self.hovered_target.is_some() || self.pressed_target.is_some();
                self.hovered_target = None;
                self.pressed_target = None;
                if changed {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.cancel_pending_activation();
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } if self.visible => match key {
                KeyCode::Left | KeyCode::Home => {
                    self.focused_action = 0;
                    EventResult::Handled
                }
                KeyCode::Right | KeyCode::End => {
                    self.focused_action = 1;
                    EventResult::Handled
                }
                KeyCode::Enter | KeyCode::Space => {
                    self.pressed_key = Some(*key);
                    EventResult::Handled
                }
                KeyCode::Escape => {
                    // Escape 统一进入用户取消动作。
                    self.cancel_action();
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },
            SystemEvent::KeyDown {
                key: key @ (KeyCode::Enter | KeyCode::Space),
                ..
            } => {
                self.pressed_key = Some(*key);
                // 关闭态组合 trigger 继续接收自己的键盘激活。
                if self.custom_trigger {
                    EventResult::NotHandled
                } else {
                    EventResult::Handled
                }
            }
            SystemEvent::KeyUp {
                key: key @ (KeyCode::Enter | KeyCode::Space),
                ..
            } => {
                // 记录动作前状态，避免 open() 后误判为应吞掉 trigger 的 KeyUp。
                let was_visible = self.visible;
                let matches = self.pressed_key.take() == Some(*key);
                if matches {
                    if self.visible {
                        if self.focused_action == 0 {
                            self.confirm_action();
                        } else {
                            self.cancel_action();
                        }
                    } else {
                        self.open();
                    }
                }
                // 关闭态组合 trigger 继续合成自己的唯一 Click。
                if self.custom_trigger && !was_visible {
                    EventResult::NotHandled
                } else {
                    EventResult::Handled
                }
            }
            _ => EventResult::NotHandled,
        }
    }

    on_focus_within => (&mut self, focused: bool) -> EventResult {
        self.focused = focused;
        if !focused {
            self.cancel_pending_activation();
        }
        if !focused && self.visible {
            // 用户焦点离开整个组合范围属于一次取消。
            self.cancel_action();
        }
        EventResult::Handled
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_submit
            .replace(false)
            .then(|| SemanticEvent::submit(id, "confirm"))
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        self.dirty_rect_for_frame(frame)
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let frame = Self::normalize_frame(frame);
        let surface_size = ctx.logical_surface_size();
        let surface = Self::normalize_frame(Rect::new(0.0, 0.0, surface_size.w, surface_size.h));
        self.last_frame.set(frame);
        self.surface_rect.set(surface);
        // 组合模式以本轮实际 trigger border-box 作为唯一锚点。
        let trigger_frame = self.absolute_trigger_frame(frame);
        let popup_geometry = resolve_popconfirm_geometry(
            trigger_frame,
            surface,
            self.placement,
            self.arrow,
            POPCONFIRM_WIDTH,
            POPCONFIRM_HEIGHT,
        );
        self.popup_rect.set(Rect::new(
            popup_geometry.popup.x - frame.x,
            popup_geometry.popup.y - frame.y,
            popup_geometry.popup.w,
            popup_geometry.popup.h,
        ));

        let loc = crate::ui::widget_runtime::locale::use_locale();
        let bg = ctx.tokens().color_bg_elevated();
        let border = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let primary = ctx.tokens().color_primary();
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));

        ctx.push_clip(surface);
        if !self.custom_trigger && (self.hovered_target == Some(PopconfirmTarget::Trigger)
            || self.pressed_target == Some(PopconfirmTarget::Trigger)
        ) {
            ctx.fill_rect(
                frame,
                if self.pressed_target == Some(PopconfirmTarget::Trigger) {
                    ctx.tokens().color_fill_secondary()
                } else {
                    ctx.tokens().color_fill_tertiary()
                },
                r,
            );
        }
        if !self.custom_trigger {
            // 兼容零子节点 Rust 构造继续绘制旧删除触发器。
            Self::paint_elided_text(
                ctx,
                loc.delete_text,
                frame,
                ctx.tokens().color_error(),
                13.0,
                true,
            );
        }
        if !self.custom_trigger && self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(frame, primary, 2.0, r);
        }

        if self.is_present() && popup_geometry.popup.w > 0.0 && popup_geometry.popup.h > 0.0 {
            let opacity = self.transition.opacity_progress.clamp(0.0, 1.0);
            let popup_bg = fade_color(bg, opacity);
            let popup_border = fade_color(border, opacity);
            let popup_text = fade_color(text_color, opacity);
            let popup_primary = fade_color(primary, opacity);
            let popup_warning = fade_color(ctx.tokens().color_warning(), opacity);
            let pop_rect = popup_geometry.popup;
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
                draw_popconfirm_arrow(
                    ctx,
                    trigger_frame,
                    pop_rect,
                    popup_geometry.placement,
                    popup_bg,
                );
            }

            let loc = crate::ui::widget_runtime::locale::use_locale();
            let title = if self.title.is_empty() {
                loc.popconfirm_title
            } else {
                &self.title
            };
            let inset = 12.0_f32.min(pop_rect.w * 0.5);
            let (confirm_rect, cancel_rect) = button_rects_for_popup(pop_rect);
            let title_bottom = if confirm_rect.h > 0.0 {
                (confirm_rect.y - 6.0).max(pop_rect.y)
            } else {
                pop_rect.y + pop_rect.h
            };
            let icon_width = if self.icon && pop_rect.w >= 48.0 {
                20.0
            } else {
                0.0
            };
            if icon_width > 0.0 {
                crate::ui::widgets::icon::Icon::paint_in_frame(
                    ctx,
                    "alert-triangle",
                    Rect::new(
                        pop_rect.x + inset,
                        pop_rect.y + 8.0,
                        icon_width,
                        (title_bottom - pop_rect.y - 8.0).max(0.0),
                    ),
                    popup_warning,
                    16.0,
                );
            }
            let title_rect = Rect::new(
                pop_rect.x + inset + icon_width,
                pop_rect.y + 6.0,
                (pop_rect.w - inset * 2.0 - icon_width).max(0.0),
                (title_bottom - pop_rect.y - 6.0).max(0.0),
            );
            Self::paint_elided_text(ctx, title, title_rect, popup_text, 13.0, false);

            let btn_r = Some(Radius::uniform(4.0));
            if self.hovered_target == Some(PopconfirmTarget::Confirm)
                || self.pressed_target == Some(PopconfirmTarget::Confirm)
            {
                ctx.fill_rect(
                    confirm_rect,
                    if self.pressed_target == Some(PopconfirmTarget::Confirm) {
                        fade_color(ctx.tokens().color_primary_active(), opacity)
                    } else {
                        fade_color(ctx.tokens().color_primary_hover(), opacity)
                    },
                    btn_r,
                );
            } else {
                ctx.fill_rect(confirm_rect, popup_primary, btn_r);
            }
            let confirm = if self.confirm_text.is_empty() {
                loc.popconfirm_ok
            } else {
                &self.confirm_text
            };
            Self::paint_elided_text(
                ctx,
                confirm,
                confirm_rect,
                // 确认按钮文字：白色 token 随透明度淡入淡出。
                fade_color(ctx.tokens().color_white(), opacity),
                12.0,
                true,
            );

            if self.hovered_target == Some(PopconfirmTarget::Cancel)
                || self.pressed_target == Some(PopconfirmTarget::Cancel)
            {
                ctx.fill_rect(
                    cancel_rect,
                    if self.pressed_target == Some(PopconfirmTarget::Cancel) {
                        fade_color(ctx.tokens().color_fill_secondary(), opacity)
                    } else {
                        fade_color(ctx.tokens().color_fill_tertiary(), opacity)
                    },
                    btn_r,
                );
            }
            ctx.stroke_rect(cancel_rect, popup_border, 1.0, btn_r);
            let cancel = if self.cancel_text.is_empty() {
                loc.popconfirm_cancel
            } else {
                &self.cancel_text
            };
            Self::paint_elided_text(ctx, cancel, cancel_rect, popup_text, ctx.tokens().font_size_sm(), true);
            if self.focused && tree.keyboard_focus_visible() && self.visible {
                let (focus_rect, focus_color) = if self.focused_action == 0 {
                    (confirm_rect, fade_color(ctx.tokens().color_white(), opacity))
                } else {
                    (cancel_rect, popup_primary)
                };
                ctx.stroke_rect(focus_rect, focus_color, 2.0, btn_r);
            }
        }
        ctx.pop_clip();
    }

    overlay_entry => (&self, id: crate::ui::ComponentId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        if !self.is_present() {
            return None;
        }

        let frame = Self::normalize_frame(frame);
        // 浮层命中范围同时保留气泡与真实 trigger，允许打开态 trigger 继续收到 Click。
        let popup = expand_popconfirm_rect(self.absolute_popup_rect(frame), 12.0)
            // 合并真实 trigger border-box。
            .union(&self.absolute_trigger_frame(frame))
            .intersect(&self.surface_or_fallback(frame))
            .unwrap_or_default();
        Some(
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Popover)
                .bounds(popup)
                .z_index(950)
                // 外部点击由树的 System 私有取消端口回调 owner。
                .dismiss_on_outside(true),
        )
    }


    // 布局阶段以当前逻辑表面刷新确认气泡几何，再沿用既有登记策略。
    overlay_entry_for_surface => (&self, id: crate::ui::ComponentId, frame: Rect, surface: Rect) -> Option<crate::ui::OverlayEntry> {
        // 记录与本次 OverlayStack 重建一致的表面边界。
        self.surface_rect.set(Self::normalize_frame(surface));
        // 复用统一的浮层登记逻辑。
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
            self.dirty_rect_for_frame(frame)
        } else {
            Rect::zero()
        }
    }
}

impl Default for Popconfirm {
    fn default() -> Self {
        Self::new()
    }
}
