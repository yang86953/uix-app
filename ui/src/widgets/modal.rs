use std::cell::Cell;

use crate::define_widget;
use crate::animation::transition::{presets, TransitionPlayer};
use uix_platform::{ControlSize, Point, Rect, Size};
use uix_graphics::{Color, Radius};
use crate::render_context::RenderContext;
use crate::widget::{EventResult, WidgetCore, WidgetEvent, WidgetTree};

// Modal — 模态对话框。
//
// 两种模式：
// - 普通模式（默认）：preferred_size 返回实际尺寸，参与父容器 flex 布局。
// - 覆盖层模式（`.overlay(true)`）：preferred_size 始终返回 (0,0)，不参与 flex 布局，
//   render 时通过 `ctx.canvas_2d().width()/height()` 获取窗口尺寸居中弹窗。
//   适合放在 Column/Row 末尾作全屏覆盖层，不挤压主内容。
define_widget! {
    pub struct Modal {
        title: String,
        visible: bool,
        width: f32,
        height: f32,
        modal_size: ControlSize,
        closable: bool,
        mask_closable: bool,
        footer_visible: bool,
        centered: bool,
        on_ok: Option<Box<dyn FnMut() + 'static>>,
        on_cancel: Option<Box<dyn FnMut() + 'static>>,
        transition_player: Option<TransitionPlayer>,
        /// 上次 visible 值，用于检测变化触发过渡动画。
        prev_visible: bool,
        /// 覆盖层模式：preferred_size 始终返回 (0,0)，render 使用窗口尺寸居中。
        overlay: bool,
        /// 覆盖层模式下缓存的窗口尺寸（render 时用 Cell 更新，layout_children/on_event 时读取）
        last_win_w: Cell<f32>,
        last_win_h: Cell<f32>,
        /// 退场动画进行中（visible 保持 true，动画结束后自动设 false）。
        closing: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        if self.overlay {
            // 覆盖层模式：不参与父容器 flex 布局
            Size::zero()
        } else if self.visible || self.closing {
            // 退场动画期间保持布局空间
            Size::new(self.width, self.height)
        } else {
            Size::zero()
        }
    }

    visible => (&self) -> bool { self.visible }

    hit_test_frame => (&self, _actual_frame: Rect) -> Rect {
        if self.overlay {
            // 覆盖层模式：使用缓存的窗口尺寸作为命中区域，
            // 让 (0,0,0,0) 的 layout frame 也能被 hit-test 命中。
            Rect::new(0.0, 0.0, self.last_win_w.get(), self.last_win_h.get())
        } else {
            _actual_frame
        }
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if !self.visible || self.closing { return EventResult::NotHandled; }
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                if self.overlay {
                    // 覆盖层模式：计算弹窗在全屏中的实际矩形
                    let dw = self.width;
                    let dh = self.height;
                    let dlg_x = (self.last_win_w.get() - dw) / 2.0;
                    let dlg_y = (self.last_win_h.get() - dh) / 2.0;
                    let dlg_rect = Rect::new(dlg_x, dlg_y, dw, dh);

                    let close_rect = Rect::new(dlg_x + dw - 48.0, dlg_y, 48.0, 48.0);
                    if self.closable && close_rect.contains(*pos) {
                        self.close();
                        return EventResult::Handled;
                    }
                    if self.mask_closable && !dlg_rect.contains(*pos) {
                        self.close();
                        return EventResult::Handled;
                    }
                } else {
                    let close_rect = Rect::new(self.width - 48.0, 0.0, 48.0, 48.0);
                    if self.closable && close_rect.contains(*pos) {
                        self.close();
                        return EventResult::Handled;
                    }
                    if self.mask_closable && (pos.x < 0.0 || pos.y < 0.0) {
                        self.close();
                        return EventResult::Handled;
                    }
                }
                EventResult::Handled
            }
            WidgetEvent::KeyDown { key, .. } => {
                if *key == crate::widget::KeyCode::Escape && self.closable {
                    self.close();
                    return EventResult::Handled;
                }
                EventResult::Handled
            }
            _ => { if self.visible { EventResult::Handled } else { EventResult::NotHandled } }
        }
    }

    on_update => (&mut self, dt: f64) {
        if self.visible != self.prev_visible {
            self.prev_visible = self.visible;
            if self.visible {
                // 进场：创建进场动画
                self.transition_player = Some(TransitionPlayer::new(presets::modal_enter()));
                self.closing = false;
            } else {
                // 退场（外部直接 set_visible(false) 时触发）
                self.transition_player = Some(TransitionPlayer::new(presets::modal_exit()));
                self.closing = true;
            }
        }
        // 推进动画，用 map 避免借用冲突
        let finished = self.transition_player.as_mut()
            .map(|tp| { tp.update(dt); tp.finished })
            .unwrap_or(false);
        if finished {
            if self.closing {
                // 退场动画结束 → 真正隐藏
                self.visible = false;
                self.prev_visible = false;
                self.closing = false;
            }
            // 进场/退场动画结束 → 清除播放器
            self.transition_player = None;
        }
    }

    needs_continuous_update => (&self) -> bool {
        self.transition_player.as_ref().is_some_and(|tp| !tp.finished)
    }

    render => (&self, _frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        // 完全隐藏 → 不渲染
        if !self.visible && self.transition_player.is_none() { return; }
        let opacity = self.transition_player.as_ref().map_or(1.0, |tp| tp.opacity_progress);
        let scale = self.transition_player.as_ref().map_or(1.0, |tp| tp.scale);

        let border_radius_lg = ctx.tokens().border_radius_lg();
        let bg_container = ctx.tokens().color_bg_container();
        let border_secondary = ctx.tokens().color_border_secondary();
        let text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();

        // 确定弹窗位置：覆盖层模式使用窗口尺寸居中，否则使用 frame 中心
        let (cx, cy, sw, sh) = if self.overlay {
            let win_w = ctx.canvas_2d().width() as f32;
            let win_h = ctx.canvas_2d().height() as f32;
            // 缓存窗口尺寸供 layout_children / on_event 使用
            self.last_win_w.set(win_w);
            self.last_win_h.set(win_h);
            (win_w / 2.0, win_h / 2.0, self.width * scale, self.height * scale)
        } else {
            (_frame.x + _frame.w / 2.0, _frame.y + _frame.h / 2.0, _frame.w * scale, _frame.h * scale)
        };

        ctx.canvas_2d().set_opacity(opacity);

        // 遮罩：从弹窗中心向四周扩展足够大覆盖全屏
        let mask_size = 2000.0;
        ctx.fill_rect(
            Rect::new(cx - mask_size, cy - mask_size, mask_size * 2.0, mask_size * 2.0),
            Color::from_rgba(0, 0, 0, 128), None,
        );

        let scaled_frame = Rect::new(cx - sw / 2.0, cy - sh / 2.0, sw, sh);

        let radius = Some(Radius::uniform(border_radius_lg));
        ctx.fill_rect(scaled_frame, bg_container, radius);
        ctx.stroke_rect(scaled_frame, border_secondary, 1.0, radius);

        let title_h = 56.0;
        let footer_h = if self.footer_visible { 56.0 } else { 0.0 };

        let title_rect = Rect::new(scaled_frame.x, scaled_frame.y, scaled_frame.w, title_h);
        let ty = ctx.visual_center_y(title_rect, 16.0);
        ctx.draw_text(&self.title, Point::new(scaled_frame.x + 24.0, ty), text_color, 16.0);
        ctx.fill_rect(Rect::new(scaled_frame.x, scaled_frame.y + title_h, scaled_frame.w, 1.0), border_secondary, None);

        if self.closable {
            ctx.draw_text("✕", Point::new(scaled_frame.x + scaled_frame.w - 36.0, ty), text_secondary, 16.0);
        }
        if self.footer_visible {
            ctx.fill_rect(Rect::new(scaled_frame.x, scaled_frame.y + scaled_frame.h - footer_h, scaled_frame.w, 1.0), border_secondary, None);
        }

        ctx.canvas_2d().set_opacity(1.0);
    }

    layout_children => (&self, _frame: Rect, children: &[crate::widget::WidgetId], tree: &WidgetTree)
        -> Vec<(crate::widget::WidgetId, Rect)>
    {
        if children.is_empty() {
            return Vec::new();
        }
        // 无论 visible 与否都返回正确的子节点 frame。
        // 渲染管线通过 BoxedWidget::visible() 跳过不可见节点及其子树，
        // 因此无需用 off-screen 坐标防止子节点渲染。
        // 确定弹窗内容区域
        let (dlg_x, dlg_y, dlg_w, dlg_h) = if self.overlay {
            // 覆盖层模式：从树根节点 frame 获取窗口尺寸（根 frame = 窗口尺寸）。
            // 事件循环的重布局检查保证根 frame 始终匹配实际窗口大小。
            let (win_w, win_h) = tree
                .root_id()
                .and_then(|rid| tree.get(rid))
                .map(|root| (root.frame().w, root.frame().h))
                .filter(|(w, h)| *w > 0.0 && *h > 0.0)
                .unwrap_or((1200.0, 760.0));
            // 同时更新缓存的窗口尺寸，供 on_event 中的命测试计算使用
            self.last_win_w.set(win_w);
            self.last_win_h.set(win_h);
            let cx = win_w / 2.0;
            let cy = win_h / 2.0;
            (cx - self.width / 2.0, cy - self.height / 2.0, self.width, self.height)
        } else {
            (_frame.x, _frame.y, _frame.w, _frame.h)
        };
        let title_h = 56.0;
        let footer_h = if self.footer_visible { 56.0 } else { 0.0 };
        let body_y = dlg_y + title_h;
        let body_h = dlg_h - title_h - footer_h;
        let padding = 24.0;
        children.iter().map(|&cid| (cid, Rect::new(dlg_x + padding, body_y + padding, dlg_w - padding * 2.0, body_h - padding * 2.0))).collect()
    }
}

