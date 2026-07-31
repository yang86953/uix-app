//! Carousel widget for switching between child content.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintPass;
use crate::ui::component::paint_context::PaintContext;

use crate::ui::children::WidgetChildren;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent, WidgetComponent,
    WidgetTree,
};
use crate::ui::{SemanticKind, SemanticPayload, SnapshotFields};

const ARROW_HIT_WIDTH: f32 = 30.0;
const DOT_SLOT_WIDTH: f32 = 18.0;
const DOT_HIT_HEIGHT: f32 = 16.0;
const DOT_HEIGHT: f32 = 6.0;
const DOT_BOTTOM_INSET: f32 = 8.0;
const ARROW_ICON_SIZE: f32 = 16.0;
const FADE_DURATION_SECS: f32 = 0.3;

/// 轮播切换效果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarouselEffect {
    Slide,
    Fade,
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
}

impl CarouselRuntime {
    fn new() -> Self {
        Self {
            current: Cell::new(0),
            child_count: Cell::new(0),
            pending_change: Cell::new(None),
            layout_requested: Cell::new(false),
            manually_paused: Cell::new(false),
            effect: Cell::new(CarouselEffect::Slide),
            fade_from: Cell::new(None),
            fade_progress: Cell::new(1.0),
            fade_dirty: Cell::new(false),
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
            self.fade_progress.set(1.0);
            self.fade_dirty.set(false);
        }
    }

