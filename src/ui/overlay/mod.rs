use crate::core::ComponentId;
use crate::core::{Point, Rect};

/// overlay backdrop 模糊使用的逻辑区域来源。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OverlayBackdropRegion {
    /// 跟随 overlay 的遮罩/命中边界。
    MaskBounds,
    /// 使用调用方给定的独立逻辑区域。
    Logical(Rect),
}

/// UI 层尚未解析 Theme 默认值的 backdrop 模糊请求。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlayBackdropBlur {
    /// `None` 表示使用当前 Theme token。
    radius: Option<f32>,
    /// 保存 mask 跟随或独立逻辑区域策略。
    region: OverlayBackdropRegion,
}

impl OverlayBackdropBlur {
    /// 创建使用 Theme 默认半径并跟随 mask 的请求。
    pub const fn theme() -> Self {
        // 默认请求不固化主题值，确保主题切换会重新解析。
        Self {
            // 延迟到 ScenePaint 桥接时读取主题 token。
            radius: None,
            // 默认与遮罩、命中几何保持一致。
            region: OverlayBackdropRegion::MaskBounds,
        }
    }

    /// 创建显式半径并跟随 mask 的请求。
    pub const fn radius(radius: f32) -> Self {
        // 显式值覆盖 Theme token，但仍在解析边界校验。
        Self {
            // 保存调用方给定半径。
            radius: Some(radius),
            // 继续使用默认 mask 区域。
            region: OverlayBackdropRegion::MaskBounds,
        }
    }

    /// 改用独立逻辑区域。
    pub const fn region(mut self, region: Rect) -> Self {
        // Custom 与特殊场景可以脱离 mask 几何。
        self.region = OverlayBackdropRegion::Logical(region);
        // 返回更新后的 typed 请求。
        self
    }

