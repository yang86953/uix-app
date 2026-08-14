//! 气泡确认框行为与几何。

use super::*;

// 构造、声明式配置和可见性生命周期保持在同一私有实现边界内。
mod builder;

impl Popconfirm {
    pub(crate) fn sync_from(&mut self, next: Self) {
        let geometry_changed = self.placement != next.placement || self.arrow != next.arrow;
        self.title = next.title;
        self.confirm_text = next.confirm_text;
        self.cancel_text = next.cancel_text;
        self.placement = next.placement;
        self.arrow = next.arrow;
        self.icon = next.icon;
        // 同步声明期组合触发器模式。
        self.custom_trigger = next.custom_trigger;
        // 下一声明提供新的完整 trigger ViewNode 供组件树 reconcile。
        self.custom_trigger_view = next.custom_trigger_view;
        // 同步最新确认业务回调。
        self.confirm_callback = next.confirm_callback;
        // 同步最新取消业务回调。
        self.cancel_callback = next.cancel_callback;
        if geometry_changed {
            self.cancel_pending_activation();
        }
    }

    fn trigger_rect(&self) -> Rect {
        let frame = self.last_frame.get();
        // 组合模式优先消费本轮布局测得的真实子树尺寸。
        let trigger_size = self.trigger_size.get();
        let width = if self.custom_trigger && trigger_size.w > 0.0 {
            trigger_size.w
        } else if frame.w > 0.0 {
            frame.w
        } else {
            FALLBACK_TRIGGER_WIDTH
        };
        let height = if self.custom_trigger && trigger_size.h > 0.0 {
            trigger_size.h
        } else if frame.h > 0.0 {
            frame.h
        } else {
            FALLBACK_TRIGGER_HEIGHT
        };
        Rect::new(0.0, 0.0, width, height)
    }

    // 返回当前组合模式是否由真实子 trigger 拥有交互。
    pub(crate) fn uses_custom_trigger(&self) -> bool {
        // 只有已经挂载唯一子树时才启用代理语义。
        self.custom_trigger && self.trigger_child_count == 1
    }

    // 把组件 frame 收敛为实际 trigger 的绝对 border-box。
    pub(super) fn absolute_trigger_frame(&self, frame: Rect) -> Rect {
        // 组合模式使用本轮布局事实，兼容模式沿用组件 frame。
        let size = self.trigger_size.get();
        // 只有正尺寸组合 trigger 才覆盖包装节点尺寸。
        if self.uses_custom_trigger() && size.w > 0.0 && size.h > 0.0 {
            // trigger 与包装节点共享布局原点。
            Rect::new(frame.x, frame.y, size.w, size.h)
        } else {
            // 兼容或布局尚未完成时使用归一化包装 frame。
            Self::normalize_frame(frame)
        }
    }

    fn button_rects(&self) -> (Rect, Rect) {
        button_rects_for_popup(self.popup_rect.get())
    }

    pub(super) fn target_at(&self, pos: Point) -> Option<PopconfirmTarget> {
        if self.trigger_rect().contains(pos) {
            return Some(PopconfirmTarget::Trigger);
        }
        if !self.visible {
            return None;
        }
        let (confirm, cancel) = self.button_rects();
        if confirm.contains(pos) {
            Some(PopconfirmTarget::Confirm)
        } else if cancel.contains(pos) {
            Some(PopconfirmTarget::Cancel)
        } else {
            None
        }
    }

    pub(super) fn cancel_pending_activation(&mut self) {
        self.pressed_target = None;
        self.pressed_key = None;
    }

    // 执行唯一确认动作并保留兼容 Submit 语义。
    pub(crate) fn confirm_action(&mut self) {
        // 只允许稳定打开且未提交动作的实例进入回调。
        if !self.visible || self.closing || self.action_committed {
            // 离场或重复输入不得再次执行业务动作。
            return;
        }
        // 先锁定本轮动作，防止回调或后续事件重入。
        self.action_committed = true;
        // 克隆窄回调句柄以避免同步调用期间借用 self。
        if let Some(callback) = self.confirm_callback.clone() {
            // 在当前调用线程同步执行确认副作用。
            callback();
        }
        // 继续发布既有 Submit(id, "confirm") 兼容事实。
        self.pending_submit.set(true);
        // 回调返回后进入正常离场。
        self.close();
    }

