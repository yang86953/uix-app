use crate::core::WidgetId;
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
/// 浮层的稳定语义类别。
pub enum OverlayKind {
    /// 模态对话框。
    Modal,
    /// 从窗口边缘滑入的抽屉。
    Drawer,
    /// 锚定到组件的气泡卡片。
    Popover,
    /// 短暂提示浮层。
    Tooltip,
    /// 上下文菜单。
    ContextMenu,
    /// 全局消息提示。
    Message,
    /// 持续通知浮层。
    Notification,
    /// 应用定义的自定义浮层。
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// 在单个浮层栈内唯一的登记标识。
pub struct OverlayId(u64);

impl OverlayId {
    /// 返回底层稳定数值标识。
    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone)]
/// 一项待排序、命中和管理的浮层登记。
pub struct OverlayEntry {
    id: OverlayId,
    owner: WidgetId,
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

// 保存一次组件浮层重建中需要跨阶段复用的临时集合。
#[derive(Default)]
pub(crate) struct OverlayRebuildScratch {
    pub(crate) previous_trap_owners: Vec<WidgetId>,
    pub(crate) entries: Vec<OverlayEntry>,
}

// 只保留判断浮层拓扑变化所需的复制值。
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct OverlayTopologySnapshot {
    owner: WidgetId,
    kind: OverlayKind,
    bounds: Option<Rect>,
    z_index: i32,
    modal: bool,
    dismiss_on_outside: bool,
    focus_trap: bool,
    backdrop_blur: Option<OverlayBackdropBlur>,
}

impl OverlayEntry {
    // 提取不携带浮层实例所有权的拓扑比较快照。
    pub(crate) fn topology_snapshot(&self) -> OverlayTopologySnapshot {
        OverlayTopologySnapshot {
            owner: self.owner,
            kind: self.kind,
            bounds: self.bounds,
            z_index: self.z_index,
            modal: self.modal,
            dismiss_on_outside: self.dismiss_on_outside,
            focus_trap: self.focus_trap,
            backdrop_blur: self.backdrop_blur,
        }
    }

    /// 为组件所有者和语义类别创建默认浮层登记。
    pub fn new(owner: WidgetId, kind: OverlayKind) -> Self {
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

    /// 设置浮层的逻辑边界。
    pub fn bounds(mut self, bounds: Rect) -> Self {
        self.bounds = Some(bounds);
        self
    }

    /// 设置浮层排序层级。
    pub fn z_index(mut self, z_index: i32) -> Self {
        self.z_index = z_index;
        self
    }

    /// 设置浮层是否阻断底层交互。
    pub fn modal(mut self, modal: bool) -> Self {
        self.modal = modal;
        self
    }

    /// 设置点击浮层外部是否触发关闭。
    pub fn dismiss_on_outside(mut self, dismiss: bool) -> Self {
        self.dismiss_on_outside = dismiss;
        self
    }

    /// 设置焦点是否限制在浮层子树内。
    pub fn focus_trap(mut self, focus_trap: bool) -> Self {
        self.focus_trap = focus_trap;
        self
    }

    /// 设置登记是否由浮层管理器持续维护。
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

    /// 返回压栈时分配的浮层标识。
    pub fn id(&self) -> OverlayId {
        self.id
    }

    /// 返回拥有该浮层的组件标识。
    pub fn owner(&self) -> WidgetId {
        self.owner
    }

    /// 返回浮层语义类别。
    pub fn kind(&self) -> OverlayKind {
        self.kind
    }

    /// 返回可选的逻辑边界。
    pub fn bounds_rect(&self) -> Option<Rect> {
        self.bounds
    }

    /// 返回浮层排序层级。
    pub fn z_index_value(&self) -> i32 {
        self.z_index
    }

    /// 判断浮层是否阻断底层交互。
    pub fn is_modal(&self) -> bool {
        self.modal
    }

    /// 判断点击外部是否应关闭浮层。
    pub fn dismisses_on_outside(&self) -> bool {
        self.dismiss_on_outside
    }

    /// 判断焦点是否限制在浮层子树内。
    pub fn traps_focus(&self) -> bool {
        self.focus_trap
    }

    /// 判断登记是否由浮层管理器持续维护。
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
/// 按层级保存当前窗口浮层登记的所有者容器。
pub struct OverlayStack {
    next_id: u64,
    entries: Vec<OverlayEntry>,
}

impl OverlayStack {
    /// 创建空浮层栈。
    pub fn new() -> Self {
        Self::default()
    }

    /// 使用默认设置登记指定组件的浮层。
    pub fn push(&mut self, owner: WidgetId, kind: OverlayKind) -> OverlayId {
        self.push_entry(OverlayEntry::new(owner, kind))
    }

    /// 分配标识并按层级登记完整浮层条目。
    pub fn push_entry(&mut self, mut entry: OverlayEntry) -> OverlayId {
        self.next_id += 1;
        let id = OverlayId(self.next_id);
        entry.id = id;
        // 在相同层级的既有项之后插入，保持稳定声明顺序且不创建排序缓冲。
        let insertion = self
            .entries
            .partition_point(|current| current.z_index <= entry.z_index);
        self.entries.insert(insertion, entry);
        id
    }

    /// 按标识移除并返回浮层条目。
    pub fn remove(&mut self, id: OverlayId) -> Option<OverlayEntry> {
        let index = self.entries.iter().position(|entry| entry.id == id)?;
        Some(self.entries.remove(index))
    }

    /// 移除并返回指定组件拥有的全部浮层。
    pub fn remove_for_owner(&mut self, owner: WidgetId) -> Vec<OverlayEntry> {
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

    /// 仅保留谓词接受的浮层条目。
    pub fn retain_entries(&mut self, mut keep: impl FnMut(&OverlayEntry) -> bool) {
        self.entries.retain(|entry| keep(entry));
    }

    /// 移除全部浮层条目。
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// 返回当前浮层数量。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 判断当前是否没有浮层。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 按从低到高的层级顺序遍历浮层。
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &OverlayEntry> {
        self.entries.iter()
    }

    /// 返回层级最高的浮层。
    pub fn top(&self) -> Option<&OverlayEntry> {
        self.entries.last()
    }

    /// 从最高层开始查找包含指定逻辑坐标的浮层。
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
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../tests/unit/ui/overlay/mod__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
