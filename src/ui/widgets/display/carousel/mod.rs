//! Carousel widget for switching between child content.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintPass;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::widget;

use crate::ui::children::WidgetChildren;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use crate::ui::view::{View, ViewNode};
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent, Widget, WidgetId, WidgetTree,
};
use crate::ui::{SemanticKind, SemanticPayload, SnapshotFields, ThemeTokens};

/// 轮播切换效果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarouselEffect {
    /// 使用滑动式页面切换。
    Slide,
    /// 使用前后页面淡入淡出切换。
    Fade,
}

// 保存由 UIX 声明的轮播默认尺寸、控制开关与切换效果。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CarouselDefaultsVisual {
    width: f32,
    height: f32,
    show_dots: bool,
    show_arrows: bool,
    effect: CarouselEffect,
    flex_grow: f32,
}

// 保存由 UIX 声明的默认箭头命中区、图标与缩放比例。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CarouselArrowVisual {
    hit_width: f32,
    icon_size: f32,
    icon_frame_ratio: f32,
    min_icon_size: f32,
    previous_icon: &'static str,
    next_icon: &'static str,
}

// 保存由 UIX 声明的页码点槽位、尺寸、圆角与激活态比例。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CarouselDotsVisual {
    slot_width: f32,
    hit_height: f32,
    height: f32,
    bottom_inset: f32,
    height_slot_ratio: f32,
    min_height: f32,
    active_width: f32,
    inactive_width: f32,
    active_fill_ratio: f32,
    inactive_fill_ratio: f32,
    center_ratio: f32,
    radius: f32,
}

// 保存由 UIX 声明的焦点圈几何与主题圆角角色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CarouselFocusVisual {
    inset: f32,
    stroke_width: f32,
    radius: CarouselRadiusRole,
}

// 保存由 UIX 声明的淡入淡出时长与三角透明度关键点。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CarouselMotionVisual {
    fade_duration_secs: f32,
    fade_start: f32,
    fade_midpoint: f32,
    fade_end: f32,
}

// 轮播焦点圈使用的主题圆角尺寸角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CarouselRadiusRole {
    Small,
}

impl CarouselRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

// 保存由 UIX 声明的轮播背景、控制与指示器主题角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CarouselPaletteVisual {
    background: ColorValue,
    primary: ColorValue,
    inactive_dot: ColorValue,
    arrow: ColorValue,
}

// 完整视觉配置由全部 Carousel 实例共享，实例只保存一个静态引用。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CarouselVisual {
    defaults: CarouselDefaultsVisual,
    arrow: CarouselArrowVisual,
    dots: CarouselDotsVisual,
    focus: CarouselFocusVisual,
    motion: CarouselMotionVisual,
    palette: CarouselPaletteVisual,
}

// 同目录 UIX 生成轮播全部分组视觉、根记录及稳定借用。
crate::uix_items!("src/ui/widgets/display/carousel/carousel.uix");

// 向 UIX 提供默认滑动效果。
const fn carousel_slide_effect() -> CarouselEffect {
    CarouselEffect::Slide
}

// 向 UIX 提供小圆角主题角色。
const fn carousel_small_radius() -> CarouselRadiusRole {
    CarouselRadiusRole::Small
}

// 向 UIX 提供轮播背景主题角色。
const fn carousel_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}

// 向 UIX 提供轮播品牌主色角色。
const fn carousel_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}

// 向 UIX 提供未激活页码点主题角色。
const fn carousel_inactive_dot() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextQuaternary)
}

// 向 UIX 提供默认箭头主题角色。
const fn carousel_arrow_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectionSource {
    User,
    Autoplay,
}

struct CarouselRuntime {
    current: Cell<usize>,
    child_count: Cell<usize>,
    pending_change: Cell<Option<usize>>,
    layout_requested: Cell<bool>,
    manually_paused: Cell<bool>,
    effect: Cell<CarouselEffect>,
    fade_from: Cell<Option<usize>>,
    fade_progress: Cell<f32>,
    fade_dirty: Cell<bool>,
    motion: Cell<&'static CarouselMotionVisual>,
}