    // 执行所有用户取消入口共用的唯一动作。
    pub(crate) fn cancel_action(&mut self) {
        // 只允许稳定打开且未提交动作的实例进入回调。
        if !self.visible || self.closing || self.action_committed {
            // 离场或重复输入不得再次执行业务动作。
            return;
        }
        // 先锁定本轮动作，防止 PointerUp、FocusOut 或动画重入。
        self.action_committed = true;
        // 克隆窄回调句柄以避免同步调用期间借用 self。
        if let Some(callback) = self.cancel_callback.clone() {
            // 在当前调用线程同步执行取消副作用。
            callback();
        }
        // 用户取消不发布 Submit 或 Change，只进入正常离场。
        self.close();
    }

    pub(super) fn intrinsic_size(&self) -> Size {
        Size::new(FALLBACK_TRIGGER_WIDTH, FALLBACK_TRIGGER_HEIGHT)
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Popconfirm(SnapshotPopconfirm {
            title: self.title.clone(),
            confirm_text: self.confirm_text.clone(),
            cancel_text: self.cancel_text.clone(),
            placement: self.placement,
            arrow: self.arrow,
            icon: self.icon,
            visible: self.visible,
            focused_action: self.focused_action(),
        })
    }

    pub(super) fn absolute_popup_rect(&self, frame: Rect) -> Rect {
        // 始终以当前表面重新解析，避免触发器不动时沿用旧窗口边界下的缓存。
        resolve_popconfirm_geometry(
            // 传入当前触发器矩形。
            self.absolute_trigger_frame(frame),
            // 传入布局阶段或绘制阶段记录的最新表面。
            self.surface_or_fallback(frame),
            // 保留作者指定位置。
            self.placement,
            // 保留箭头间距配置。
            self.arrow,
            // 使用标准确认气泡宽度。
            POPCONFIRM_WIDTH,
            // 使用标准确认气泡高度。
            POPCONFIRM_HEIGHT,
        )
        // 返回同一解析器生成的最终矩形。
        .popup
    }

    pub(super) fn surface_or_fallback(&self, frame: Rect) -> Rect {
        let surface = self.surface_rect.get();
        if surface.w > 0.0 && surface.h > 0.0 {
            surface
        } else {
            frame.union(&Rect::new(
                frame.x - POPCONFIRM_WIDTH * 2.0,
                frame.y - POPCONFIRM_HEIGHT * 2.0,
                POPCONFIRM_WIDTH * 5.0 + frame.w,
                POPCONFIRM_HEIGHT * 5.0 + frame.h,
            ))
        }
    }

    pub(super) fn dirty_rect_for_frame(&self, frame: Rect) -> Rect {
        let frame = Self::normalize_frame(frame);
        self.absolute_trigger_frame(frame)
            .union(&expand_popconfirm_rect(
                self.absolute_popup_rect(frame),
                12.0,
            ))
            .intersect(&self.surface_or_fallback(frame))
            .unwrap_or_default()
    }

    pub(super) fn normalize_frame(frame: Rect) -> Rect {
        Rect::new(
            if frame.x.is_finite() { frame.x } else { 0.0 },
            if frame.y.is_finite() { frame.y } else { 0.0 },
            if frame.w.is_finite() {
                frame.w.max(0.0)
            } else {
                0.0
            },
            if frame.h.is_finite() {
                frame.h.max(0.0)
            } else {
                0.0
            },
        )
    }

    pub(super) fn paint_elided_text(
        ctx: &mut PaintContext,
        value: &str,
        frame: Rect,
        color: Color,
        font_size: f32,
        centered: bool,
    ) {
        let Some(value) = Self::elide_single_line(ctx, value, font_size, frame.w) else {
            return;
        };
        if frame.h <= 0.0 {
            return;
        }
        ctx.push_clip(frame);
        if centered {
            ctx.text_center(&value, frame, color, font_size);
        } else {
            let y = ctx.visual_center_y(frame, font_size);
            ctx.draw_text(&value, Point::new(frame.x, y), color, font_size);
        }
        ctx.pop_clip();
    }

