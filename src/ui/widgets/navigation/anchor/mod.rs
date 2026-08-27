//! Anchor 锚点组件 — 页面内导航，滚动侦听高亮。
//!
//! 与 ScrollView 配合使用：监听滚动位置，自动高亮当前锚点。

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Color;
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent, WidgetId,
    WidgetTree,
};
use crate::widget;
use std::cell::Cell;
use std::rc::Rc;

mod presentation;

use presentation::*;

widget! {
    /// Anchor — 锚点导航条。
    ///
    /// 传入 LinkItem 列表，点击跳转到对应锚点，滚动时高亮当前锚点。
    pub struct Anchor {
        /// 锚点链接列表
        items: Vec<AnchorItem>,
        /// 当前高亮索引
        active_index: usize,
        /// 每个锚点对应的滚动 Y 位置（由外部注入或 on_update 计算）
        anchor_positions: Vec<f32>,
        /// 容器顶部偏移（Header 高度等）
        offset_top: f32,
        /// 背景色
        bg_color: Option<Color>,
        show_ink: bool,
        bounds: f32,
        container_enabled: bool,
        #[snapshot(skip)]
        container_view: Option<Rc<dyn Fn() -> crate::ui::view::ViewNode>>,
        last_frame: Cell<Option<Rect>>,
        focused: bool,
        pending_change: Cell<Option<usize>>,
        // 同目录 UIX 生成的唯一静态视觉表。
        #[snapshot(skip)]
        visual: &'static AnchorVisual,
    }

    tab_index => (&self) -> i32 { i32::from(!self.items.is_empty()) }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(crate::ui::WidgetId, Rect)>
    {
        if !self.container_enabled {
            return Vec::new();
        }
        let content = self.container_rect(frame);
        children.iter().map(|child| (child.id, content)).collect()
    }

    children_clip => (&self, frame: Rect) -> Option<Rect> {
        Some(if self.container_enabled { self.container_rect(frame) } else { frame })
    }

    build_view_children => (&self) -> Vec<crate::ui::view::ViewNode> {
        // 容器工厂只能在真实 Anchor owner 已注册后由树级动态捕获入口调用。
        Vec::new()
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if !self.local_navigation_rect().contains(*pos) || pos.y < 0.0 {
                    return EventResult::NotHandled;
                }
                let idx = (pos.y / self.visual.layout.row_height) as usize;
                if idx < self.items.len() {
                    self.select(idx, true);
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerEnter => EventResult::Handled,
            SystemEvent::PointerLeave => EventResult::Handled,
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } => match key {
                KeyCode::Down => {
                    self.move_active(true);
                    EventResult::Handled
                }
                KeyCode::Up => {
                    self.move_active(false);
                    EventResult::Handled
                }
                KeyCode::Home => {
                    self.select(0, true);
                    EventResult::Handled
                }
                KeyCode::End if !self.items.is_empty() => {
                    self.select(self.items.len() - 1, true);
                    EventResult::Handled
                }
                KeyCode::Enter | KeyCode::Space if !self.items.is_empty() => {
                    self.pending_change.set(Some(self.active_index));
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        let idx = self.pending_change.take()?;
        let href = self
            .items
            .get(idx)
            .map(|item| item.href.clone())
            .unwrap_or_default();
        Some(SemanticEvent::change(id, href))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let frame = Self::normalized_frame(frame);
        self.last_frame.set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let navigation = self.navigation_rect(frame);
        // 背景
        if let Some(bg) = self.bg_color {
            ctx.fill_rect(frame, bg, None);
        }
        // 导航行、墨线和焦点框共享一次主题解析。
        let visual = self.visual.resolve(ctx.tokens());

        // 分割线
        ctx.stroke_rect(
            Rect::new(
                navigation.x + navigation.w - self.visual.chrome.divider_width,
                navigation.y,
                self.visual.chrome.divider_width,
                navigation.h,
            ),
            visual.border_secondary,
            self.visual.chrome.border_width,
            None,
        );

        ctx.push_clip(navigation);
        for (i, item) in self.items.iter().enumerate() {
            let y = navigation.y + i as f32 * self.visual.layout.row_height;
            let is_active = i == self.active_index;
            let color = if is_active {
                visual.primary
            } else {
                visual.text_secondary
            };

            // 激活态左侧指示条
            if is_active && self.show_ink {
                ctx.fill_rect(
                    Rect::new(
                        navigation.x,
                        y,
                        self.visual.layout.indicator_width,
                        self.visual.layout.row_height,
                    ),
                    visual.primary,
                    None,
                );
            }

            let label_x = navigation.x + self.visual.layout.label_x;
            let row_rect = Rect::new(
                navigation.x,
                y,
                navigation.w,
                self.visual.layout.row_height,
            );
            let label_y = ctx.visual_center_y(row_rect, visual.font_size);
            ctx.draw_text(
                &item.label,
                Point::new(label_x, label_y),
                color,
                visual.font_size,
            );
        }
        ctx.pop_clip();

        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(
                navigation,
                visual.primary,
                self.visual.chrome.focus_width,
                Some(crate::draw::Radius::uniform(visual.radius)),
            );
        }
    }
}

/// 锚点项
#[derive(Debug, Clone, PartialEq)]
pub struct AnchorItem {
    /// 显示文本
    pub label: String,
    /// 锚点标识（对应目标 widget 的 ID 或 key）
    pub href: String,
}

impl AnchorItem {
    /// 创建显示文本和目标标识组成的锚点项。
    pub fn new(label: impl Into<String>, href: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            href: href.into(),
        }
    }
}

