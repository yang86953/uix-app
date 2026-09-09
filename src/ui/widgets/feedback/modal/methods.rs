use super::{
    Modal, ModalBuilder, ModalContext, ModalPointerTarget, presentation::MODAL_VISUAL_REF,
};

use std::cell::{Cell, RefCell};

use crate::core::{Point, Rect, Size};
use crate::platform::windowing::ControlSize;
use crate::ui::SnapshotFields;
use crate::ui::animation::{AnimationConfig, TransitionPlayer};

use std::rc::Rc;

/// Modal 内容回调上下文；关闭请求只作用于持有该上下文的 Modal。
impl Modal {
    /// 创建默认关闭、居中、带遮罩关闭能力和底部操作区的模态框。
    pub fn new(title: &str) -> Self {
        let size = crate::ui::widget_runtime::config::use_config().size;
        // 直接构造与 UIX View 构建共同读取同一视觉静态项。
        let visual = MODAL_VISUAL_REF;
        // 三档尺寸只由 UIX 默认表决定。
        let (width, height) = visual.defaults.dimensions(size);
        let enter_animation = AnimationConfig::zoom_in(visual.motion.enter_duration);
        let leave_animation = AnimationConfig::zoom_out(visual.motion.leave_duration);
        Self {
            title: title.into(),
            visible: false,
            width,
            height,
            modal_size: size,
            closable: visual.defaults.closable,
            mask_closable: visual.defaults.mask_closable,
            footer_visible: visual.defaults.footer_visible,
            centered: visual.defaults.centered,
            overlay: visual.defaults.overlay,
            // backdrop blur 默认关闭，遵守显式 opt-in 决策。
            backdrop_blur: None,
            destroy_on_close: visual.defaults.destroy_on_close,
            controlled: None,
            context_close_requested: None,
            // 默认没有确认业务回调。
            ok_callback: None,
            // 默认没有取消业务回调。
            cancel_callback: None,
            last_win_w: Cell::new(0.0),
            last_win_h: Cell::new(0.0),
            enter_animation,
            leave_animation,
            transition: TransitionPlayer::new(enter_animation),
            closing: false,
            transition_dirty: false,
            layout_requested: Cell::new(false),
            last_frame: Cell::new(Rect::zero()),
            last_trigger_rect: Cell::new(Rect::new(
                0.0,
                0.0,
                visual.layout.trigger_width,
                visual.layout.trigger_height,
            )),
            last_dialog_rect: Cell::new(Rect::zero()),
            close_hovered: Cell::new(false),
            // 默认没有悬停的底部操作。
            footer_hovered: Cell::new(None),
            pressed_target: Cell::new(None),
            activation_key: Cell::new(None),
            visual,
        }
    }

    /// 设置初始可见性，并通过正式打开或关闭生命周期应用状态。
    pub fn visible(mut self, v: bool) -> Self {
        self.set_visible(v);
        self
    }

    /// 构建默认打开的声明式 Modal；内容可通过 [`ModalContext::close`] 关闭它。
    pub fn show<V>(content: impl FnOnce(ModalContext) -> V) -> ModalBuilder
    where
        V: crate::ui::view::View,
    {
        let context = ModalContext::new();
        let content = crate::ui::view::View::build(content(context.clone()));
        let mut modal = Self::new("").visible(true).overlay(true);
        modal.footer_visible = false;
        modal.context_close_requested = Some(context.close_requested);
        // 快捷构建器仍只保存一个显式内容 View。
        ModalBuilder {
            // 交出配置完成的 Modal。
            modal,
            // 把单内容包装为有序集合。
            content: vec![content],
        }
    }

    /// 构建受控声明式 Modal（E-03）：配合 `.open(&State<bool>)` 由业务状态
    /// 驱动可见性，`.on_open_change` 在用户侧关闭时回调；`closable(false)`
    /// 禁用外部关闭但保留 Escape。
    pub fn builder() -> ModalBuilder {
        let mut modal = Self::new("").overlay(true);
        modal.footer_visible = false;
        let content = crate::ui::render_empty_for::<Modal>()
            .unwrap_or_else(|| crate::ui::view::View::build(crate::ui::widgets::label("")));
        // 受控构建器以单个空内容节点保持既有默认形状。
        ModalBuilder {
            // 交出配置完成的 Modal。
            modal,
            // 把默认内容包装为有序集合。
            content: vec![content],
        }
    }