impl Modal {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            visible: false, width: 520.0, height: 300.0,
            modal_size: ControlSize::Medium,
            closable: true, mask_closable: true, footer_visible: true,
            centered: true,
            on_ok: None, on_cancel: None,
            transition_player: None,
            prev_visible: false,
            overlay: false,
            last_win_w: Cell::new(0.0),
            last_win_h: Cell::new(0.0),
            closing: false,
        }
    }

    pub fn visible(mut self, v: bool) -> Self { self.visible = v; self.prev_visible = v; self }
    pub fn show(mut self) -> Self { self.visible = true; self.prev_visible = false; self }
    pub fn size(mut self, w: f32, h: f32) -> Self { self.width = w; self.height = h; self }
    pub fn modal_size(mut self, s: ControlSize) -> Self {
        self.modal_size = s;
        match s {
            ControlSize::Small => { self.width = 400.0; self.height = 200.0; }
            ControlSize::Medium => { self.width = 520.0; self.height = 300.0; }
            ControlSize::Large => { self.width = 720.0; self.height = 400.0; }
        }
        self
    }
    pub fn closable(mut self, v: bool) -> Self { self.closable = v; self }
    pub fn mask_closable(mut self, v: bool) -> Self { self.mask_closable = v; self }
    pub fn footer_visible(mut self, v: bool) -> Self { self.footer_visible = v; self }
    pub fn centered(mut self, v: bool) -> Self { self.centered = v; self }
    /// 设为覆盖层模式：preferred_size 返回 (0,0)，render 使用窗口尺寸居中。
    /// 适合放在 flex 容器末尾作全屏弹窗叠加层，不挤压主内容。
    pub fn overlay(mut self, v: bool) -> Self { self.overlay = v; self }
    pub fn on_ok<F: FnMut() + 'static>(mut self, f: F) -> Self { self.on_ok = Some(Box::new(f)); self }
    pub fn on_cancel<F: FnMut() + 'static>(mut self, f: F) -> Self { self.on_cancel = Some(Box::new(f)); self }
    pub fn is_visible(&self) -> bool { self.visible }
    pub fn set_visible(&mut self, v: bool) { self.prev_visible = self.visible; self.visible = v; }
    /// 设置可见性但不触发过渡动画（用于状态同步等场景，避免延迟一帧启动动画导致闪烁）
    pub fn set_visible_no_anim(&mut self, v: bool) {
        self.visible = v;
        self.prev_visible = v;
        self.closing = false;
        self.transition_player = None;
    }
    /// 打开弹窗（触发进场动画）。
    pub fn open(&mut self) {
        if self.closing {
            // 退场动画进行中 → 取消退场，直接显示
            self.closing = false;
            self.transition_player = None;
            self.visible = true;
            self.prev_visible = true;
        } else if !self.visible {
            // 完全隐藏 → 正常打开
            self.visible = true;
            // prev_visible 保持 false，on_update 检测到变化后创建进场 TP
        }
    }
    /// 关闭弹窗（触发退场动画，动画结束后自动隐藏）。
    pub fn close(&mut self) {
        if !self.visible || self.closing { return; }
        // 启动退场动画，保持 visible=true 直到动画结束
        self.transition_player = Some(TransitionPlayer::new(presets::modal_exit()));
        self.closing = true;
        // 触发回调
        if let Some(ref mut cb) = self.on_cancel { cb(); }
    }
    pub fn confirm(&mut self) { if let Some(ref mut cb) = self.on_ok { cb(); } self.close(); }
}