impl Anchor {
    // 固定动态容器运行时 key，使子树身份与捕获命名空间严格一致。
    pub(crate) const CONTAINER_CHILD_KEY: &'static str = "uix:anchor:container";

    fn intrinsic_size(&self) -> Size {
        let navigation = self.intrinsic_navigation_size();
        if self.container_enabled {
            Size::new(
                navigation.w + self.visual.layout.container_width,
                navigation.h.max(self.visual.layout.container_min_height),
            )
        } else {
            navigation
        }
    }

    fn intrinsic_navigation_size(&self) -> Size {
        let w = self
            .items
            .iter()
            .map(|i| {
                i.label.chars().count() as f32 * self.visual.layout.label_char_width
                    + self.visual.layout.label_horizontal_space
            })
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(self.visual.layout.navigation_min_width)
            .max(self.visual.layout.navigation_min_width);
        Size::new(w, self.items.len() as f32 * self.visual.layout.row_height)
    }

    /// 创建默认显示墨线、激活首项且偏移为零的锚点导航。
    pub fn new(items: Vec<AnchorItem>) -> Self {
        let count = items.len();
        let visual = ANCHOR_VISUAL_REF;
        Self {
            items,
            active_index: 0,
            anchor_positions: vec![0.0; count],
            offset_top: 0.0,
            bg_color: None,
            show_ink: visual.defaults.show_ink,
            bounds: visual.defaults.bounds,
            container_enabled: false,
            container_view: None,
            last_frame: Cell::new(None),
            focused: false,
            pending_change: Cell::new(None),
            visual,
        }
    }

    /// 设置锚点 Y 位置列表（由外部根据内容布局计算后注入）
    pub fn set_positions(&mut self, positions: Vec<f32>) {
        self.anchor_positions = positions;
    }

    /// 根据当前滚动 Y 更新高亮
    pub fn update_active(&mut self, scroll_y: f32) {
        let mut idx = self.items.len().saturating_sub(1);
        for (i, &pos) in self.anchor_positions.iter().enumerate() {
            if scroll_y < pos - self.offset_top - self.bounds {
                idx = i.saturating_sub(1);
                break;
            }
        }
        self.active_index = idx;
    }

    /// 设置滚动定位时从锚点位置扣除的顶部偏移。
    pub fn set_offset_top(mut self, v: f32) -> Self {
        self.offset_top = v;
        self
    }

    /// 设置目标顶部偏移，是 [`Self::set_offset_top`] 的别名。
    pub fn target_offset(self, offset: f32) -> Self {
        self.set_offset_top(offset)
    }