    /// 使用当前 mask 与 Theme token 解析为 draw System 的稳定效果值。
    fn resolve(
        // 读取当前 overlay 的遮罩/命中边界。
        self,
        // 缺失 mask 时不能解析默认区域。
        mask_bounds: Option<Rect>,
        // 读取当前主题默认半径。
        theme_radius: f32,
    ) -> Option<crate::draw::OverlayBackdropEffect> {
        // 显式值优先，否则使用当前 Theme token。
        let radius = self.radius.unwrap_or(theme_radius);
        // 将区域策略解析为唯一逻辑矩形。
        let region = match self.region {
            // 默认区域必须来自真实 mask/命中几何。
            OverlayBackdropRegion::MaskBounds => mask_bounds?,
            // 独立区域直接使用调用方逻辑值。
            OverlayBackdropRegion::Logical(region) => region,
        };
        // 统一由 draw 契约校验 no-op 与非法几何。
        crate::draw::OverlayBackdropEffect::new(region, radius)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayKind {
    Modal,
    Drawer,
    Popover,
    Tooltip,
    ContextMenu,
    Message,
    Notification,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OverlayId(u64);

impl OverlayId {
    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct OverlayEntry {
    id: OverlayId,
    owner: ComponentId,
    kind: OverlayKind,
    bounds: Option<Rect>,
    z_index: i32,
    modal: bool,
    dismiss_on_outside: bool,
    focus_trap: bool,
    managed: bool,
    /// 可选的显式 backdrop blur 请求；默认关闭。
    backdrop_blur: Option<OverlayBackdropBlur>,
}

impl OverlayEntry {
    pub fn new(owner: ComponentId, kind: OverlayKind) -> Self {
        Self {
            id: OverlayId(0),
            owner,
            kind,
            bounds: None,
            z_index: 0,
            modal: matches!(kind, OverlayKind::Modal | OverlayKind::Drawer),
            dismiss_on_outside: matches!(kind, OverlayKind::Modal | OverlayKind::Drawer),
            focus_trap: matches!(kind, OverlayKind::Modal | OverlayKind::Drawer),
            managed: false,
            // 所有 OverlayKind 默认关闭，只有显式 opt-in 才执行效果。
            backdrop_blur: None,
        }
    }

    pub fn bounds(mut self, bounds: Rect) -> Self {
        self.bounds = Some(bounds);
        self
    }

    pub fn z_index(mut self, z_index: i32) -> Self {
        self.z_index = z_index;
        self
    }

    pub fn modal(mut self, modal: bool) -> Self {
        self.modal = modal;
        self
    }

    pub fn dismiss_on_outside(mut self, dismiss: bool) -> Self {
        self.dismiss_on_outside = dismiss;
        self
    }

    pub fn focus_trap(mut self, focus_trap: bool) -> Self {
        self.focus_trap = focus_trap;
        self
    }

    pub fn managed(mut self, managed: bool) -> Self {
        self.managed = managed;
        self
    }

    /// 为任意 OverlayKind 显式启用 typed backdrop blur。
    pub fn backdrop_blur(mut self, blur: OverlayBackdropBlur) -> Self {
        // OverlayEntry 与内置、Custom overlay 使用同一效果入口。
        self.backdrop_blur = Some(blur);
        // 返回更新后的登记。
        self
    }

    pub fn id(&self) -> OverlayId {
        self.id
    }

    pub fn owner(&self) -> ComponentId {
        self.owner
    }

    pub fn kind(&self) -> OverlayKind {
        self.kind
    }

    pub fn bounds_rect(&self) -> Option<Rect> {
        self.bounds
    }

    pub fn z_index_value(&self) -> i32 {
        self.z_index
    }

    pub fn is_modal(&self) -> bool {
        self.modal
    }

    pub fn dismisses_on_outside(&self) -> bool {
        self.dismiss_on_outside
    }

    pub fn traps_focus(&self) -> bool {
        self.focus_trap
    }

    pub fn is_managed(&self) -> bool {
        self.managed
    }

    /// 返回尚未解析 Theme 的 backdrop 请求。
    pub fn backdrop_blur_value(&self) -> Option<OverlayBackdropBlur> {
        // 请求为 Copy 值，不泄漏 OverlayEntry 的可变所有权。
        self.backdrop_blur
    }
}

#[derive(Debug, Default)]
pub struct OverlayStack {
    next_id: u64,
    entries: Vec<OverlayEntry>,
}

impl OverlayStack {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, owner: ComponentId, kind: OverlayKind) -> OverlayId {
        self.push_entry(OverlayEntry::new(owner, kind))
    }

    pub fn push_entry(&mut self, mut entry: OverlayEntry) -> OverlayId {
        self.next_id += 1;
        let id = OverlayId(self.next_id);
        entry.id = id;
        self.entries.push(entry);
        self.entries.sort_by_key(|entry| entry.z_index);
        id
    }

    pub fn remove(&mut self, id: OverlayId) -> Option<OverlayEntry> {
        let index = self.entries.iter().position(|entry| entry.id == id)?;
        Some(self.entries.remove(index))
    }

    pub fn remove_for_owner(&mut self, owner: ComponentId) -> Vec<OverlayEntry> {
        let mut removed = Vec::new();
        self.entries.retain(|entry| {
            if entry.owner == owner {
                removed.push(entry.clone());
                false
            } else {
                true
            }
        });
        removed
    }

    pub fn retain_entries(&mut self, mut keep: impl FnMut(&OverlayEntry) -> bool) {
        self.entries.retain(|entry| keep(entry));
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &OverlayEntry> {
        self.entries.iter()
    }

    pub fn top(&self) -> Option<&OverlayEntry> {
        self.entries.last()
    }

    pub fn hit_test(&self, x: f32, y: f32) -> Option<&OverlayEntry> {
        self.entries.iter().rev().find(|entry| {
            entry
                .bounds
                .is_some_and(|bounds| bounds.contains(Point::new(x, y)))
        })
    }

    /// 将全部显式请求聚合为 ScenePipeline 每帧唯一的效果计划。
    pub(crate) fn backdrop_effect(
        // 借用当前 overlay 栈。
        &self,
        // 使用当前 Theme token 解析未显式覆盖的半径。
        theme_radius: f32,
    ) -> Option<crate::draw::OverlayBackdropEffect> {
        // 逐 overlay 解析，并把多请求压缩为区域并集与最大半径。
        self.entries
            // 只读遍历不会改变 z-index 或命中顺序。
            .iter()
            // 默认关闭的 overlay 不参与效果计划。
            .filter_map(|entry| {
                // 读取该 entry 的显式请求。
                entry
                    // 请求缺失时跳过。
                    .backdrop_blur
                    // 使用当前 mask 与主题解析。
                    .and_then(|blur| blur.resolve(entry.bounds, theme_radius))
            })
            // 多 overlay 每帧只形成一个效果事务。
            .reduce(crate::draw::OverlayBackdropEffect::union)
    }
}

pub(crate) mod placement;
pub use placement::Placement;

// 单元测试锁定全 OverlayKind 的统一 opt-in 与聚合策略。
#[cfg(test)]
mod tests {
    // 引入本模块公开契约。
    use super::*;

    // 验证全部 OverlayKind 共用一个 typed effect 入口，并聚合区域与最大半径。
    #[test]
    fn backdrop_effect_aggregates_all_overlay_kinds() {
        // 枚举产品决策开放的全部 overlay 类型。
        let kinds = [
            // 模态对话框。
            OverlayKind::Modal,
            // 抽屉。
            OverlayKind::Drawer,
            // 气泡卡片。
            OverlayKind::Popover,
            // 提示。
            OverlayKind::Tooltip,
            // 上下文菜单。
            OverlayKind::ContextMenu,
            // 全局消息。
            OverlayKind::Message,
            // 通知。
            OverlayKind::Notification,
            // 自定义浮层。
            OverlayKind::Custom,
        ];
        // 创建空栈并逐项登记显式请求。
        let mut stack = OverlayStack::new();
        // 每个类型使用不同区域与半径，证明没有按 kind 开洞。
        for (index, kind) in kinds.into_iter().enumerate() {
            // 使用稳定组件身份。
            let owner = ComponentId::new(index + 1);
            // 默认请求跟随当前 entry 的 mask bounds。
            let entry = OverlayEntry::new(owner, kind)
                // 让全部区域形成可预测并集。
                .bounds(Rect::new(index as f32, 0.0, 10.0, 10.0))
                // 最大半径应来自最后一个请求。
                .backdrop_blur(OverlayBackdropBlur::radius(index as f32 + 1.0));
            // 使用公开统一入口登记。
            stack.push_entry(entry);
        }
        // 聚合时主题值不会覆盖各 entry 的显式半径。
        let effect = stack
            // 传入不同 Theme 默认值以证明显式值优先。
            .backdrop_effect(3.0)
            // 八个合法请求必须形成计划。
            .expect("all overlay kinds should share backdrop blur");
        // 区域从 x=0 延伸到最后一个 bounds 的右边界 17。
        assert_eq!(effect.region(), Rect::new(0.0, 0.0, 17.0, 10.0));
        // 最大显式半径为八。
        assert_eq!(effect.radius(), 8.0);
    }

    // 验证 Theme 默认、独立区域与 no-op 半径解析。
    #[test]
    fn backdrop_effect_resolves_theme_and_independent_region() {
        // 使用 Custom 证明独立区域不依赖 mask bounds。
        let entry = OverlayEntry::new(ComponentId::new(1), OverlayKind::Custom)
            // 故意不设置 bounds。
            .backdrop_blur(
                // 半径延迟读取 Theme。
                OverlayBackdropBlur::theme()
                    // 独立逻辑区域供 Custom 使用。
                    .region(Rect::new(4.0, 6.0, 20.0, 12.0)),
            );
        // 构造只含一个 Custom 的栈。
        let mut stack = OverlayStack::new();
        // 登记统一 effect 请求。
        stack.push_entry(entry);
        // 主题默认值应进入已解析效果。
        let effect = stack
            // 使用产品默认八像素。
            .backdrop_effect(8.0)
            // 独立区域不需要 mask bounds。
            .expect("independent backdrop region should resolve");
        // 保留调用方逻辑区域。
        assert_eq!(effect.region(), Rect::new(4.0, 6.0, 20.0, 12.0));
        // 使用 Theme token 半径。
        assert_eq!(effect.radius(), 8.0);
        // 小于半像素按契约为 no-op。
        assert!(stack.backdrop_effect(0.49).is_none());
    }
}