    fn set_child_count(&self, count: usize) {
        self.child_count.set(count);
        let current = if count == 0 {
            0
        } else {
            self.current.get().min(count - 1)
        };
        self.current.set(current);
        if self.fade_from.get().is_some_and(|index| index >= count) {
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
            self.fade_progress.set(0.0);
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
        if self.effect.get() == CarouselEffect::Fade && self.fade_progress.get() < 0.5 {
            self.fade_from.get().unwrap_or_else(|| self.current.get())
        } else {
            self.current.get()
        }
    }

    fn fade_overlay_opacity(&self) -> f32 {
        if self.effect.get() != CarouselEffect::Fade || self.fade_from.get().is_none() {
            return 0.0;
        }
        let progress = self.fade_progress.get().clamp(0.0, 1.0);
        if progress < 0.5 {
            progress * 2.0
        } else {
            (1.0 - progress) * 2.0
        }
    }

    fn finish_fade(&self) {
        self.fade_from.set(None);
        self.fade_progress.set(1.0);
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

component! {
    /// Displays one child at a time with dot and arrow controls.
    pub struct Carousel {
        children: WidgetChildren,
        #[snapshot(skip)]
        runtime: Rc<CarouselRuntime>,
        show_dots: bool,
        show_arrows: bool,
        autoplay: Option<Duration>,
        autoplay_timer_id: u32,
        effect: CarouselEffect,
        pause_on_hover: bool,
        custom_arrows: bool,
        #[snapshot(skip)]
        custom_arrows_view: Option<Rc<CarouselArrowFactory>>,
        hovered: bool,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
        focused: bool,
        last_frame: Cell<Option<Rect>>,
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
        if self.fixed_width.is_some() || self.fixed_height.is_some() { 0.0 } else { 1.0 }
    }

    build => (&self) -> Vec<Box<dyn WidgetComponent>> {
        let children = self.children.take();
        if !children.is_empty() {
            self.runtime.set_child_count(children.len());
        }
        children
    }

    build_view_children => (&self) -> Vec<crate::ui::view::ViewNode> {
        if !self.has_custom_arrows() {
            return Vec::new();
        }
        let Some(factory) = self.custom_arrows_view.as_ref() else {
            return Vec::new();
        };
        let runtime = self.runtime.clone();
        let previous: CarouselArrowAction = Rc::new(move || {
            let index = runtime.previous_index();
            runtime.select(index, SelectionSource::User);
        });
        let runtime = self.runtime.clone();
        let next: CarouselArrowAction = Rc::new(move || {
            let index = runtime.next_index();
            runtime.select(index, SelectionSource::User);
        });
        let runtime = self.runtime.clone();
        vec![factory(previous, next)
            .key("carousel-custom-arrows")
            .on_semantic(SemanticKind::Click, move |event| {
                if let Some(index) = runtime.pending_change.take() {
                    event.kind = SemanticKind::Change;
                    event.target = event.current_target;
                    event.payload = SemanticPayload::Text(index.to_string());
                }
            })]
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
                        let (left, right) = Self::arrow_frames(frame);
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

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
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
        let delta = if dt.is_finite() && dt > 0.0 {
            (dt as f32 / FADE_DURATION_SECS).max(0.0)
        } else {
            0.0
        };
        let previous = self.runtime.fade_progress.get();
        let progress = (previous + delta).min(1.0);
        if progress != previous {
            self.runtime.fade_progress.set(progress);
            self.runtime.fade_dirty.set(true);
        }
        if previous < 0.5 && progress >= 0.5 {
            self.runtime.layout_requested.set(true);
        }
        if progress >= 1.0 {
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
        let bg = ctx.tokens().color_bg_container();
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
        let primary = ctx.tokens().color_primary();
        let dot_color = ctx.tokens().color_text_quaternary();
        ctx.push_clip(frame);

        let fade_opacity = self.runtime.fade_overlay_opacity();
        if fade_opacity > 0.0 {
            let alpha = (bg.a as f32 * fade_opacity)
                .round()
                .clamp(0.0, 255.0) as u8;
            ctx.fill_rect(frame, bg.with_alpha(alpha), None);
        }

        if self.show_arrows && !self.has_custom_arrows() && count > 1 {
            let (left, right) = Self::arrow_frames(frame);
            let icon_size = ARROW_ICON_SIZE
                .min(left.w * 0.6)
                .min(frame.h * 0.6);
            if icon_size >= 1.0 {
                crate::ui::widgets::icon::Icon::paint_in_frame(
                    ctx,
                    "chevron-left",
                    left,
                    ctx.tokens().color_text(),
                    icon_size,
                );
                crate::ui::widgets::icon::Icon::paint_in_frame(
                    ctx,
                    "chevron-right",
                    right,
                    ctx.tokens().color_text(),
                    icon_size,
                );
            }
        }

        if self.show_dots && count > 1 {
            let strip = Self::dot_strip(frame, count);
            for i in 0..count {
                let is_active = i == idx;
                let preferred_width: f32 = if is_active { 16.0 } else { 8.0 };
                let fill_ratio: f32 = if is_active { 0.88 } else { 0.45 };
                let dot_width = preferred_width.min(strip.slot_width * fill_ratio);
                let dot_rect = Rect::new(
                    strip.hit_rect.x
                        + i as f32 * strip.slot_width
                        + (strip.slot_width - dot_width) * 0.5,
                    strip.dot_y,
                    dot_width,
                    strip.dot_height,
                );
                ctx.fill_rect(
                    dot_rect,
                    if is_active { primary } else { dot_color },
                    Some(crate::draw::Radius::uniform(3.0)),
                );
            }
        }
        if self.focused && tree.keyboard_focus_visible() {
            let inset = 0.75_f32.min(frame.w * 0.5).min(frame.h * 0.5);
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
                    1.5,
                    Some(crate::draw::Radius::uniform(ctx.tokens().border_radius_sm())),
                );
            }
        }
        ctx.pop_clip();
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(ComponentId, Rect)>
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

impl Carousel {
    fn intrinsic_size(&self) -> Size {
        Size::new(300.0, 200.0)
    }

    fn normalized_frame(frame: Rect) -> Rect {
        Rect::new(frame.x, frame.y, frame.w.max(0.0), frame.h.max(0.0))
    }

    fn arrow_frames(frame: Rect) -> (Rect, Rect) {
        let width = ARROW_HIT_WIDTH.min(frame.w * 0.5);
        (
            Rect::new(frame.x, frame.y, width, frame.h),
            Rect::new(frame.x + frame.w - width, frame.y, width, frame.h),
        )
    }

    fn dot_strip(frame: Rect, count: usize) -> DotStrip {
        let count_f = count.max(1) as f32;
        let total_width = (count_f * DOT_SLOT_WIDTH).min(frame.w);
        let slot_width = total_width / count_f;
        let dot_height = DOT_HEIGHT.min(frame.h).min((slot_width * 0.75).max(1.0));
        let bottom_inset = DOT_BOTTOM_INSET.min((frame.h - dot_height).max(0.0));
        let dot_y = frame.y + frame.h - bottom_inset - dot_height;
        let hit_height = DOT_HIT_HEIGHT.min(frame.h);
        let hit_y = (dot_y + dot_height * 0.5 - hit_height * 0.5)
            .clamp(frame.y, frame.y + frame.h - hit_height);
        DotStrip {
            hit_rect: Rect::new(
                frame.x + (frame.w - total_width) * 0.5,
                hit_y,
                total_width,
                hit_height,
            ),
            slot_width,
            dot_y,
            dot_height,
        }
    }

    pub fn new() -> Self {
        Self {
            children: WidgetChildren::new(),
            runtime: Rc::new(CarouselRuntime::new()),
            show_dots: true,
            show_arrows: true,
            autoplay: None,
            autoplay_timer_id: 0xCAFE_0001,
            effect: CarouselEffect::Slide,
            pause_on_hover: false,
            custom_arrows: false,
            custom_arrows_view: None,
            hovered: false,
            fixed_width: None,
            fixed_height: None,
            focused: false,
            last_frame: Cell::new(None),
        }
    }

    pub fn show_dots(mut self, v: bool) -> Self {
        self.show_dots = v;
        self
    }

    pub fn show_arrows(mut self, v: bool) -> Self {
        self.show_arrows = v;
        self
    }

    /// 按指定间隔自动切换；组件仍只在有多个子项时申请工作。
    pub fn autoplay(mut self, interval: Duration) -> Self {
        self.autoplay = (interval > Duration::ZERO).then_some(interval);
        self
    }

    pub fn effect(mut self, effect: CarouselEffect) -> Self {
        self.effect = effect;
        self.runtime.effect.set(effect);
        self
    }

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

    /// Sets the preferred Carousel viewport size.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.fixed_width = Some(width.max(0.0));
        self.fixed_height = Some(height.max(0.0));
        self
    }

    pub fn current_index(&self) -> usize {
        self.runtime.current.get()
    }

    pub fn slide_count(&self) -> usize {
        self.runtime.child_count.get()
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        next.runtime.copy_state_from(&self.runtime, next.effect);
        self.show_dots = next.show_dots;
        self.show_arrows = next.show_arrows;
        self.autoplay = next.autoplay;
        self.effect = next.effect;
        self.pause_on_hover = next.pause_on_hover;
        self.custom_arrows = next.custom_arrows;
        self.custom_arrows_view = next.custom_arrows_view;
        self.runtime = next.runtime;
        self.fixed_width = next.fixed_width;
        self.fixed_height = next.fixed_height;
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
        let strip = Self::dot_strip(frame, count);
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