    /// 设置是否显示当前锚点墨线。
    pub fn show_ink(mut self, show: bool) -> Self {
        self.show_ink = show;
        self
    }

    /// 设置滚动激活边界；负值截断为零，非有限值恢复为十。
    pub fn bounds(mut self, bounds: f32) -> Self {
        self.bounds = if bounds.is_finite() {
            bounds.max(0.0)
        } else {
            self.visual.defaults.bounds
        };
        self
    }

    /// 设置与锚点导航共同布局的动态内容容器工厂。
    pub fn container<F, V>(mut self, factory: F) -> Self
    where
        F: Fn() -> V + 'static,
        V: crate::ui::view::View,
    {
        self.container_enabled = true;
        self.container_view = Some(Rc::new(move || crate::ui::view::View::build(factory())));
        self
    }

    // 在已验证 Anchor owner 的树私有捕获边界内构建当前容器声明根。
    pub(crate) fn container_view_for_reconcile(
        // 借用树签发且 owner 不可替换的动态捕获能力。
        &self,
        // 接收仅属于当前活跃 Anchor 的动态捕获能力。
        capture_context: &crate::ui::adapter::DynamicViewCaptureContext,
        // 返回启用且具备工厂时的完整捕获输出。
    ) -> Option<crate::ui::view::ViewNode> {
        // 关闭容器时由父级协调器移除既有固定动态子树。
        if !self.container_enabled {
            // 不执行应用工厂，避免禁用状态产生新的私有资源。
            return None;
        }
        // 缺少工厂时保持无动态容器语义。
        let factory = self.container_view.as_ref()?;
        // 在固定 owner、槽位和业务 key 中捕获完整状态与生命周期输出。
        let root = capture_context.capture(
            // 隔离 Anchor 容器工厂与同一 owner 的其他动态入口。
            "anchor-container",
            // 命名空间与运行时根 key 使用同一稳定身份。
            Self::CONTAINER_CHILD_KEY,
            // 工厂只在捕获上下文安装后执行一次。
            || factory(),
        );
        // 用户不得占用动态容器根 key，否则 keyed 协调会把 authored 身份误作框架子树。
        assert!(
            // 仅接受未设置 key 的工厂根，框架随后注入唯一固定 key。
            root.key.is_none(),
            // 给出稳定的调用方诊断，避免静默覆盖用户 key。
            "Anchor container 工厂返回根不得设置 key"
        );
        // 由框架唯一写入稳定根 key，确保物化、reconcile 与状态命名空间相同。
        Some(root.key(Self::CONTAINER_CHILD_KEY))
    }

    // 报告当前声明是否确实需要一个动态容器，供树在执行用户工厂前决定生命周期分支。
    pub(crate) fn needs_container_view(&self) -> bool {
        // 开关与工厂必须同时存在，缺少任一项都表示应移除旧框架容器。
        self.container_enabled && self.container_view.is_some()
    }