    /// 快捷确认对话框；回调由应用在接入业务动作时持有。
    pub fn confirm<Ok, Cancel>(
        title: impl Into<String>,
        content: impl Into<String>,
        ok_callback: Ok,
        cancel_callback: Cancel,
    ) -> ModalBuilder
    where
        Ok: FnOnce() + 'static,
        Cancel: FnOnce() + 'static,
    {
        let ok_callback = Rc::new(RefCell::new(Some(ok_callback)));
        let cancel_callback = Rc::new(RefCell::new(Some(cancel_callback)));
        let completed = Rc::new(Cell::new(false));
        let content = content.into();
        Self::show(move |context| {
            let cancel_context = context.clone();
            let ok_context = context;
            let cancel_callback = cancel_callback.clone();
            let ok_callback = ok_callback.clone();
            let cancel_completed = completed.clone();
            let ok_completed = completed;
            let cancel = crate::ui::widgets::button("取消").on_click_fn(move || {
                if cancel_completed.replace(true) {
                    return;
                }
                if let Some(callback) = cancel_callback.borrow_mut().take() {
                    callback();
                }
                cancel_context.close();
            });
            let confirm = crate::ui::widgets::button("确定")
                .primary()
                .on_click_fn(move || {
                    if ok_completed.replace(true) {
                        return;
                    }
                    if let Some(callback) = ok_callback.borrow_mut().take() {
                        callback();
                    }
                    ok_context.close();
                });
            crate::ui::widgets::column((
                crate::ui::widgets::label(content),
                crate::ui::widgets::row((cancel, confirm)).gap(8.0),
            ))
            .gap(16.0)
        })
        .title(title)
    }

    /// 创建带信息图标和单个确定按钮的快捷对话框。
    pub fn info(title: impl Into<String>, content: impl Into<String>) -> ModalBuilder {
        Self::shortcut(
            title,
            content,
            "info",
            "信息",
            crate::ui::PaletteColor::Info,
        )
    }

    /// 创建带警告图标和单个确定按钮的快捷对话框。
    pub fn warning(title: impl Into<String>, content: impl Into<String>) -> ModalBuilder {
        Self::shortcut(
            title,
            content,
            "alert-triangle",
            "警告",
            crate::ui::PaletteColor::Warning,
        )
    }

    /// 创建带错误图标和单个确定按钮的快捷对话框。
    pub fn error(title: impl Into<String>, content: impl Into<String>) -> ModalBuilder {
        Self::shortcut(
            title,
            content,
            "x-circle",
            "错误",
            crate::ui::PaletteColor::Error,
        )
    }

    pub(crate) fn shortcut(
        title: impl Into<String>,
        content: impl Into<String>,
        icon_name: &'static str,
        status_name: &'static str,
        status_color: crate::ui::PaletteColor,
    ) -> ModalBuilder {
        let title = title.into();
        let content = content.into();
        Self::show(move |context| {
            let status_icon =
                crate::ui::widgets::embed(crate::ui::widgets::Icon::new(icon_name).size(24.0))
                    .color(crate::ui::ColorValue::Palette(status_color))
                    .role(crate::ui::AccessibilityRole::Image)
                    .accessible_name(status_name);
            let message =
                crate::ui::widgets::row((status_icon, crate::ui::widgets::label(content)))
                    .align(crate::ui::layout::AlignItems::Center)
                    .gap(12.0);
            let confirm = crate::ui::widgets::button("确定")
                .primary()
                .on_click_fn(move || context.close());
            crate::ui::widgets::column((message, confirm)).gap(16.0)
        })
        .title(title)
    }