impl CarouselRuntime {
    fn new(motion: &'static CarouselMotionVisual) -> Self {
        Self {
            current: Cell::new(0),
            child_count: Cell::new(0),
            pending_change: Cell::new(None),
            layout_requested: Cell::new(false),
            manually_paused: Cell::new(false),
            effect: Cell::new(CarouselEffect::Slide),
            fade_from: Cell::new(None),
            fade_progress: Cell::new(motion.fade_end),
            fade_dirty: Cell::new(false),
            motion: Cell::new(motion),
        }
    }

    fn copy_state_from(&self, current: &Self, effect: CarouselEffect) {
        self.current.set(current.current.get());
        self.child_count.set(current.child_count.get());
        self.pending_change.set(current.pending_change.get());
        self.layout_requested.set(current.layout_requested.get());
        self.manually_paused.set(current.manually_paused.get());
        self.effect.set(effect);
        if effect == CarouselEffect::Fade && current.effect.get() == CarouselEffect::Fade {
            self.fade_from.set(current.fade_from.get());
            self.fade_progress.set(current.fade_progress.get());
            self.fade_dirty.set(current.fade_dirty.get());
        } else {
            if current.fade_from.get().is_some() {
                self.layout_requested.set(true);
            }
            self.fade_from.set(None);
            self.fade_progress.set(self.motion.get().fade_end);
            self.fade_dirty.set(false);
        }
    }

    fn set_child_count(&self, count: usize) {
        // 先发布新的幻灯片数量，供其余运行态归一逻辑读取。
        self.child_count.set(count);
        // 空轮播不应继续交付旧幻灯片产生的语义变化。
        if count == 0 {
            // 丢弃已经失去目标的待发选择事件。
            self.pending_change.set(None);
            // 结构失效已负责调度布局，无需保留旧动画的额外请求。
            self.layout_requested.set(false);
            // 重新加入幻灯片后允许自动播放恢复。
            self.manually_paused.set(false);
            // 空轮播没有需要重绘的淡入淡出脏区。
            self.fade_dirty.set(false);
        }
        // 把活动索引限制在新的幻灯片范围内。
        let current = if count == 0 {
            0
        } else {
            self.current.get().min(count - 1)
        };
        // 发布归一后的活动索引。
        self.current.set(current);
        // 淡出来源越界时结束动画，避免重新加入后引用旧幻灯片。
        if self.fade_from.get().is_some_and(|index| index >= count) {
            // 恢复稳定的单页显示状态。
            self.finish_fade();
        }
    }

    fn select(&self, index: usize, source: SelectionSource) {
        let count = self.child_count.get();
        if count == 0 {
            return;
        }
        let index = index.min(count - 1);
        if index == self.current.get() {
            return;
        }

        let displayed = self.displayed_index();
        self.current.set(index);
        self.pending_change.set(Some(index));
        self.layout_requested.set(true);
        if source == SelectionSource::User {
            self.manually_paused.set(true);
        }
        if self.effect.get() == CarouselEffect::Fade {
            self.fade_from.set(Some(displayed));
            self.fade_progress.set(self.motion.get().fade_start);
            self.fade_dirty.set(true);
        } else {
            self.finish_fade();
        }
    }

    fn previous_index(&self) -> usize {
        let count = self.child_count.get();
        if count == 0 {
            0
        } else {
            (self.current.get() + count - 1) % count
        }
    }

    fn next_index(&self) -> usize {
        let count = self.child_count.get();
        if count == 0 {
            0
        } else {
            (self.current.get() + 1) % count
        }
    }

    fn displayed_index(&self) -> usize {
        if self.effect.get() == CarouselEffect::Fade
            && self.fade_progress.get() < self.motion.get().fade_midpoint
        {
            self.fade_from.get().unwrap_or_else(|| self.current.get())
        } else {
            self.current.get()
        }
    }

    fn fade_overlay_opacity(&self) -> f32 {
        if self.effect.get() != CarouselEffect::Fade || self.fade_from.get().is_none() {
            return 0.0;
        }
        let motion = self.motion.get();
        let progress = self
            .fade_progress
            .get()
            .clamp(motion.fade_start, motion.fade_end);
        if progress < motion.fade_midpoint {
            (progress - motion.fade_start)
                / (motion.fade_midpoint - motion.fade_start).max(f32::EPSILON)
        } else {
            (motion.fade_end - progress)
                / (motion.fade_end - motion.fade_midpoint).max(f32::EPSILON)
        }
    }