    /// 设置锚点导航背景颜色。
    pub fn bg(mut self, c: Color) -> Self {
        self.bg_color = Some(c);
        self
    }
    /// 返回当前高亮锚点索引。
    pub fn active_index(&self) -> usize {
        self.active_index
    }
    /// 返回当前高亮锚点标识；没有有效条目时返回空字符串。
    pub fn active_href(&self) -> &str {
        self.items
            .get(self.active_index)
            .map(|i| i.href.as_str())
            .unwrap_or("")
    }
    /// 返回全部锚点项。
    pub fn items(&self) -> &[AnchorItem] {
        &self.items
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        // UIX 视觉随下一声明更新。
        self.visual = next.visual;
        // 保存旧运行节点的索引，供无稳定 href 的兼容条目回退。
        let previous_active_index = self.active_index;
        // 保存旧激活项的非空 href，声明重排时按稳定身份保持选择。
        let previous_active_href = self
            // 从组件拥有的当前索引读取条目。
            .items
            // 安全处理空集合或越界旧状态。
            .get(previous_active_index)
            // 借用稳定 href。
            .map(|item| item.href.as_str())
            // 空 href 不冒充稳定身份。
            .filter(|href| !href.is_empty())
            // 在替换声明条目前取得拥有型副本。
            .map(str::to_owned);
        // 判断新声明是否仍对应同一组有序滚动目标。
        let href_sequence_changed = self.items.len() != next.items.len()
            // 同长度时逐项核对位置缓存所绑定的 href。
            || self
                // 借用旧条目序列。
                .items
                // 遍历旧条目。
                .iter()
                // 与新声明条目逐项配对。
                .zip(next.items.iter())
                // 任一 href 改变或重排都使位置缓存失效。
                .any(|(previous, current)| previous.href != current.href);
        // 判断调用方注入的位置数量是否仍覆盖全部条目。
        let position_count_changed = self.anchor_positions.len() != next.items.len();
        // 采用新声明的条目集合。
        self.items = next.items;
        // 采用新声明的顶部偏移。
        self.offset_top = next.offset_top;
        // 采用新声明的背景色。
        self.bg_color = next.bg_color;
        // 采用新声明的墨球指示器配置。
        self.show_ink = next.show_ink;
        // 采用新声明的激活边界配置。
        self.bounds = next.bounds;
        // 采用新声明的内容容器开关。
        self.container_enabled = next.container_enabled;
        // 采用新声明的内容工厂。
        self.container_view = next.container_view;
        // 优先按稳定 href 查找同一当前项。
        self.active_index = previous_active_href
            // 只有旧条目提供稳定身份时才搜索。
            .and_then(|href| {
                // 在新声明中定位同一滚动目标。
                self.items.iter().position(|item| item.href == href)
            })
            // 目标消失或旧条目无 href 时兼容沿用归一化旧索引。
            .unwrap_or_else(|| previous_active_index.min(self.items.len().saturating_sub(1)));
        // href 顺序或位置数量变化时必须丢弃已失配的测量缓存。
        if href_sequence_changed || position_count_changed {
            // 等待调用方按新目标顺序重新注入真实位置。
            self.anchor_positions = vec![0.0; self.items.len()];
        }
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Anchor {
            items: self.items.clone(),
            active_index: self.active_index,
            offset_top: self.offset_top,
            bg_color: self.bg_color,
        }
    }

    fn select(&mut self, index: usize, emit: bool) {
        if self.items.is_empty() {
            return;
        }
        let index = index.min(self.items.len() - 1);
        let changed = index != self.active_index;
        self.active_index = index;
        if emit && changed {
            self.pending_change.set(Some(index));
        }
    }

    fn move_active(&mut self, forward: bool) {
        if self.items.is_empty() {
            return;
        }
        let next = if forward {
            (self.active_index + 1).min(self.items.len() - 1)
        } else {
            self.active_index.saturating_sub(1)
        };
        self.select(next, true);
    }

    fn navigation_rect(&self, frame: Rect) -> Rect {
        let width = self.intrinsic_navigation_size().w.min(frame.w).max(0.0);
        Rect::new(frame.x, frame.y, width, frame.h)
    }

    fn container_rect(&self, frame: Rect) -> Rect {
        let navigation = self.navigation_rect(frame);
        Rect::new(
            navigation.x + navigation.w,
            frame.y,
            (frame.w - navigation.w).max(0.0),
            frame.h,
        )
    }

    fn local_navigation_rect(&self) -> Rect {
        self.navigation_rect(self.last_frame.get().unwrap_or_else(|| {
            let size = self.intrinsic_size();
            Rect::new(0.0, 0.0, size.w, size.h)
        }))
    }

    fn normalized_frame(frame: Rect) -> Rect {
        Rect::new(
            frame.x,
            frame.y,
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
}

// UIX 根把声明视觉注入 Rust 导航内核。
fn build_anchor_view(mut kernel: Anchor, visual: &'static AnchorVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Anchor {
    fn build(self) -> ViewNode {
        build_anchor_view(self, ANCHOR_VISUAL_REF)
    }
}