    /// 设置自定义对话框宽高，并将非法尺寸归一化。
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.width = Self::normalize_dimension(w);
        self.height = Self::normalize_dimension(h);
        self
    }

    /// 接收 UIX 公共 width/height 样式并写入 Modal 自己的对话框几何。
    pub(crate) fn apply_view_layout_style(&mut self, style: &crate::ui::theme::style::Style) {
        if let Some(width) = style.width {
            self.width = Self::normalize_dimension(width);
        }
        if let Some(height) = style.height {
            self.height = Self::normalize_dimension(height);
        }
    }

    /// 设置控件尺寸档位及其对应的预设宽高。
    pub fn modal_size(mut self, s: ControlSize) -> Self {
        self.modal_size = s;
        // 预设尺寸与直接构造共同读取 UIX 唯一表。
        (self.width, self.height) = self.visual.defaults.dimensions(s);
        self
    }

    /// 设置是否显示并响应右上角关闭按钮。
    pub fn closable(mut self, v: bool) -> Self {
        self.closable = v;
        self
    }

    /// 设置点击遮罩区域是否关闭对话框。
    pub fn mask_closable(mut self, v: bool) -> Self {
        self.mask_closable = v;
        self
    }

    /// 设置默认底部确认和取消操作区是否可见。
    pub fn footer_visible(mut self, v: bool) -> Self {
        self.footer_visible = v;
        self
    }

    /// 设置非浮层模式下是否在可用区域中居中对话框。
    pub fn centered(mut self, v: bool) -> Self {
        self.centered = v;
        self
    }

    /// 设置是否把对话框登记为覆盖宿主表面的浮层。
    pub fn overlay(mut self, v: bool) -> Self {
        self.overlay = v;
        self
    }

    /// 为 Modal 显式启用统一 backdrop blur 请求。
    pub fn backdrop_blur(mut self, blur: crate::ui::OverlayBackdropBlur) -> Self {
        // 保存 typed 请求，实际 Theme 与区域在 ScenePaint 桥接时解析。
        self.backdrop_blur = Some(blur);
        // 返回配置完成的 Modal。
        self
    }

    /// 设置退出动画完成后是否从组件树销毁关闭的内容子树。
    pub fn destroy_on_close(mut self, v: bool) -> Self {
        self.destroy_on_close = v;
        self
    }

    /// 设置打开时播放的动画；已打开时从当前声明重新开始进场。
    pub fn enter_animation(mut self, animation: AnimationConfig) -> Self {
        self.enter_animation = animation;
        if self.visible && !self.closing {
            self.restart_enter_transition();
        }
        self
    }

    /// 设置关闭时播放的动画。
    pub fn leave_animation(mut self, animation: AnimationConfig) -> Self {
        self.leave_animation = animation;
        if self.closing {
            self.transition = TransitionPlayer::new(animation);
            self.transition_dirty = true;
        }
        self
    }

    /// 返回对话框是否处于稳定打开状态。
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// 通过正式生命周期打开或关闭对话框。
    pub fn set_visible(&mut self, v: bool) {
        if v {
            self.open();
        } else {
            self.close();
        }
    }

    /// 打开对话框、启动进入动画并同步受控状态。
    pub fn open(&mut self) {
        // 记录调用前是否已经处于稳定打开态，保证回调幂等。
        let was_open = self.visible && !self.closing;
        // 复用唯一进场状态转换。
        self.do_open();
        // 只有真实的关闭到打开转换才写回并通知受控状态。
        if !was_open && let Some(controlled) = &self.controlled {
            // 把组件确认的打开事实写回唯一业务状态源。
            controlled.state.set(true);
            // 同步通知当前实例持有的窄回调。
            controlled.notify(true);
        }
    }

    /// 关闭对话框、启动退出动画并同步受控状态。
    pub fn close(&mut self) {
        // 记录调用前是否处于可关闭的稳定打开态。
        let was_open = self.visible && !self.closing;
        // 复用唯一离场状态转换。
        self.do_close();
        // 重复关闭或离场期间的关闭不得重复写回和通知。
        if was_open && let Some(controlled) = &self.controlled {
            // 把组件确认的关闭事实写回唯一业务状态源。
            controlled.state.set(false);
            // 同步通知当前实例持有的窄回调。
            controlled.notify(false);
        }
    }

    // 执行底部确认操作并进入正常离场。
    pub(crate) fn confirm_action(&mut self) {
        // 已关闭或离场中的实例不得重复执行业务回调。
        if !self.visible || self.closing {
            // 直接保持当前生命周期状态。
            return;
        }
        // 克隆窄回调句柄，避免用户回调重入时保持对 self 的借用。
        if let Some(callback) = self.ok_callback.clone() {
            // 在调用线程同步执行确认业务动作。
            callback();
        }
        // 回调返回后关闭并写回受控状态。
        self.close();
    }

    // 执行取消语义并进入正常离场。
    pub(crate) fn cancel(&mut self) {
        // 已关闭或离场中的实例不得重复执行业务回调。
        if !self.visible || self.closing {
            // 直接保持当前生命周期状态。
            return;
        }
        // 克隆窄回调句柄，避免用户回调重入时保持对 self 的借用。
        if let Some(callback) = self.cancel_callback.clone() {
            // 在调用线程同步执行取消业务动作。
            callback();
        }
        // 回调返回后关闭并写回受控状态。
        self.close();
    }

    // 把命中目标映射为 Modal 自身拥有的同步操作。
    pub(crate) fn activate_target(&mut self, target: ModalPointerTarget) {
        // 离场阶段不接受任何新激活。
        if self.closing {
            // 保持离场不可重入。
            return;
        }
        // 按目标选择唯一操作语义。
        match target {
            // 关闭状态下的触发器执行打开。
            ModalPointerTarget::Trigger => self.open(),
            // 标题栏、遮罩与取消按钮统一为取消。
            ModalPointerTarget::Close | ModalPointerTarget::Mask | ModalPointerTarget::Cancel => {
                // 执行取消回调并关闭。
                self.cancel();
            }
            // 确认按钮执行确认。
            ModalPointerTarget::Ok => self.confirm_action(),
        }
    }

    pub(crate) fn do_open(&mut self) {
        self.cancel_interaction();
        self.visible = true;
        self.closing = false;
        self.restart_enter_transition();
        self.layout_requested.set(true);
    }

    pub(crate) fn do_close(&mut self) {
        self.cancel_interaction();
        if !self.is_present() {
            self.visible = false;
            self.closing = false;
            self.transition_dirty = false;
            return;
        }
        self.visible = false;
        self.closing = true;
        self.transition = TransitionPlayer::new(self.leave_animation);
        self.transition_dirty = true;
        self.layout_requested.set(true);
    }

    /// 受控跟随（E-03）：每帧把受控 State 同步到 visible；外部 `set(true)`
    /// 在下一帧打开，`set(false)` 走正常离场动画。
    pub(crate) fn controlled_sync(&mut self) {
        if let Some(controlled) = &self.controlled {
            let want_open = controlled.want_open();
            let is_open = self.visible && !self.closing;
            if want_open != is_open {
                if want_open {
                    self.do_open();
                } else {
                    self.do_close();
                }
            }
        }
    }

    /// 执行确认回调，并按正常关闭生命周期离开对话框。
    pub fn confirm_close(&mut self) {
        // 公开确认关闭入口复用确认回调与受控写回语义。
        self.confirm_action();
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        // 记录声明式可见性变化，避免关闭动画期间每次重建都重启离场。
        let visibility_changed = self.visible != next.visible;
        // 只有声明值真正变化时才同步运行态，保留进行中的离场动画。
        if visibility_changed {
            // 声明式打开需要重置关闭状态并启动进入动画。
            if next.visible {
                // 复用正式打开路径，确保 present、transition 和布局请求一致。
                self.do_open();
            } else {
                // 复用正式关闭路径，确保离场动画只启动一次。
                self.do_close();
            }
        }
        let interaction_geometry_changed = self.width != next.width
            || self.height != next.height
            || self.closable != next.closable
            || self.mask_closable != next.mask_closable
            // 底部显隐改变内容、命中与裁剪几何。
            || self.footer_visible != next.footer_visible
            || self.centered != next.centered
            || self.overlay != next.overlay
            || !std::ptr::eq(self.visual, next.visual);
        self.title = next.title;
        self.width = next.width;
        self.height = next.height;
        self.modal_size = next.modal_size;
        self.closable = next.closable;
        self.mask_closable = next.mask_closable;
        self.footer_visible = next.footer_visible;
        self.centered = next.centered;
        self.overlay = next.overlay;
        // 声明重建同步最新 backdrop blur 请求。
        self.backdrop_blur = next.backdrop_blur;
        self.destroy_on_close = next.destroy_on_close;
        self.context_close_requested = next.context_close_requested;
        // 声明式重建替换当前实例的确认回调。
        self.ok_callback = next.ok_callback;
        // 声明式重建替换当前实例的取消回调。
        self.cancel_callback = next.cancel_callback;
        if next.controlled.is_some() {
            self.controlled = next.controlled;
        }
        self.enter_animation = next.enter_animation;
        self.leave_animation = next.leave_animation;
        // UIX 静态项按共享引用替换，不复制完整视觉表。
        self.visual = next.visual;
        if interaction_geometry_changed {
            self.cancel_interaction();
            // 尺寸、页脚或定位策略变化必须让现有内容子树重新取得 dialog frame。
            self.layout_requested.set(true);
        }
    }

    pub(crate) fn dialog_rect_for_surface(&self, surface: Rect) -> Rect {
        let surface = Self::normalize_frame(surface);
        // 连续缩窗时始终保留可见安全边距；极窄表面按比例收敛而不产生负尺寸。
        let margin_x = self
            .visual
            .layout
            .safe_margin
            .min(surface.w * self.visual.layout.safe_margin_ratio);
        let margin_y = self
            .visual
            .layout
            .safe_margin
            .min(surface.h * self.visual.layout.safe_margin_ratio);
        let available_w = (surface.w - margin_x * 2.0).max(0.0);
        let available_h = (surface.h - margin_y * 2.0).max(0.0);
        let width = Self::normalize_dimension(self.width).min(available_w);
        let height = Self::normalize_dimension(self.height).min(available_h);
        let (x, y) = if self.centered || self.overlay {
            (
                surface.x + (surface.w - width) * 0.5,
                surface.y + (surface.h - height) * 0.5,
            )
        } else {
            (surface.x, surface.y)
        };
        Rect::new(x, y, width, height)
    }

    pub(crate) fn trigger_rect_for_size(&self, frame_w: f32, frame_h: f32) -> Rect {
        let frame_w = Self::normalize_dimension(frame_w);
        let frame_h = Self::normalize_dimension(frame_h);
        let width = frame_w.min(self.visual.layout.trigger_width);
        Rect::new(
            (frame_w - width) * 0.5,
            0.0,
            width,
            frame_h.min(self.visual.layout.trigger_height),
        )
    }

    pub(crate) fn trigger_rect_local(&self) -> Rect {
        let trigger = self.last_trigger_rect.get();
        if trigger.w > 0.0 && trigger.h > 0.0 {
            trigger
        } else {
            Rect::new(
                0.0,
                0.0,
                self.visual.layout.trigger_width,
                self.visual.layout.trigger_height,
            )
        }
    }

    pub(crate) fn dialog_rect_local(&self) -> Rect {
        let frame = self.last_frame.get();
        let dialog = self.last_dialog_rect.get();
        let dialog = if dialog.w > 0.0 && dialog.h > 0.0 {
            dialog
        } else if self.overlay {
            self.dialog_rect_for_surface(Rect::new(
                0.0,
                0.0,
                self.last_win_w.get(),
                self.last_win_h.get(),
            ))
        } else {
            self.dialog_rect_for_surface(frame)
        };
        Rect::new(dialog.x - frame.x, dialog.y - frame.y, dialog.w, dialog.h)
    }

    pub(crate) fn close_rect_local(&self) -> Rect {
        let dialog = self.dialog_rect_local();
        let width = dialog.w.min(self.visual.layout.close_width);
        Rect::new(
            dialog.x + dialog.w - width,
            dialog.y,
            width,
            dialog.h.min(self.visual.layout.header_height),
        )
    }

    // 从最终对话框几何派生取消与确认按钮，供绘制和命中共同使用。
    pub(crate) fn footer_action_rects(&self, dialog: Rect) -> (Rect, Rect) {
        let layout = &self.visual.layout;
        // 标题高度与内容布局保持同一约束。
        let header_height = dialog.h.min(layout.header_height);
        // 底部区域最多占用五十六逻辑像素。
        let footer_height = (dialog.h - header_height).clamp(0.0, layout.footer_height);
        // 水平内边距在窄对话框内自适应收敛。
        let horizontal_padding = layout
            .footer_side_inset
            .min(dialog.w.max(0.0) * layout.footer_side_inset_ratio);
        // 两按钮间距同样收敛到可用宽度。
        let gap = layout
            .footer_gap
            .min(dialog.w.max(0.0) * layout.footer_gap_ratio);
        // 计算扣除边距与间距后的按钮总可用宽度。
        let available_width = (dialog.w - horizontal_padding * 2.0 - gap).max(0.0);
        // 每个按钮不超过八十逻辑像素且平分可用空间。
        let button_width = (available_width * 0.5).min(layout.footer_button_max_width);
        // 底部上下各保留八像素，并限制标准按钮高度。
        let button_height = (footer_height - layout.footer_button_vertical_inset)
            .clamp(0.0, layout.footer_button_max_height);
        // 在底部区域内垂直居中按钮。
        let button_y = dialog.y + dialog.h - footer_height + (footer_height - button_height) * 0.5;
        // 确认按钮靠右排列。
        let ok_x = dialog.x + dialog.w - horizontal_padding - button_width;
        // 取消按钮位于确认按钮左侧。
        let cancel_x = ok_x - gap - button_width;
        // 构造取消按钮最终矩形。
        let cancel = Rect::new(cancel_x, button_y, button_width, button_height);
        // 构造确认按钮最终矩形。
        let ok = Rect::new(ok_x, button_y, button_width, button_height);
        // 返回固定顺序的取消和确认几何。
        (cancel, ok)
    }

    pub(crate) fn pointer_target_at(&self, pos: Point) -> Option<ModalPointerTarget> {
        // 事件已是 Modal 本地坐标；把面板动画逆变换后复用稳定的命中几何。
        let frame = self.last_frame.get();
        let point = Point::new(pos.x + frame.x, pos.y + frame.y);
        let point = self
            .dialog_transition_transform(self.last_dialog_rect.get())
            .inverse()?
            .transform_point(point);
        let pos = Point::new(point.x - frame.x, point.y - frame.y);
        // 标题栏关闭槽优先于其他目标。
        if self.closable && self.close_rect_local().contains(pos) {
            // 返回关闭目标。
            Some(ModalPointerTarget::Close)
        } else if self.footer_visible {
            // 从最终本地对话框派生底部操作几何。
            let (cancel, ok) = self.footer_action_rects(self.dialog_rect_local());
            // 取消按钮优先匹配自己的非重叠区域。
            if cancel.contains(pos) {
                // 返回取消目标。
                Some(ModalPointerTarget::Cancel)
            } else if ok.contains(pos) {
                // 返回确认目标。
                Some(ModalPointerTarget::Ok)
            } else if self.mask_closable && !self.dialog_rect_local().contains(pos) {
                // 对话框外仍由遮罩取消策略处理。
                Some(ModalPointerTarget::Mask)
            } else {
                // 对话框内容区没有内置操作目标。
                None
            }
        } else if self.mask_closable && !self.dialog_rect_local().contains(pos) {
            // 底部隐藏时仍保留遮罩关闭。
            Some(ModalPointerTarget::Mask)
        } else {
            // 没有命中任何 Modal 自有目标。
            None
        }
    }

    // 同步关闭槽与底部操作的悬停状态。
    pub(crate) fn update_hover_target(&self, target: Option<ModalPointerTarget>) {
        // 关闭槽只读取关闭目标。
        self.close_hovered
            // 保存是否悬停关闭槽。
            .set(target == Some(ModalPointerTarget::Close));
        // 底部只保存确认或取消目标。
        self.footer_hovered.set(match target {
            // 保留可绘制的底部操作目标。
            Some(target @ (ModalPointerTarget::Cancel | ModalPointerTarget::Ok)) => Some(target),
            // 其他目标清空底部悬停。
            _ => None,
        });
    }

    pub(crate) fn cancel_interaction(&self) {
        self.close_hovered.set(false);
        // 清空底部操作悬停。
        self.footer_hovered.set(None);
        self.pressed_target.set(None);
        self.activation_key.set(None);
    }

    pub(crate) fn body_rect(&self, dialog: Rect) -> Rect {
        let header_h = dialog.h.min(self.visual.layout.header_height);
        let footer_h = if self.footer_visible {
            (dialog.h - header_h).clamp(0.0, self.visual.layout.footer_height)
        } else {
            0.0
        };
        Rect::new(
            dialog.x,
            dialog.y + header_h,
            dialog.w.max(0.0),
            (dialog.h - header_h - footer_h).max(0.0),
        )
    }

    pub(crate) fn normalize_frame(frame: Rect) -> Rect {
        Rect::new(
            if frame.x.is_finite() { frame.x } else { 0.0 },
            if frame.y.is_finite() { frame.y } else { 0.0 },
            Self::normalize_dimension(frame.w),
            Self::normalize_dimension(frame.h),
        )
    }

    pub(crate) fn normalize_dimension(value: f32) -> f32 {
        if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        }
    }

    pub(crate) fn inset_rect(frame: Rect, inset: f32) -> Rect {
        let inset_x = inset.min(frame.w * 0.5);
        let inset_y = inset.min(frame.h * 0.5);
        Rect::new(
            frame.x + inset_x,
            frame.y + inset_y,
            (frame.w - inset_x * 2.0).max(0.0),
            (frame.h - inset_y * 2.0).max(0.0),
        )
    }

    pub(crate) fn is_present(&self) -> bool {
        self.visible || self.closing
    }

    pub(crate) fn should_destroy_on_close(&self) -> bool {
        self.destroy_on_close
    }

    pub(crate) fn take_context_close_request(&self) -> bool {
        self.context_close_requested
            .as_ref()
            .is_some_and(|requested| requested.replace(false))
    }

    pub(crate) fn restart_enter_transition(&mut self) {
        let mut transition = TransitionPlayer::new(self.enter_animation);
        // 与 View 进出场一致：零时长动画在创建后立即完成，避免登记多余帧。
        if self.enter_animation.duration() <= 0.0 {
            transition.update(0.0);
        }
        self.transition = transition;
        self.transition_dirty = true;
    }

    pub(crate) fn transition_opacity(&self) -> f32 {
        self.transition.opacity_progress.clamp(0.0, 1.0)
    }

    pub(crate) fn dialog_transition_transform(&self, rect: Rect) -> crate::draw::Transform {
        let scale = self.transition.scale.clamp(0.0, 1.0);
        let center_x = rect.x + rect.w * 0.5;
        let center_y = rect.y + rect.h * 0.5;
        crate::draw::Transform::translate(
            center_x + self.transition.offset.x,
            center_y + self.transition.offset.y,
        )
        .concat(crate::draw::Transform::scale(scale, scale))
        .concat(crate::draw::Transform::translate(-center_x, -center_y))
    }

    pub(crate) fn intrinsic_size(&self) -> Size {
        if self.is_present() {
            if self.overlay {
                Size::zero()
            } else {
                Size::new(
                    Self::normalize_dimension(self.width),
                    Self::normalize_dimension(self.height),
                )
            }
        } else {
            // Closed: reserve a trigger slot for gallery / live demos.
            Size::new(
                self.visual.layout.trigger_width,
                self.visual.layout.trigger_height,
            )
        }
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Modal {
            title: self.title.to_string(),
            open: self.is_present(),
            width: self.width,
            height: self.height,
            modal_size: self.modal_size,
            closable: self.closable,
            mask_closable: self.mask_closable,
            footer_visible: self.footer_visible,
            centered: self.centered,
            overlay: self.overlay,
            // 快照纳入效果请求，确保声明式变更可触发 reconcile。
            backdrop_blur: self.backdrop_blur,
        }
    }
}