    fn elide_single_line(
        ctx: &mut PaintContext,
        value: &str,
        font_size: f32,
        max_width: f32,
    ) -> Option<String> {
        if !max_width.is_finite() || max_width <= 0.0 {
            return None;
        }
        let value = value.replace(['\r', '\n'], " ");
        if Self::text_width(ctx, &value, font_size) <= max_width {
            return Some(value);
        }
        const ELLIPSIS: &str = "…";
        if Self::text_width(ctx, ELLIPSIS, font_size) > max_width {
            return None;
        }
        let mut visible = String::new();
        for ch in value.chars() {
            visible.push(ch);
            visible.push_str(ELLIPSIS);
            let fits = Self::text_width(ctx, &visible, font_size) <= max_width;
            visible.pop();
            if !fits {
                visible.pop();
                break;
            }
        }
        visible.push_str(ELLIPSIS);
        Some(visible)
    }

    fn text_width(ctx: &mut PaintContext, value: &str, font_size: f32) -> f32 {
        ctx.measure_text(value, font_size).w.max(
            crate::draw::resources::font::text_backend::estimate_text_metrics(
                value,
                f32::INFINITY,
                font_size,
            )
            .max_line_width,
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PopconfirmGeometry {
    pub(crate) popup: Rect,
    pub(crate) placement: PopconfirmPlacement,
}

pub(super) fn resolve_popconfirm_geometry(
    trigger: Rect,
    surface: Rect,
    placement: PopconfirmPlacement,
    arrow: bool,
    preferred_width: f32,
    preferred_height: f32,
) -> PopconfirmGeometry {
    let width = preferred_width.min(surface.w).max(0.0);
    let height = preferred_height.min(surface.h).max(0.0);
    if width <= 0.0 || height <= 0.0 {
        return PopconfirmGeometry {
            popup: Rect::zero(),
            placement,
        };
    }
    let flipped = match placement {
        PopconfirmPlacement::Top => PopconfirmPlacement::Bottom,
        PopconfirmPlacement::TopLeft => PopconfirmPlacement::BottomLeft,
        PopconfirmPlacement::TopRight => PopconfirmPlacement::BottomRight,
        PopconfirmPlacement::Bottom => PopconfirmPlacement::Top,
        PopconfirmPlacement::BottomLeft => PopconfirmPlacement::TopLeft,
        PopconfirmPlacement::BottomRight => PopconfirmPlacement::TopRight,
    };
    let authored = rect_for_popconfirm_placement(trigger, placement, arrow, width, height);
    let alternate = rect_for_popconfirm_placement(trigger, flipped, arrow, width, height);
    let (candidate, resolved) = if popconfirm_overflow_score(alternate, surface)
        < popconfirm_overflow_score(authored, surface)
    {
        (alternate, flipped)
    } else {
        (authored, placement)
    };
    let max_x = surface.x + surface.w - width;
    let max_y = surface.y + surface.h - height;
    PopconfirmGeometry {
        popup: Rect::new(
            candidate.x.clamp(surface.x, max_x),
            candidate.y.clamp(surface.y, max_y),
            width,
            height,
        ),
        placement: resolved,
    }
}

pub(super) fn rect_for_popconfirm_placement(
    frame: Rect,
    placement: PopconfirmPlacement,
    arrow: bool,
    width: f32,
    height: f32,
) -> Rect {
    let (x, y) = popconfirm_position(frame, placement, arrow, width, height);
    Rect::new(x, y, width, height)
}

pub(super) fn popconfirm_overflow_score(rect: Rect, surface: Rect) -> f32 {
    (surface.x - rect.x).max(0.0)
        + (surface.y - rect.y).max(0.0)
        + (rect.x + rect.w - surface.x - surface.w).max(0.0)
        + (rect.y + rect.h - surface.y - surface.h).max(0.0)
}

pub(super) fn button_rects_for_popup(popup: Rect) -> (Rect, Rect) {
    let inset = 12.0_f32.min(popup.w * 0.5);
    let gap = 8.0_f32.min(popup.w);
    let available = (popup.w - inset * 2.0 - gap).max(0.0);
    let button_width = available * 0.5;
    let button_height = 26.0_f32.min((popup.h - 10.0).max(0.0));
    let y = (popup.y + popup.h - 10.0 - button_height).max(popup.y);
    (
        Rect::new(popup.x + inset, y, button_width, button_height),
        Rect::new(
            popup.x + inset + button_width + gap,
            y,
            button_width,
            button_height,
        ),
    )
}

pub(super) fn popconfirm_position(
    frame: Rect,
    placement: PopconfirmPlacement,
    arrow: bool,
    pw: f32,
    ph: f32,
) -> (f32, f32) {
    let gap = if arrow { 10.0 } else { 4.0 };
    match placement {
        PopconfirmPlacement::Top | PopconfirmPlacement::TopLeft => (frame.x, frame.y - ph - gap),
        PopconfirmPlacement::TopRight => (frame.x + frame.w - pw, frame.y - ph - gap),
        PopconfirmPlacement::Bottom | PopconfirmPlacement::BottomLeft => {
            (frame.x, frame.y + frame.h + gap)
        }
        PopconfirmPlacement::BottomRight => (frame.x + frame.w - pw, frame.y + frame.h + gap),
    }
}

pub(super) fn fade_color(color: Color, opacity: f32) -> Color {
    let alpha = (color.a as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    color.with_alpha(alpha)
}

pub(super) fn expand_popconfirm_rect(rect: Rect, amount: f32) -> Rect {
    if rect.w <= 0.0 || rect.h <= 0.0 {
        Rect::zero()
    } else {
        Rect::new(
            rect.x - amount,
            rect.y - amount,
            rect.w + amount * 2.0,
            rect.h + amount * 2.0,
        )
    }
}

pub(super) fn draw_popconfirm_arrow(
    ctx: &mut PaintContext,
    trigger: Rect,
    popup: Rect,
    placement: PopconfirmPlacement,
    color: Color,
) {
    let arrow_sz = 6.0;
    let (x1, y1, x2, y2, x3, y3) = match placement {
        PopconfirmPlacement::Top | PopconfirmPlacement::TopLeft | PopconfirmPlacement::TopRight => {
            let cx =
                popconfirm_arrow_anchor(trigger.x + trigger.w * 0.5, popup.x, popup.w, arrow_sz);
            (
                cx - arrow_sz,
                popup.y + popup.h,
                cx + arrow_sz,
                popup.y + popup.h,
                cx,
                popup.y + popup.h + arrow_sz,
            )
        }
        PopconfirmPlacement::Bottom
        | PopconfirmPlacement::BottomLeft
        | PopconfirmPlacement::BottomRight => {
            let cx =
                popconfirm_arrow_anchor(trigger.x + trigger.w * 0.5, popup.x, popup.w, arrow_sz);
            (
                cx - arrow_sz,
                popup.y,
                cx + arrow_sz,
                popup.y,
                cx,
                popup.y - arrow_sz,
            )
        }
    };
    let mut pb = PathBuilder::new();
    pb.move_to(x1, y1);
    pb.line_to(x2, y2);
    pb.line_to(x3, y3);
    pb.close();
    ctx.fill_path(&pb.build(), color, FillRule::NonZero);
}

pub(super) fn popconfirm_arrow_anchor(desired: f32, start: f32, length: f32, inset: f32) -> f32 {
    if length <= inset * 2.0 {
        start + length * 0.5
    } else {
        desired.clamp(start + inset, start + length - inset)
    }
}

// 仅在测试构建中编译确认气泡几何契约。
#[cfg(test)]
// 将测试放在同模块内以核验私有缓存状态。
mod tests {
    // 复用被测模块中的组件与几何辅助函数。
    use super::*;
    // 使用真实 ViewAdapter 发布组合 trigger 与处理器表。
    use crate::ui::adapter::ViewAdapter;
    // 引入组件与布局 trait 以核验组合生命周期和 frame。
    use crate::ui::component::traits::WidgetComponent;
    // 引入组件包装的 frame 读写入口。
    use crate::ui::component::widget::WidgetCore;
    // 引入标准指针修饰状态与事件路由类型。
    use crate::ui::{EventResult, KeyMod, MouseButton, SystemEvent};
    // 使用真实按钮 View 验证业务 Click 不被包装器吞掉。
    use crate::ui::widgets::button;

    // 标记表面缩放时缓存几何必须失效的回归契约。
    #[test]
    // 触发器不动时，缩小表面也必须重新约束确认气泡。
    fn popup_cache_does_not_survive_surface_resize() {
        // 构造顶部展开、会在窄表面内水平收敛的确认气泡。
        let mut popconfirm = Popconfirm::new().placement(PopconfirmPlacement::Top);
        // 打开确认气泡以覆盖真实的浮层登记路径。
        popconfirm.open();
        // 固定触发器位置以隔离表面尺寸这一项变量。
        let frame = Rect::new(200.0, 140.0, 40.0, 20.0);
        // 大表面允许气泡保持作者指定的水平位置。
        let large_surface = Rect::new(0.0, 0.0, 500.0, 300.0);
        // 小表面要求气泡向左约束到可见范围内。
        let small_surface = Rect::new(0.0, 0.0, 260.0, 200.0);
        // 计算大表面下已绘制并写入缓存的确认气泡矩形。
        let cached = resolve_popconfirm_geometry(
            // 传入固定触发器矩形。
            frame,
            // 传入初始大表面。
            large_surface,
            // 沿用组件作者指定位置。
            PopconfirmPlacement::Top,
            // 保留箭头间距。
            true,
            // 使用组件标准宽度。
            POPCONFIRM_WIDTH,
            // 使用组件标准高度。
            POPCONFIRM_HEIGHT,
        )
        // 只取最终确认气泡矩形。
        .popup;
        // 模拟上一帧绘制留下的触发器缓存键。
        popconfirm.last_frame.set(frame);
        // 模拟上一帧绘制留下的相对气泡缓存。
        popconfirm.popup_rect.set(Rect::new(
            // 保存相对触发器的横坐标。
            cached.x - frame.x,
            // 保存相对触发器的纵坐标。
            cached.y - frame.y,
            // 保存缓存宽度。
            cached.w,
            // 保存缓存高度。
            cached.h,
        ));
        // 先记录与缓存一致的大表面。
        popconfirm.surface_rect.set(large_surface);
        // 通过布局阶段的新能力注入缩小后的当前表面。
        let overlay = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
            // 传入被测确认气泡组件。
            &popconfirm,
            // 使用稳定的测试组件标识。
            crate::core::ComponentId::new(2),
            // 保持触发器 frame 不变。
            frame,
            // 仅改变当前逻辑表面。
            small_surface,
        );
        // 打开状态必须生成使用新表面的浮层登记。
        assert!(overlay.is_some());
        // 以新表面直接计算当前帧应使用的几何。
        let expected = resolve_popconfirm_geometry(
            // 触发器保持不变。
            frame,
            // 表面改为缩小后的尺寸。
            small_surface,
            // 位置配置保持不变。
            PopconfirmPlacement::Top,
            // 箭头配置保持不变。
            true,
            // 宽度配置保持不变。
            POPCONFIRM_WIDTH,
            // 高度配置保持不变。
            POPCONFIRM_HEIGHT,
        )
        // 只比较最终确认气泡矩形。
        .popup;

        // 缓存读取必须与新表面下的绘制几何一致。
        assert_eq!(popconfirm.absolute_popup_rect(frame), expected);
    }

    // 验证确认、取消和编程关闭使用互不混淆的单次动作契约。
    #[test]
    fn callbacks_fire_once_only_for_user_actions() {
        // 建立跨回调共享的确认计数。
        let confirms = Rc::new(Cell::new(0));
        // 为确认回调克隆独立共享句柄。
        let confirm_count = Rc::clone(&confirms);
        // 建立跨回调共享的取消计数。
        let cancels = Rc::new(Cell::new(0));
        // 为取消回调克隆独立共享句柄。
        let cancel_count = Rc::clone(&cancels);
        // 构造同时登记两类窄回调的确认气泡。
        let mut popconfirm = Popconfirm::new()
            // 确认回调只累加确认计数。
            .on_confirm(move || confirm_count.set(confirm_count.get() + 1))
            // 取消回调只累加取消计数。
            .on_cancel(move || cancel_count.set(cancel_count.get() + 1));

        // 首次打开建立稳定用户动作周期。
        popconfirm.open();
        // 执行一次确认动作。
        popconfirm.confirm_action();
        // 离场期间重复确认不得再次调用回调。
        popconfirm.confirm_action();
        // 确认回调必须恰好执行一次。
        assert_eq!(confirms.get(), 1);
        // 确认路径不得伪造取消。
        assert_eq!(cancels.get(), 0);
        // 兼容 Submit 事实必须只等待消费一次。
        assert!(popconfirm.pending_submit.replace(false));

        // 重新打开开始新的稳定动作周期。
        popconfirm.open();
        // 执行一次用户取消动作。
        popconfirm.cancel_action();
        // PointerUp、FocusOut 等重复取消不得再次调用回调。
        popconfirm.cancel_action();
        // 取消回调必须恰好执行一次。
        assert_eq!(cancels.get(), 1);

        // 再次打开以隔离编程关闭语义。
        popconfirm.open();
        // 公共 close 是无业务含义的编程关闭。
        popconfirm.close();
        // 编程关闭不得触发取消回调。
        assert_eq!(cancels.get(), 1);
    }

    // 验证真实 trigger 子树的尺寸、点击与焦点身份由子 View 保持。
    #[test]
    fn composite_trigger_keeps_natural_size_click_and_focus_identity() {
        // 建立真实 trigger 独占的业务点击计数。
        let clicks = Rc::new(Cell::new(0));
        // 为子 View 处理器克隆独立共享句柄。
        let child_clicks = Rc::clone(&clicks);
        // 构造长文本按钮以暴露旧固定八十像素裁剪问题。
        let trigger = button("删除这个很长的自定义项目").on_click_fn(move || {
            // 标准 Click 只由真实子 View 累加一次。
            child_clicks.set(child_clicks.get() + 1);
        });
        // 构造底部气泡，避免测试表面顶部翻转干扰触发器断言。
        let view = Popconfirm::new()
            // 配置可观察标题。
            .title("确定删除？")
            // 使用真实长文本按钮作为唯一 trigger。
            .trigger_view(trigger);
        // 通过真实适配器物化父子组件和处理器表。
        // Popconfirm 通过叶声明把私有 build_view_children 交给适配器展开。
        let mut tree = ViewAdapter::build(crate::ui::view::ViewNode::leaf(view));
        // 取得稳定 Popconfirm 根身份。
        let root = tree.root_id().expect("Popconfirm 根必须存在");
        // 取得唯一直接 trigger 子树身份。
        let child = tree.get(root).expect("Popconfirm 根必须可读").children()[0];
        // 模拟真实窗口把根组件安排到足够大的逻辑表面。
        tree.get_mut(root)
            // 根节点在布局前必须仍可寻址。
            .expect("Popconfirm 根必须可写")
            // 使用固定表面尺寸隔离自然 trigger 测量。
            .set_frame(Rect::new(0.0, 0.0, 500.0, 300.0));
        // 执行一次真实父子布局收敛。
        tree.layout();
        // 读取本轮安排出的实际子 trigger frame。
        let child_frame = tree.get(child).expect("trigger 子树必须可读").frame();
        // 长文本 trigger 宽度不得退回旧固定八十像素。
        assert!(child_frame.w > FALLBACK_TRIGGER_WIDTH);
        // trigger 高度必须来自按钮自己的自然尺寸。
        assert!(child_frame.h > 0.0);
        // 组合 owner 不得与真实 trigger 重复进入 Tab 顺序。
        assert_eq!(
            // 读取运行时 Popconfirm 组件的 Tab 契约。
            WidgetComponent::tab_index(
                tree.get(root)
                    // 根节点必须存在。
                    .expect("Popconfirm 根必须存在")
                    // 借用组件 trait 对象。
                    .component()
            ),
            // 组合 owner 必须退出 Tab 顺序。
            0
        );

        // 在真实 trigger 内部按下主指针。
        let down = tree.dispatch_event(&SystemEvent::PointerDown {
            // 坐标落在长按钮 border-box 内。
            pos: Point::new(8.0, 8.0),
            // 使用标准业务左键。
            button: MouseButton::Left,
            // 不携带组合修饰键。
            mods: KeyMod::NONE,
        });
        // 子按钮必须处理按下事件。
        assert_eq!(down, EventResult::Handled);
        // 在同一真实 trigger 内部释放主指针。
        let up = tree.dispatch_event(&SystemEvent::PointerUp {
            // 释放位置与按下位置一致。
            pos: Point::new(8.0, 8.0),
            // 使用配对左键。
            button: MouseButton::Left,
            // 不携带组合修饰键。
            mods: KeyMod::NONE,
        });
        // 子按钮必须处理释放事件并合成标准 Click。
        assert_eq!(up, EventResult::Handled);
        // 业务 Click 必须且只执行一次。
        assert_eq!(clicks.get(), 1);
        // 捕获阶段必须同时打开确认气泡。
        let runtime = tree
            // 读取根节点。
            .get(root)
            // 根节点必须保持可寻址。
            .expect("Popconfirm 根必须存在")
            // 下转到具体组件。
            .component()
            // 取得运行时类型视图。
            .as_any()
            // 窄化为 Popconfirm。
            .downcast_ref::<Popconfirm>()
            // 类型变化表示适配器发布了错误根组件。
            .expect("根组件必须是 Popconfirm");
        // 打开状态必须已经提交。
        assert!(runtime.is_visible());
        // 气泡动作后的焦点代理目标必须是真实 trigger 子树。
        assert_eq!(
            crate::ui::tree_widget_hooks::pointer_focus_target(&tree, root),
            child
        );

        // 先用 Escape 结束指针打开周期，并保留焦点代理到真实 trigger。
        assert_eq!(
            tree.dispatch_event(&SystemEvent::KeyDown {
                // 使用浮层统一拥有的取消键。
                key: KeyCode::Escape,
                // 本次取消不携带修饰键。
                mods: KeyMod::NONE,
            }),
            EventResult::Handled
        );
        // 关闭态 Enter 按下必须由父级捕获并继续交给真实子按钮。
        assert_eq!(
            tree.dispatch_event(&SystemEvent::KeyDown {
                // 使用标准键盘激活键。
                key: KeyCode::Enter,
                // 本次激活不携带修饰键。
                mods: KeyMod::NONE,
            }),
            EventResult::Handled
        );
        // 配对 KeyUp 必须完成父级打开与子按钮唯一 Click。
        assert_eq!(
            tree.dispatch_event(&SystemEvent::KeyUp {
                // 释放与按下相同的激活键。
                key: KeyCode::Enter,
                // KeyUp 事件无需重复修饰键。
                mods: KeyMod::NONE,
            }),
            EventResult::Handled
        );
        // 键盘周期必须重新打开确认气泡。
        let runtime = tree.get(root).expect("Popconfirm 根必须存在");
        // 读取具体组件确认父级已完成开关动作。
        let runtime = runtime
            .component()
            .as_any()
            .downcast_ref::<Popconfirm>()
            .expect("根组件必须是 Popconfirm");
        // Enter 释放后气泡必须可见。
        assert!(runtime.is_visible());
        // 指针与键盘各自只能让真实 trigger Click 一次。
        assert_eq!(clicks.get(), 2);
        // 构造打开态确认动作的按下事件。
        let confirm_down = SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        };
        // 构造与确认按下配对的释放事件。
        let confirm_up = SystemEvent::KeyUp {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        };
        // 打开态 Enter 按下必须继续由父级确认组件接管。
        assert_eq!(tree.dispatch_event(&confirm_down), EventResult::Handled);
        // 配对释放必须提交确认动作并启动离场。
        assert_eq!(tree.dispatch_event(&confirm_up), EventResult::Handled);
        // 读取确认后的运行时状态，区分业务关闭与动画完成。
        let runtime = tree
            .get(root)
            .expect("Popconfirm 根必须存在")
            .component()
            .as_any()
            .downcast_ref::<Popconfirm>()
            .expect("根组件必须是 Popconfirm");
        // 确认后应立即结束可见态并保留离场呈现态。
        assert!(!runtime.is_visible() && runtime.is_present());
    }
}