    fn finish_fade(&self) {
        self.fade_from.set(None);
        self.fade_progress.set(self.motion.get().fade_end);
    }
}

type CarouselArrowAction = Rc<dyn Fn()>;
type CarouselArrowFactory =
    dyn Fn(CarouselArrowAction, CarouselArrowAction) -> crate::ui::view::ViewNode;

#[derive(Clone, Copy)]
struct DotStrip {
    hit_rect: Rect,
    slot_width: f32,
    dot_y: f32,
    dot_height: f32,
}

widget! {
    /// Displays one child at a time with dot and arrow controls.
    pub struct Carousel {
        children: WidgetChildren,
        #[snapshot(skip)]
        runtime: Rc<CarouselRuntime>,
        show_dots: bool,
        #[snapshot(skip)]
        show_dots_authored: bool,
        show_arrows: bool,
        #[snapshot(skip)]
        show_arrows_authored: bool,
        autoplay: Option<Duration>,
        autoplay_timer_id: u32,
        effect: CarouselEffect,
        #[snapshot(skip)]
        effect_authored: bool,
        pause_on_hover: bool,
        custom_arrows: bool,
        #[snapshot(skip)]
        custom_arrows_view: Option<Rc<CarouselArrowFactory>>,
        hovered: bool,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
        focused: bool,
        last_frame: Cell<Option<Rect>>,
        #[snapshot(skip)]
        visual: &'static CarouselVisual,
    }

    tab_index => (&self) -> i32 { i32::from(self.runtime.child_count.get() > 1) }

    measure => (&self, constraints: Constraints) -> Size {
        let intrinsic = self.intrinsic_size();
        constraints.clamp(Size::new(
            self.fixed_width.unwrap_or(intrinsic.w),
            self.fixed_height.unwrap_or(intrinsic.h),
        ))
    }

    flex_grow => (&self) -> f32 {
        if self.fixed_width.is_some() || self.fixed_height.is_some() {
            0.0
        } else {
            self.visual.defaults.flex_grow
        }
    }

    build => (&self) -> Vec<Box<dyn Widget>> {
        let children = self.children.take();
        if !children.is_empty() {
            self.runtime.set_child_count(children.len());
        }
        children
    }

    on_children_changed => (&mut self, child_count: usize) {
        // 自定义箭头是直接子节点但不是幻灯片，需要从结构计数中扣除。
        let slide_count = child_count.saturating_sub(usize::from(self.has_custom_arrows()));
        // 立即同步轮播运行态，避免空节点被布局阶段跳过后保留旧计数。
        self.runtime.set_child_count(slide_count);
    }

    build_view_children => (&self) -> Vec<crate::ui::view::ViewNode> {
        // 自定义箭头只能在真实 Carousel owner 注册后由树级动态捕获入口执行。
        Vec::new()
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(frame) = self.last_frame.get() {
                    if let Some(index) = self.dot_index_at(frame, *pos) {
                        self.select(index, SelectionSource::User);
                        return EventResult::Handled;
                    }

                    let count = self.runtime.child_count.get();
                    if self.show_arrows
                        && !self.has_custom_arrows()
                        && count > 1
                        && frame.contains(*pos)
                    {
                        let (left, right) = self.arrow_frames(frame);
                        if pos.x < left.x + left.w {
                            self.select(self.previous_index(), SelectionSource::User);
                            return EventResult::Handled;
                        }
                        if pos.x >= right.x {
                            self.select(self.next_index(), SelectionSource::User);
                            return EventResult::Handled;
                        }
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerEnter => {
                self.hovered = true;
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                self.hovered = self.last_frame.get().is_some_and(|frame| frame.contains(*pos));
                EventResult::NotHandled
            }
            SystemEvent::PointerLeave | SystemEvent::WindowBlur => {
                self.hovered = false;
                EventResult::NotHandled
            }
            SystemEvent::Timer { id } if *id == self.autoplay_timer_id => {
                if self.autoplay.is_some()
                    && !self.runtime.manually_paused.get()
                    && !(self.pause_on_hover && self.hovered)
                {
                    self.select(self.next_index(), SelectionSource::Autoplay);
                }
                EventResult::Handled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.hovered = false;
                self.runtime.manually_paused.set(false);
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. }
                if self.focused && self.runtime.child_count.get() > 1 => match key {
                KeyCode::Left | KeyCode::Up => {
                    self.select(self.previous_index(), SelectionSource::User);
                    EventResult::Handled
                }
                KeyCode::Right | KeyCode::Down => {
                    self.select(self.next_index(), SelectionSource::User);
                    EventResult::Handled
                }
                KeyCode::Home => {
                    self.select(0, SelectionSource::User);
                    EventResult::Handled
                }
                KeyCode::End => {
                    self.select(self.runtime.child_count.get() - 1, SelectionSource::User);
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },
            _ => EventResult::NotHandled,
        }
    }

    on_focus_within => (&mut self, focused: bool) -> EventResult {
        if focused {
            return EventResult::NotHandled;
        }
        self.hovered = false;
        self.runtime.manually_paused.set(false);
        EventResult::NotHandled
    }

    wants_capture_phase => (&self) -> bool { true }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.runtime
            .pending_change
            .take()
            .map(|index| SemanticEvent::change(id, index.to_string()))
    }

    take_layout_request => (&mut self) -> bool {
        self.runtime.layout_requested.replace(false)
    }

    active_timer => (&self) -> Option<(u64, Duration)> {
        let paused = self.runtime.manually_paused.get() || (self.pause_on_hover && self.hovered);
        (self.runtime.child_count.get() > 1 && !paused)
            .then(|| self.autoplay.map(|interval| (u64::from(self.autoplay_timer_id), interval)))
            .flatten()
    }

    update_animation => (&mut self, dt: f64) -> bool {
        if self.effect != CarouselEffect::Fade || self.runtime.fade_from.get().is_none() {
            self.runtime.fade_dirty.set(false);
            return false;
        }
        let motion = self.runtime.motion.get();
        let delta = if dt.is_finite() && dt > 0.0 {
            (dt as f32 / motion.fade_duration_secs.max(f32::EPSILON)).max(0.0)
        } else {
            0.0
        };
        let previous = self.runtime.fade_progress.get();
        let progress = (previous + delta).min(motion.fade_end);
        if progress != previous {
            self.runtime.fade_progress.set(progress);
            self.runtime.fade_dirty.set(true);
        }
        if previous < motion.fade_midpoint && progress >= motion.fade_midpoint {
            self.runtime.layout_requested.set(true);
        }
        if progress >= motion.fade_end {
            self.runtime.finish_fade();
            false
        } else {
            true
        }
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.runtime.fade_dirty.replace(false) { frame } else { Rect::zero() }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let frame = Self::normalized_frame(frame);
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        let bg = self.visual.palette.background.resolve(ctx.tokens());
        if ctx.paint_pass() == PaintPass::Content {
            if frame.w > 0.0 && frame.h > 0.0 {
                ctx.fill_rect(frame, bg, None);
            }
            return;
        }

        let count = self.runtime.child_count.get();
        if count == 0 || frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let idx = self.runtime.current.get().min(count - 1);
        let primary = self.visual.palette.primary.resolve(ctx.tokens());
        let dot_color = self.visual.palette.inactive_dot.resolve(ctx.tokens());
        let arrow_color = self.visual.palette.arrow.resolve(ctx.tokens());
        ctx.push_clip(frame);

        let fade_opacity = self.runtime.fade_overlay_opacity();
        if fade_opacity > 0.0 {
            let alpha = (bg.a as f32 * fade_opacity)
                .round()
                .clamp(0.0, 255.0) as u8;
            ctx.fill_rect(frame, bg.with_alpha(alpha), None);
        }

        if self.show_arrows && !self.has_custom_arrows() && count > 1 {
            let (left, right) = self.arrow_frames(frame);
            let icon_size = self
                .visual
                .arrow
                .icon_size
                .min(left.w * self.visual.arrow.icon_frame_ratio)
                .min(frame.h * self.visual.arrow.icon_frame_ratio);
            if icon_size >= self.visual.arrow.min_icon_size {
                crate::ui::widgets::icon::Icon::paint_in_frame(
                    ctx,
                    self.visual.arrow.previous_icon,
                    left,
                    arrow_color,
                    icon_size,
                );
                crate::ui::widgets::icon::Icon::paint_in_frame(
                    ctx,
                    self.visual.arrow.next_icon,
                    right,
                    arrow_color,
                    icon_size,
                );
            }
        }

        if self.show_dots && count > 1 {
            let strip = self.dot_strip(frame, count);
            for i in 0..count {
                let is_active = i == idx;
                let preferred_width = if is_active {
                    self.visual.dots.active_width
                } else {
                    self.visual.dots.inactive_width
                };
                let fill_ratio = if is_active {
                    self.visual.dots.active_fill_ratio
                } else {
                    self.visual.dots.inactive_fill_ratio
                };
                let dot_width = preferred_width.min(strip.slot_width * fill_ratio);
                let dot_rect = Rect::new(
                    strip.hit_rect.x
                        + i as f32 * strip.slot_width
                        + (strip.slot_width - dot_width) * self.visual.dots.center_ratio,
                    strip.dot_y,
                    dot_width,
                    strip.dot_height,
                );
                ctx.fill_rect(
                    dot_rect,
                    if is_active { primary } else { dot_color },
                    Some(crate::draw::Radius::uniform(self.visual.dots.radius)),
                );
            }
        }
        if self.focused && tree.keyboard_focus_visible() {
            let inset = self
                .visual
                .focus
                .inset
                .min(frame.w * self.visual.dots.center_ratio)
                .min(frame.h * self.visual.dots.center_ratio);
            let focus_rect = Rect::new(
                frame.x + inset,
                frame.y + inset,
                (frame.w - inset * 2.0).max(0.0),
                (frame.h - inset * 2.0).max(0.0),
            );
            if focus_rect.w > 0.0 && focus_rect.h > 0.0 {
                ctx.stroke_rect(
                    focus_rect,
                    primary,
                    self.visual.focus.stroke_width,
                    Some(crate::draw::Radius::uniform(
                        self.visual.focus.radius.resolve(ctx.tokens()),
                    )),
                );
            }
        }
        ctx.pop_clip();
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(WidgetId, Rect)>
    {
        let frame = Self::normalized_frame(frame);
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        let slide_count = children
            .len()
            .saturating_sub(usize::from(self.has_custom_arrows()));
        self.runtime.set_child_count(slide_count);
        let displayed = self.runtime.displayed_index();
        children
            .iter()
            .enumerate()
            .map(|(index, child)| {
                let child_frame = if index >= slide_count || index == displayed {
                    frame
                } else {
                    Rect::new(frame.x, frame.y, 0.0, 0.0)
                };
                (child.id, child_frame)
            })
            .collect()
    }

    child_visible => (&self, index: usize) -> bool {
        index >= self.runtime.child_count.get() || index == self.runtime.displayed_index()
    }

    children_clip => (&self, frame: Rect) -> Option<Rect> {
        Some(frame)
    }
}

impl Default for Carousel {
    fn default() -> Self {
        Self::new()
    }
}

// 把轮播 Rust 内核与已有拥有型幻灯片子树融合为 UIX 声明的单一根节点。
fn build_carousel_view(
    mut kernel: Carousel,
    children: Vec<ViewNode>,
    visual: &'static CarouselVisual,
) -> ViewNode {
    if !kernel.show_dots_authored {
        kernel.show_dots = visual.defaults.show_dots;
    }
    if !kernel.show_arrows_authored {
        kernel.show_arrows = visual.defaults.show_arrows;
    }
    if !kernel.effect_authored {
        kernel.effect = visual.defaults.effect;
    }
    kernel.visual = visual;
    kernel.runtime.motion.set(&visual.motion);
    kernel.runtime.effect.set(kernel.effect);
    ViewNode::new(kernel, children)
}

impl View for Carousel {
    fn build(self) -> ViewNode {
        // 空轮播同样经由组件自己的 UIX 根声明构建。
        self.build_view_with_children(Vec::new())
    }
}

impl Carousel {
    // 固定自定义箭头的树内根 key，并与动态状态命名空间保持完全一致。
    pub(crate) const CUSTOM_ARROWS_CHILD_KEY: &'static str = "uix:carousel:custom-arrows";

    fn intrinsic_size(&self) -> Size {
        Size::new(self.visual.defaults.width, self.visual.defaults.height)
    }

    fn normalized_frame(frame: Rect) -> Rect {
        Rect::new(frame.x, frame.y, frame.w.max(0.0), frame.h.max(0.0))
    }

    fn arrow_frames(&self, frame: Rect) -> (Rect, Rect) {
        let width = self
            .visual
            .arrow
            .hit_width
            .min(frame.w * self.visual.dots.center_ratio);
        (
            Rect::new(frame.x, frame.y, width, frame.h),
            Rect::new(frame.x + frame.w - width, frame.y, width, frame.h),
        )
    }

    fn dot_strip(&self, frame: Rect, count: usize) -> DotStrip {
        let count_f = count.max(1) as f32;
        let total_width = (count_f * self.visual.dots.slot_width).min(frame.w);
        let slot_width = total_width / count_f;
        let dot_height = self.visual.dots.height.min(frame.h).min(
            (slot_width * self.visual.dots.height_slot_ratio).max(self.visual.dots.min_height),
        );
        let bottom_inset = self
            .visual
            .dots
            .bottom_inset
            .min((frame.h - dot_height).max(0.0));
        let dot_y = frame.y + frame.h - bottom_inset - dot_height;
        let hit_height = self.visual.dots.hit_height.min(frame.h);
        let hit_y = (dot_y + dot_height * self.visual.dots.center_ratio
            - hit_height * self.visual.dots.center_ratio)
            .clamp(frame.y, frame.y + frame.h - hit_height);
        DotStrip {
            hit_rect: Rect::new(
                frame.x + (frame.w - total_width) * self.visual.dots.center_ratio,
                hit_y,
                total_width,
                hit_height,
            ),
            slot_width,
            dot_y,
            dot_height,
        }
    }

    /// 创建显示指示点和默认箭头的空轮播组件。
    pub fn new() -> Self {
        let visual = CAROUSEL_VISUAL_REF;
        Self {
            children: WidgetChildren::new(),
            runtime: Rc::new(CarouselRuntime::new(&visual.motion)),
            show_dots: visual.defaults.show_dots,
            show_dots_authored: false,
            show_arrows: visual.defaults.show_arrows,
            show_arrows_authored: false,
            autoplay: None,
            autoplay_timer_id: 0xCAFE_0001,
            effect: visual.defaults.effect,
            effect_authored: false,
            pause_on_hover: false,
            custom_arrows: false,
            custom_arrows_view: None,
            hovered: false,
            fixed_width: None,
            fixed_height: None,
            focused: false,
            last_frame: Cell::new(None),
            visual,
        }
    }

    /// 设置是否显示底部页码指示点。
    pub fn show_dots(mut self, v: bool) -> Self {
        self.show_dots = v;
        self.show_dots_authored = true;
        self
    }

    /// 设置是否显示上一页和下一页箭头。
    pub fn show_arrows(mut self, v: bool) -> Self {
        self.show_arrows = v;
        self.show_arrows_authored = true;
        self
    }

    /// 按指定间隔自动切换；组件仍只在有多个子项时申请工作。
    pub fn autoplay(mut self, interval: Duration) -> Self {
        self.autoplay = (interval > Duration::ZERO).then_some(interval);
        self
    }

    /// 设置幻灯片切换效果。
    pub fn effect(mut self, effect: CarouselEffect) -> Self {
        self.effect = effect;
        self.effect_authored = true;
        self.runtime.effect.set(effect);
        self
    }

    /// 设置指针悬停时是否暂停自动播放。
    pub fn pause_on_hover(mut self, pause: bool) -> Self {
        self.pause_on_hover = pause;
        self
    }

    /// 替换默认箭头视图。箭头行为仍由 Carousel 托管。
    pub fn arrows<F, V>(mut self, factory: F) -> Self
    where
        F: Fn(Rc<dyn Fn()>, Rc<dyn Fn()>) -> V + 'static,
        V: crate::ui::view::View,
    {
        self.custom_arrows = true;
        self.custom_arrows_view = Some(Rc::new(move |previous, next| {
            crate::ui::view::View::build(factory(previous, next))
        }));
        self
    }

    // 在已验证 Carousel owner 的树私有捕获边界内构建当前自定义箭头。
    pub(crate) fn custom_arrows_view_for_reconcile(
        // 借用由所属 WidgetTree 为当前 owner 签发的不可替换捕获能力。
        &self,
        // 接收固定树 store、owner 身份与 receipt 事务。
        capture_context: &crate::ui::adapter::DynamicViewCaptureContext,
        // 返回启用且拥有工厂时的完整动态箭头声明。
    ) -> Option<crate::ui::view::ViewNode> {
        // 默认箭头或关闭状态不执行应用自定义工厂。
        if !self.has_custom_arrows() {
            // 缺少动态声明让父协调器移除旧固定箭头。
            return None;
        }
        // 自定义标志有效时必须同时存在可调用工厂。
        let factory = self.custom_arrows_view.as_ref()?;
        // 克隆当前 live 运行态供 previous 窄动作长期持有。
        let runtime = self.runtime.clone();
        // 建立不暴露 Carousel 实现的上一页动作。
        let previous: CarouselArrowAction = Rc::new(move || {
            // 每次调用都从 live 运行态计算循环索引。
            let index = runtime.previous_index();
            // 通过 Carousel 自身选择策略建立用户操作事实。
            runtime.select(index, SelectionSource::User);
        });
        // 克隆同一 live 运行态供 next 窄动作持有。
        let runtime = self.runtime.clone();
        // 建立不泄漏 owner 生命周期的下一页动作。
        let next: CarouselArrowAction = Rc::new(move || {
            // 每次调用都使用当前真实幻灯片数量计算索引。
            let index = runtime.next_index();
            // 仍由 Carousel 拥有选择、暂停与动画策略。
            runtime.select(index, SelectionSource::User);
        });
        // 在固定 owner、槽位与产品身份内原子捕获全部运行时输出。
        let root = capture_context.capture(
            // 隔离 Carousel 箭头与同一 owner 未来可能拥有的其他动态槽位。
            "carousel-custom-arrows",
            // 同一稳定值同时限定状态命名空间与 keyed reconcile。
            Self::CUSTOM_ARROWS_CHILD_KEY,
            // 工厂只在捕获上下文安装后执行一次。
            || factory(previous, next),
        );
        // 应用工厂不得另行声明根身份，否则会形成双重 owner 语义。
        assert!(
            // 仅接受尚未设置 key 的箭头根。
            root.key.is_none(),
            // 提供稳定错误以便调用方移除冲突身份。
            "Carousel arrows 工厂返回根不得设置 key"
        );
        // 克隆当前运行态供语义事件投影闭包持有。
        let runtime = self.runtime.clone();
        // 由框架注入唯一固定 key，并保留既有 Change 事件投影语义。
        Some(
            // 根 key 与私有状态命名空间严格一致。
            root.key(Self::CUSTOM_ARROWS_CHILD_KEY)
                // 自定义箭头的 Click 可投影刚完成的轮播选择变化。
                .on_semantic(SemanticKind::Click, move |event| {
                    // 只有 previous/next 真正改变索引时才投影 Change。
                    if let Some(index) = runtime.pending_change.take() {
                        // 把箭头 Click 转换为 Carousel 的稳定 Change 语义。
                        event.kind = SemanticKind::Change;
                        // 让事件目标保持当前动态箭头根。
                        event.target = event.current_target;
                        // 沿用原公开契约输出目标索引文本。
                        event.payload = SemanticPayload::Text(index.to_string());
                    }
                }),
        )
    }

    // 报告当前声明是否确实需要树级自定义箭头实例。
    pub(crate) fn needs_custom_arrows_view(&self) -> bool {
        // 复用产品开关、custom 标志与工厂存在性的单一真相。
        self.has_custom_arrows()
    }

    /// 设置轮播视口的首选宽高；负值归一化为零。
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.fixed_width = Some(width.max(0.0));
        self.fixed_height = Some(height.max(0.0));
        self
    }

    /// 返回已按当前幻灯片数量归一化的活动索引。
    pub fn current_index(&self) -> usize {
        self.runtime.current.get()
    }

    /// 返回当前参与轮播的幻灯片数量。
    pub fn slide_count(&self) -> usize {
        self.runtime.child_count.get()
    }

    /// 经由同目录 UIX 根声明构建轮播器及其拥有型幻灯片子树。
    #[doc(hidden)]
    pub fn build_view_with_children(self, children: Vec<ViewNode>) -> ViewNode {
        // UIX 拥有公开根；Rust 内核继续独占计时器、选择、动画与绘制机制。
        let kernel = self;
        crate::uix!("src/ui/widgets/display/carousel/carousel.uix")
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        next.runtime.motion.set(&next.visual.motion);
        next.runtime.copy_state_from(&self.runtime, next.effect);
        self.show_dots = next.show_dots;
        self.show_dots_authored = next.show_dots_authored;
        self.show_arrows = next.show_arrows;
        self.show_arrows_authored = next.show_arrows_authored;
        self.autoplay = next.autoplay;
        self.effect = next.effect;
        self.effect_authored = next.effect_authored;
        self.pause_on_hover = next.pause_on_hover;
        self.custom_arrows = next.custom_arrows;
        self.custom_arrows_view = next.custom_arrows_view;
        self.runtime = next.runtime;
        self.fixed_width = next.fixed_width;
        self.fixed_height = next.fixed_height;
        self.visual = next.visual;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Carousel {
            show_dots: self.show_dots,
            show_arrows: self.show_arrows,
            fixed_width: self.fixed_width,
            fixed_height: self.fixed_height,
            current: self.runtime.current.get(),
            slide_count: self.runtime.child_count.get(),
        }
    }

    // 测试目标观察 UIX 声明的关键视觉契约，不暴露到公开 API。
    #[cfg(test)]
    fn visual_contract_for_test(&self) -> (f32, f32, f32, f32, f32, f32, f32) {
        (
            self.visual.defaults.width,
            self.visual.defaults.height,
            self.visual.arrow.hit_width,
            self.visual.arrow.icon_size,
            self.visual.dots.slot_width,
            self.visual.dots.height,
            self.visual.motion.fade_duration_secs,
        )
    }

    // 测试目标确认实例共享同一份 UIX 视觉表。
    #[cfg(test)]
    fn shares_visual_with_for_test(&self, other: &Self) -> bool {
        std::ptr::eq(self.visual, other.visual)
    }

    fn has_custom_arrows(&self) -> bool {
        self.show_arrows && self.custom_arrows && self.custom_arrows_view.is_some()
    }

    fn select(&self, index: usize, source: SelectionSource) {
        self.runtime.select(index, source);
    }

    fn previous_index(&self) -> usize {
        self.runtime.previous_index()
    }

    fn next_index(&self) -> usize {
        self.runtime.next_index()
    }

    fn dot_index_at(&self, frame: Rect, pos: Point) -> Option<usize> {
        let count = self.runtime.child_count.get();
        if !self.show_dots || count <= 1 || !frame.contains(pos) {
            return None;
        }
        let strip = self.dot_strip(frame, count);
        if strip.slot_width <= 0.0
            || pos.x < strip.hit_rect.x
            || pos.x >= strip.hit_rect.x + strip.hit_rect.w
            || pos.y < strip.hit_rect.y
            || pos.y >= strip.hit_rect.y + strip.hit_rect.h
        {
            return None;
        }
        let index = ((pos.x - strip.hit_rect.x) / strip.slot_width) as usize;
        (index < count).then_some(index)
    }
}

// 挂载 Carousel 自定义箭头的树级状态与生命周期行为门禁。
#[cfg(test)]
// 将大体量动态捕获测试拆到独立文件，保持产品组件低于规模上限。
#[path = "../../../../../tests/unit/ui/widgets/display/carousel_dynamic_capture_tests.rs"]
// 仅在测试构建中编译自定义箭头 owner 回归用例。
mod carousel_dynamic_capture_tests;
