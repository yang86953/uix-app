//! Anchor 锚点组件 — 页面内导航，滚动侦听高亮。
//!
//! 与 ScrollView 配合使用：监听滚动位置，自动高亮当前锚点。

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Color;
use crate::ui::component::paint_context::PaintContext;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent,
    WidgetTree,
};
use std::cell::Cell;
use std::rc::Rc;

component! {
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
    }

    tab_index => (&self) -> i32 { i32::from(!self.items.is_empty()) }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(crate::ui::ComponentId, Rect)>
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
                let idx = (pos.y / 36.0) as usize;
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

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
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
        let primary = ctx.tokens().color_primary();
        let _text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let border_color = ctx.tokens().color_border_secondary();

        // 分割线
        ctx.stroke_rect(
            Rect::new(navigation.x + navigation.w - 1.0, navigation.y, 1.0, navigation.h),
            border_color, 1.0, None,
        );

        ctx.push_clip(navigation);
        for (i, item) in self.items.iter().enumerate() {
            let y = navigation.y + i as f32 * 36.0;
            let is_active = i == self.active_index;
            let color = if is_active { primary } else { text_secondary };

            // 激活态左侧指示条
            if is_active && self.show_ink {
                ctx.fill_rect(Rect::new(navigation.x, y, 3.0, 36.0), primary, None);
            }

            let label_x = navigation.x + 16.0;
            let row_rect = Rect::new(navigation.x, y, navigation.w, 36.0);
            let label_y = ctx.visual_center_y(row_rect, 14.0);
            ctx.draw_text(&item.label, Point::new(label_x, label_y), color, 14.0);
        }
        ctx.pop_clip();

        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(
                navigation,
                primary,
                1.5,
                Some(crate::draw::Radius::uniform(
                    ctx.tokens().border_radius_sm(),
                )),
            );
        }
    }
}

/// 锚点项
#[derive(Debug, Clone, PartialEq)]
pub struct AnchorItem {
    /// 显示文本
    pub label: String,
    /// 锚点标识（对应目标 component 的 ID 或 key）
    pub href: String,
}

impl AnchorItem {
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
            Size::new(navigation.w + 320.0, navigation.h.max(240.0))
        } else {
            navigation
        }
    }

    fn intrinsic_navigation_size(&self) -> Size {
        let w = self
            .items
            .iter()
            .map(|i| i.label.chars().count() as f32 * 14.0 + 32.0)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(120.0)
            .max(120.0);
        Size::new(w, self.items.len() as f32 * 36.0)
    }

    pub fn new(items: Vec<AnchorItem>) -> Self {
        let count = items.len();
        Self {
            items,
            active_index: 0,
            anchor_positions: vec![0.0; count],
            offset_top: 0.0,
            bg_color: None,
            show_ink: true,
            bounds: 10.0,
            container_enabled: false,
            container_view: None,
            last_frame: Cell::new(None),
            focused: false,
            pending_change: Cell::new(None),
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

    pub fn set_offset_top(mut self, v: f32) -> Self {
        self.offset_top = v;
        self
    }

    pub fn target_offset(self, offset: f32) -> Self {
        self.set_offset_top(offset)
    }

    pub fn show_ink(mut self, show: bool) -> Self {
        self.show_ink = show;
        self
    }

    pub fn bounds(mut self, bounds: f32) -> Self {
        self.bounds = if bounds.is_finite() {
            bounds.max(0.0)
        } else {
            10.0
        };
        self
    }

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

    pub fn bg(mut self, c: Color) -> Self {
        self.bg_color = Some(c);
        self
    }
    pub fn active_index(&self) -> usize {
        self.active_index
    }
    pub fn active_href(&self) -> &str {
        self.items
            .get(self.active_index)
            .map(|i| i.href.as_str())
            .unwrap_or("")
    }
    pub fn items(&self) -> &[AnchorItem] {
        &self.items
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
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

// 挂载 Anchor 动态容器的树级状态与生命周期行为门禁。
#[cfg(test)]
// 将大体量行为测试拆到独立文件，保持产品代码文件低于规模上限。
#[path = "anchor_dynamic_capture_tests.rs"]
// 仅在测试构建中编译动态捕获回归用例。
mod anchor_dynamic_capture_tests;

// 集中验证 Anchor reconcile 的稳定 href 与位置缓存生命周期。
#[cfg(test)]
mod tests {
    // 引入被测公开条目与组件。
    use super::{Anchor, AnchorItem};

    // 验证同长度重排按 href 保持选择并使旧位置缓存失效。
    #[test]
    // 声明 Anchor 重排回归测试。
    fn reconcile_preserves_active_href_and_invalidates_reordered_positions() {
        // 构造两个具有稳定 href 的锚点。
        let mut anchor = Anchor::new(vec![
            // 登记首个滚动目标。
            AnchorItem::new("概览", "overview"),
            // 登记次个滚动目标。
            AnchorItem::new("设置", "settings"),
        ]);
        // 注入与旧声明顺序对应的位置缓存。
        anchor.set_positions(vec![0.0, 100.0]);
        // 模拟滚动到第二个目标之后。
        anchor.update_active(200.0);
        // 前置条件必须选中 settings。
        assert_eq!(anchor.active_href(), "settings");

        // 构造 href 顺序颠倒的新声明树。
        let next = Anchor::new(vec![
            // 把当前目标移动到首位。
            AnchorItem::new("设置", "settings"),
            // 把原首项目标移动到末位。
            AnchorItem::new("概览", "overview"),
        ]);
        // 执行声明树 reconcile。
        anchor.sync_from(next);

        // 当前项必须跟随稳定 href 到新索引。
        assert_eq!(anchor.active_index(), 0);
        // 当前 href 不得因旧索引而串到 overview。
        assert_eq!(anchor.active_href(), "settings");
        // 旧位置顺序已失配，必须等待调用方重新注入。
        assert_eq!(anchor.anchor_positions, vec![0.0, 0.0]);
    }

    // 验证同一 href 序列保留位置缓存且空集合保持安全。
    #[test]
    // 声明 Anchor 稳定序列与空集合回归测试。
    fn reconcile_keeps_matching_positions_and_normalizes_empty_items() {
        // 构造稳定的两项目标序列。
        let mut anchor = Anchor::new(vec![
            // 登记首个目标。
            AnchorItem::new("旧概览", "overview"),
            // 登记次个目标。
            AnchorItem::new("旧设置", "settings"),
        ]);
        // 注入调用方测得的位置。
        anchor.set_positions(vec![16.0, 120.0]);
        // 激活第二个滚动目标。
        anchor.update_active(240.0);

        // 仅更新显示标题并保持 href 序列。
        let renamed = Anchor::new(vec![
            // 首项目标身份不变。
            AnchorItem::new("新概览", "overview"),
            // 次项目标身份不变。
            AnchorItem::new("新设置", "settings"),
        ]);
        // 执行仅文案变化的 reconcile。
        anchor.sync_from(renamed);

        // 稳定 href 仍保持第二项选择。
        assert_eq!(anchor.active_href(), "settings");
        // 相同目标序列继续复用有效位置缓存。
        assert_eq!(anchor.anchor_positions, vec![16.0, 120.0]);

        // 用空声明树移除全部目标。
        anchor.sync_from(Anchor::new(Vec::new()));
        // 空集合的公开索引保持安全零值。
        assert_eq!(anchor.active_index(), 0);
        // 空集合没有伪造 href。
        assert_eq!(anchor.active_href(), "");
        // 空集合同步清空位置缓存。
        assert!(anchor.anchor_positions.is_empty());
    }
}
