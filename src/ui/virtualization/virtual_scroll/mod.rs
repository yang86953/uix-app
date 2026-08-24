//! VirtualScroll - virtual scrolling container.
//!
//! Renders only children near the viewport; useful for large Select, Tree,
//! Table, and similar lists.
// 记录布局回调中的滚动状态与可变高度缓存。
use std::cell::{Cell, RefCell};
// 保存稳定测量代际下已经按原顺序计算完成的精确项目偏移。
use std::collections::HashMap;

// 导入可变行高的稀疏缓存和范围计算入口。
pub use super::measurement_cache::{
    VirtualListMeasurementCache, virtual_list_index_range_with_measurements,
};

// 将跨组件复用的等高列表滚动状态隔离到独立模块。
#[path = "list_scroll.rs"]
// 编译并保持共享滚动状态的原有公开路径。
mod list_scroll;
// 从虚拟滚动模块继续导出共享列表滚动状态。
pub use list_scroll::VirtualListScroll;

// 复用布局层对非有限值与测量哨兵的统一归一规则。
use crate::ui::layout::engine::{finite_non_negative, finite_or_zero};

// 限制一次刷新可创建的虚拟行数量，避免不可信视口或 overscan 耗尽资源。
const MAX_MATERIALIZED_ITEMS: usize = 4_096;
// 偏移缓存最多保留两窗结果，防止长列表滚动导致运行态无界增长。
const MAX_CACHED_ITEM_OFFSETS: usize = MAX_MATERIALIZED_ITEMS * 2;
// 为累计坐标保留充足算术余量，避免后续加减重新溢出为无穷大。
const MAX_VIRTUAL_EXTENT: f32 = f32::MAX / 4.0;

// 保存一个测量代际和估算行高下已经完成最终舍入的项目偏移。
#[derive(Debug, Default)]
struct VirtualItemOffsetCache {
    // 代际与估算值按位共同决定全部缓存结果的有效性。
    key: Option<(u64, u32)>,
    // 只保存调用方实际查询过的绝对索引，避免按数据总量分配。
    offsets: HashMap<usize, f32>,
}

impl VirtualItemOffsetCache {
    // 读取同一几何事实下已经计算完成的最终偏移。
    fn get(&mut self, key: (u64, u32), index: usize) -> Option<f32> {
        // 任一测量或估算变化都会整体丢弃旧坐标。
        if self.key != Some(key) {
            self.key = Some(key);
            self.offsets.clear();
        }
        // 命中值已经是原算法产生的最终 f32，不重新组合浮点加法。
        self.offsets.get(&index).copied()
    }

    // 写入一次原算法完成后的最终偏移。
    fn insert(&mut self, key: (u64, u32), index: usize, offset: f32) {
        // 防御读取和写入之间发生键变化；常规路径不会触发该分支。
        if self.key != Some(key) {
            self.key = Some(key);
            self.offsets.clear();
        }
        // 达到固定预算后开启新一轮有界缓存，不保留长列表历史轨迹。
        if self.offsets.len() >= MAX_CACHED_ITEM_OFFSETS {
            self.offsets.clear();
        }
        // 同一索引只保存最终有限偏移。
        self.offsets.insert(index, offset);
    }
}

// 唯一标识一次可变高度物化范围计算的全部输入事实。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VirtualRangeCacheKey {
    // 实际测量发生变化时必须重新计算范围。
    measurement_generation: u64,
    // 数据长度决定范围上界和总高度。
    item_count: usize,
    // 浮点输入按位区分，避免缓存改变 NaN 或符号零归一路径。
    estimated_height_bits: u32,
    // 滚动状态按原始位模式参与缓存身份。
    scroll_offset_bits: u32,
    // 视口高度按原始位模式参与缓存身份。
    viewport_height_bits: u32,
    // 预渲染数量决定最终物化窗口。
    overscan: usize,
}

// 保存原范围算法针对一组完整输入产生的唯一结果。
#[derive(Debug, Clone, Copy)]
struct VirtualRangeCacheEntry {
    // 输入键用于逐字段精确命中。
    key: VirtualRangeCacheKey,
    // 结果直接复用，不重新执行总高扫描和边界二分。
    range: (usize, usize),
}

// 唯一标识一次可变列表总高度计算的全部输入事实。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VirtualTotalHeightCacheKey {
    // 实际测量变化后旧总高度立即失效。
    measurement_generation: u64,
    // 数据长度决定需要纳入的项目数量。
    item_count: usize,
    // 估算行高按原始位模式区分全部浮点输入。
    estimated_height_bits: u32,
}

// 保存原总高度算法已经完成最终舍入的单值结果。
#[derive(Debug, Clone, Copy)]
struct VirtualTotalHeightCacheEntry {
    // 精确输入键防止跨几何事实复用。
    key: VirtualTotalHeightCacheKey,
    // 高度是原算法产生的最终有限 f32。
    height: f32,
}

// 把有效正度量提升为 f64，供索引与总高度计算使用。
fn positive_measurement(value: f32) -> Option<f64> {
    // f32::MAX 是布局层的无界测量哨兵，不能作为实际行高或视口。
    (value.is_finite() && value > 0.0 && value < f32::MAX).then_some(value as f64)
}

// 将尺寸限制为布局树可以安全保存的有限非负范围。
fn finite_virtual_size(value: f32) -> f32 {
    // 先套用共享布局规则，再为后续算术保留余量。
    finite_non_negative(value).min(MAX_VIRTUAL_EXTENT)
}

// 将滚动偏移限制为有限非负坐标。
fn finite_scroll_offset(value: f32) -> f32 {
    // 滚动状态与实际布局坐标采用同一安全上限。
    finite_virtual_size(value)
}

// 计算可表示且有限的固定行高内容总高度。
fn finite_total_height(item_count: usize, item_height: f32) -> f32 {
    // 非法行高或空列表没有可滚动内容。
    let Some(item_height) = positive_measurement(item_height) else {
        // 返回稳定有限的空内容高度。
        return 0.0;
    };
    // 使用 f64 避免 usize 到 f32 后的乘法提前溢出。
    let total = item_count as f64 * item_height;
    // 将超出布局坐标预算的内容夹到有限上限。
    total.min(MAX_VIRTUAL_EXTENT as f64) as f32
}

// 计算固定行高列表的有限最大滚动偏移。
fn finite_max_scroll_offset(item_count: usize, item_height: f32, viewport_height: f32) -> f32 {
    // 内容高度已经在共享虚拟坐标预算内。
    let total_height = finite_total_height(item_count, item_height);
    // 非法视口按零处理，确保减法仍保持有限。
    let viewport_height = finite_virtual_size(viewport_height);
    // 内容不满视口时稳定停留在原点。
    (total_height - viewport_height).max(0.0)
}

// 由已知的有限总高度计算有限最大滚动偏移。
fn finite_max_scroll_from_total(total_height: f32, viewport_height: f32) -> f32 {
    // 内容高度已经在共享虚拟坐标预算内。
    let total_height = finite_virtual_size(total_height);
    // 非法视口按零处理，确保减法仍保持有限。
    let viewport_height = finite_virtual_size(viewport_height);
    // 内容不满视口时稳定停留在原点。
    (total_height - viewport_height).max(0.0)
}

// 将一次滚动增量应用到有限内容区间。
fn scroll_offset_after_delta(current: f32, delta: f32, max: f32) -> (f32, f32) {
    // 最大偏移必须先满足有限非负不变量。
    let max = finite_scroll_offset(max);
    // 旧状态即使来自不可信恢复数据也先归一并夹到内容范围。
    let current = finite_scroll_offset(current).min(max);
    // NaN 没有方向语义，因此忽略；正负无穷分别表示滚到对应边界。
    let next = if delta.is_nan() {
        // 忽略无方向的非数增量。
        current
    } else if delta == f32::INFINITY {
        // 正无穷只能抵达有限末端。
        max
    } else if delta == f32::NEG_INFINITY {
        // 负无穷只能抵达有限原点。
        0.0
    } else {
        // 使用 f64 累加，避免两个有限 f32 在夹取前先溢出。
        (current as f64 + delta as f64).clamp(0.0, max as f64) as f32
    };
    // 同时返回有限的新状态和真正生效的位移。
    (next, next - current)
}

// 将非负浮点索引向下取整并饱和到 usize。
fn floor_index(value: f64) -> usize {
    // Rust 的饱和转换负责处理超过平台索引范围的有限商值。
    value.max(0.0).floor() as usize
}

// 将非负浮点索引向上取整并饱和到 usize。
fn ceil_index(value: f64) -> usize {
    // 可见窗口尾端采用开区间，因此向上覆盖最后一段行高。
    value.max(0.0).ceil() as usize
}

// 将累计坐标夹到有限虚拟坐标预算。
fn finite_virtual_coordinate(value: f64) -> f32 {
    // NaN 没有可保留的位置语义，统一回退到原点。
    if value.is_nan() {
        // 返回稳定有限的坐标。
        return 0.0;
    }
    // 正负溢出保留方向并夹到可安全参与后续算术的范围。
    value.clamp(-(MAX_VIRTUAL_EXTENT as f64), MAX_VIRTUAL_EXTENT as f64) as f32
}

// 归一虚拟滚动接收或返回的实际矩形。
fn finite_virtual_rect(frame: Rect) -> Rect {
    // 坐标允许有限负值，尺寸则保持有限非负。
    Rect::new(
        finite_virtual_coordinate(frame.x as f64),
        finite_virtual_coordinate(frame.y as f64),
        finite_virtual_size(frame.w),
        finite_virtual_size(frame.h),
    )
}

/// Visible index range `[start, end)` for a fixed-height virtual list.
pub fn virtual_list_index_range(
    item_count: usize,
    item_height: f32,
    scroll_offset: f32,
    viewport_height: f32,
    overscan: usize,
) -> (usize, usize) {
    // 空列表没有可物化内容。
    if item_count == 0 {
        // 返回规范空区间。
        return (0, 0);
    }
    // 无有效行高时无法建立索引到坐标的映射。
    let Some(item_height) = positive_measurement(item_height) else {
        // 非法度量统一返回有限空结果。
        return (0, 0);
    };
    // 无有效可见面积时 overscan 也不得单独触发行物化。
    let Some(viewport_height) = positive_measurement(viewport_height) else {
        // 非法或非正视口统一返回有限空结果。
        return (0, 0);
    };
    // 用未截断的 f64 总高度计算恢复状态的真实末端边界。
    let total_height = item_count as f64 * item_height;
    // 滚动状态同时受内容末端与虚拟坐标预算约束。
    let max_offset = (total_height - viewport_height)
        .max(0.0)
        .min(MAX_VIRTUAL_EXTENT as f64);
    // 非有限恢复值回到原点，超范围有限值夹到内容末端。
    let scroll_offset = (finite_scroll_offset(scroll_offset) as f64).min(max_offset);
    // 首个可见索引采用向下取整并限制到数据长度。
    let first = floor_index(scroll_offset / item_height).min(item_count);
    // 开区间尾索引向上覆盖视口末端并限制到数据长度。
    let last = ceil_index((scroll_offset + viewport_height) / item_height)
        .min(item_count)
        .max(first);
    // 可见区和 overscan 统一经过固定预算分配。
    materialization_window(item_count, first, last, overscan)
}

// 把可见区扩展为不超过预算的半开物化窗口。
fn materialization_window(
    item_count: usize,
    first: usize,
    last: usize,
    overscan: usize,
) -> (usize, usize) {
    // 可见行始终优先于装饰性 overscan 占用物化预算。
    let visible_len = last.saturating_sub(first);
    // 超大视口本身就超过预算时，只保留从首个可见行开始的一窗。
    if visible_len >= MAX_MATERIALIZED_ITEMS {
        // 饱和加法避免平台极值索引发生整数溢出。
        let end = first.saturating_add(MAX_MATERIALIZED_ITEMS).min(item_count);
        // 返回严格有界且仍包含最前可见内容的窗口。
        return (first, end);
    }
    // 计算可分配给窗口两侧 overscan 的剩余容量。
    let remaining = MAX_MATERIALIZED_ITEMS - visible_len;
    // 起始侧最多只能扩展到索引零。
    let before_requested = overscan.min(first);
    // 尾侧最多只能扩展到数据末端。
    let after_requested = overscan.min(item_count.saturating_sub(last));
    // 常规小窗口完整保留调用方请求的双侧 overscan。
    if before_requested.saturating_add(after_requested) <= remaining {
        // 两侧扩展均已由边界约束，无整数溢出风险。
        return (first - before_requested, last + after_requested);
    }
    // 预算不足时先为两侧分配对称份额。
    let mut before = before_requested.min(remaining / 2);
    // 尾侧可使用未被起始侧占用的全部份额。
    let mut after = after_requested.min(remaining - before);
    // 若尾侧临近边界，则把剩余预算回填给起始侧。
    let extra_before = (before_requested - before).min(remaining - before - after);
    // 应用可用的起始侧回填。
    before += extra_before;
    // 若起始侧临近边界，则把最后的剩余预算回填给尾侧。
    let extra_after = (after_requested - after).min(remaining - before - after);
    // 应用可用的尾侧回填。
    after += extra_after;
    // 最终窗口包含完整优先可见区且不超过固定预算。
    let start = first - before;
    // 尾索引只增加最多 remaining，且已受数据末端约束。
    let end = last + after;
    // 返回有界半开区间。
    (start, end)
}

use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintPass;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::widget;
// 引入 builder 建树时登记 renderer sidecar 所需的内部注册类型。
use crate::ui::render_handler::RenderHandlerRegistration;
use crate::ui::{EventResult, SystemEvent, WidgetId, WidgetTree};
// 兼容既有公开模块路径，同时让应用闭包实现保留在拆分文件中。
pub use super::renderer::VirtualScrollBuilder;

widget! {
    /// 只物化当前可见范围并拥有滚动与测量缓存的虚拟滚动组件。
    pub struct VirtualScroll {
        item_count: usize,
        item_height: f32,
        variable_height: bool,
        measurement_version: u64,
        scroll_offset: Cell<f32>,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
        overscan: usize,
        #[snapshot(skip)]
        materialized_range: Cell<Option<(usize, usize)>>,
        #[snapshot(skip)]
        materialized_measurement_generation: Cell<u64>,
        #[snapshot(skip)]
        measurement_cache: RefCell<VirtualListMeasurementCache>,
        #[snapshot(skip)]
        item_offset_cache: RefCell<VirtualItemOffsetCache>,
        #[snapshot(skip)]
        range_cache: Cell<Option<VirtualRangeCacheEntry>>,
        #[snapshot(skip)]
        total_height_cache: Cell<Option<VirtualTotalHeightCacheEntry>>,
        pub(crate) last_frame: Cell<Option<Rect>>,
        scroll_delta_strip: Cell<(f32, f32)>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    has_dynamic_content => (&self) -> bool {
        true
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if let SystemEvent::Wheel { delta, .. } = event {
            // 读取已经归一化的实际视口高度。
            let view_h = self.viewport_height();
            // 计算有限内容边界，避免总高度与视口减法产生非有限值。
            let max_offset = self.max_scroll_offset_for_viewport(view_h);
            // 乘法溢出会形成有方向的无穷增量，并由共享入口夹到边界。
            let (new_offset, applied) = scroll_offset_after_delta(
                self.scroll_offset.get(),
                delta.y * 40.0,
                max_offset,
            );
            // 即使没有实际位移也要清理可能来自恢复状态的非法旧值。
            self.scroll_offset.set(new_offset);
            // 只有达到既有交互阈值的实际位移才触发滚动脏区。
            if applied.abs() > 0.5 {
                // 向脏区系统报告已经归一的有限位移。
                self.push_scroll_delta(0.0, applied);
            }
            // 滚轮事件由虚拟滚动容器消费。
            EventResult::Handled
        } else {
            // 其余事件继续交给树中的其他处理器。
            EventResult::NotHandled
        }
    }

    scroll_delta_for_dirty => (&self) -> Option<(f32, f32)> {
        // 清除历史状态中可能残留的非有限分量。
        let delta = self.scroll_delta_strip.get();
        // 将两个轴都限制为安全有限坐标。
        let delta = (
            finite_virtual_coordinate(delta.0 as f64),
            finite_virtual_coordinate(delta.1 as f64),
        );
        // 保留既有最小脏区阈值。
        if delta.0.abs() > 0.01 || delta.1.abs() > 0.01 {
            // 已消费的累计位移回到原点。
            self.scroll_delta_strip.set((0.0, 0.0));
            // 返回有限滚动位移。
            Some(delta)
        } else {
            // 小位移继续保留，等待后续滚动累计越过阈值。
            self.scroll_delta_strip.set(delta);
            // 当前无需生成滚动脏区。
            None
        }
    }

    viewport_scroll_offset => (&self) -> Option<(f32, f32)> {
        // 对外只暴露有限非负的纵向偏移。
        Some((0.0, finite_scroll_offset(self.scroll_offset.get())))
    }

    children_clip => (&self, frame: Rect) -> Option<Rect> {
        // 裁剪矩形也必须遵守最终布局有限几何不变量。
        Some(finite_virtual_rect(frame))
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        // 脏区不能携带非有限坐标或负尺寸。
        finite_virtual_rect(frame)
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        // 在缓存和绘制前统一归一实际 frame。
        let frame = finite_virtual_rect(frame);
        // 保存有限视口供滚动范围计算使用。
        self.last_frame.set(Some(frame));
        // 非内容绘制阶段无需填充背景。
        if ctx.paint_pass() != PaintPass::Content {
            // 保持其他绘制阶段无副作用。
            return;
        }
        // 读取容器背景语义色。
        let bg = ctx.tokens().color_bg_container();
        // 使用已经归一化的矩形绘制背景。
        ctx.fill_rect(frame, bg, None);
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], tree: &WidgetTree)
        -> Vec<(WidgetId, Rect)>
    {
        // 父级输入先收敛到有限实际矩形。
        let frame = finite_virtual_rect(frame);
        // 读取当前物化窗口的绝对起始索引。
        let start = self.visible_start();
        // 按物化顺序为每个行子树计算绝对位置。
        children
            .iter()
            // 离场行保留最后 frame 供动画绘制，但不再占用活动行绝对索引。
            .filter(|child| !tree.is_pending_removal_subtree(child.id))
            .enumerate()
            .map(|(local_i, child)| {
                // 防御不一致子项数量导致绝对索引整数溢出。
                let abs_i = start.saturating_add(local_i);
                // 可变模式先记录当前已物化子项的实际测量高度。
                if self.variable_height {
                    // 测量变化会让下一轮刷新重新确认窗口和锚点。
                    self.measure_item(abs_i, child.measured_size.h);
                }
                // 使用缓存前缀或固定行高计算项目起点。
                let item_offset = self.item_offset(abs_i) as f64;
                // 每个项目都读取最新的可变行高重锚偏移。
                let scroll_offset = finite_scroll_offset(self.scroll_offset.get()) as f64;
                // 使用 f64 完成坐标与偏移累加。
                let y = frame.y as f64 + item_offset - scroll_offset;
                // 最终纵坐标保留方向并夹到有限虚拟坐标范围。
                let y = finite_virtual_coordinate(y);
                // 可变模式返回实际高度，未测量项目回退到估算高度。
                let item_height = self.item_height_for(abs_i).max(0.0);
                // 子项 frame 只包含有限坐标与非负有限尺寸。
                (child.id, Rect::new(frame.x, y, frame.w, item_height))
            })
            .collect()
    }
}

impl Default for VirtualScroll {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualScroll {
    /// 创建使用固定估算行高和默认预渲染数量的空虚拟列表。
    pub fn new() -> Self {
        Self {
            item_count: 0,
            item_height: 32.0,
            variable_height: false,
            measurement_version: 0,
            scroll_offset: Cell::new(0.0),
            fixed_width: None,
            fixed_height: None,
            overscan: 5,
            materialized_range: Cell::new(None),
            materialized_measurement_generation: Cell::new(0),
            measurement_cache: RefCell::new(VirtualListMeasurementCache::new()),
            item_offset_cache: RefCell::new(VirtualItemOffsetCache::default()),
            range_cache: Cell::new(None),
            total_height_cache: Cell::new(None),
            last_frame: Cell::new(None),
            scroll_delta_strip: Cell::new((0.0, 0.0)),
        }
    }

    /// Apply declarative configuration while retaining framework-owned scroll state.
    pub(crate) fn sync_from(&mut self, next: Self) {
        // 优先使用上次实际布局视口，否则使用下一版声明视口。
        let viewport_height = self
            .last_frame
            .get()
            .map(|frame| frame.h)
            .unwrap_or(next.fixed_height.unwrap_or(300.0));
        // 保存框架拥有的滚动运行态，声明更新不能直接覆盖它。
        let scroll_offset = self.scroll_offset.get();
        // 只有模式或测量版本变化时才整体丢弃旧缓存。
        let reset_measurements = self.variable_height != next.variable_height
            || self.measurement_version != next.measurement_version;
        // 配置更新中的版本切换必须重新测量已物化项目。
        if reset_measurements {
            // 版本切换同步缓存版本并清理旧几何结果。
            let mut cache = self.measurement_cache.borrow_mut();
            // 模式切换但版本不变时也必须清理旧模式结果。
            if self.measurement_version == next.measurement_version {
                // 固定与可变模式不能共享同一轮测量。
                cache.clear();
            } else {
                // 新字体、宽度、主题或数据版本通过缓存协议失效。
                cache.set_version(next.measurement_version);
            }
        }

        // 同步下一版列表长度。
        self.item_count = next.item_count;
        // 同步下一版固定行高声明。
        self.item_height = next.item_height;
        // 同步下一版可变行高模式。
        self.variable_height = next.variable_height;
        // 同步下一版测量版本。
        self.measurement_version = next.measurement_version;
        // 同步下一版固定宽度声明。
        self.fixed_width = next.fixed_width;
        // 同步下一版固定高度声明。
        self.fixed_height = next.fixed_height;
        // 同步下一版 overscan 声明。
        self.overscan = next.overscan;
        // 配置变化后让协调器重新核对物化窗口。
        self.materialized_range.set(None);
        // 数据缩短后丢弃超出新长度的稀疏测量。
        self.measurement_cache
            .borrow_mut()
            .retain_item_count(self.item_count);
        // 根据新版有限内容边界归一并夹取旧滚动状态。
        self.scroll_offset.set(
            finite_scroll_offset(scroll_offset)
                .min(self.max_scroll_offset_for_viewport(viewport_height)),
        );
    }

    /// 设置虚拟列表包含的逻辑项目总数。
    pub fn item_count(mut self, n: usize) -> Self {
        self.item_count = n;
        self
    }

    /// 设置固定行高，或在可变行高模式中设置未测量项目的估算高度。
    pub fn item_height(mut self, h: f32) -> Self {
        self.item_height = h;
        self
    }

    /// 开启按已物化项目实际测量高度布局的模式。
    pub fn variable_height(mut self) -> Self {
        // 固定行高仍作为尚未测量项目的估算高度。
        self.variable_height = true;
        self
    }

    /// 为字体、宽度、主题或数据变化绑定新的测量版本。
    pub fn measurement_version(mut self, version: u64) -> Self {
        // 版本切换由 sync_from 统一触发缓存清理。
        self.measurement_version = version;
        self
    }

    /// 设置可见区前后额外物化的项目数量。
    pub fn overscan(mut self, n: usize) -> Self {
        self.overscan = n;
        self
    }

    /// 设置虚拟列表视口的固定宽度与高度。
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.fixed_width = Some(w);
        self.fixed_height = Some(h);
        self
    }

    /// Total scrollable content height (fixed row height contract).
    pub fn total_height(&self) -> f32 {
        // 可变模式优先使用稀疏测量结果和固定估算高度。
        if self.variable_height {
            // 一次借用同时取得代际并执行可能需要的原总高度算法。
            let measurements = self.measurement_cache.borrow();
            // 全部高度输入按值或原始位模式组成精确缓存键。
            let key = VirtualTotalHeightCacheKey {
                measurement_generation: measurements.generation(),
                item_count: self.item_count,
                estimated_height_bits: self.item_height.to_bits(),
            };
            // 稳定状态直接复用原算法已经舍入完成的最终 f32。
            if let Some(entry) = self
                .total_height_cache
                .get()
                .filter(|entry| entry.key == key)
            {
                return entry.height;
            }
            // 未命中仍按原前缀实现计算，保持全部浮点与边界语义。
            let height = measurements.total_height(self.item_count, self.item_height);
            // 保存唯一结果供滚轮和边界夹取重复使用。
            self.total_height_cache
                .set(Some(VirtualTotalHeightCacheEntry { key, height }));
            return height;
        }
        // 固定模式使用原有饱和有限乘法。
        finite_total_height(self.item_count, self.item_height)
    }

    /// Viewport height from last layout frame or configured fixed height.
    pub fn viewport_height(&self) -> f32 {
        // 优先读取实际布局 frame，否则回退到声明或默认高度。
        let viewport_height = self
            .last_frame
            .get()
            .map(|f| f.h)
            .unwrap_or(self.fixed_height.unwrap_or(300.0));
        // 对外只返回有限非负实际高度。
        finite_virtual_size(viewport_height)
    }

    /// 返回当前视口与预渲染配置对应的半开项目索引范围。
    pub fn scroll_range(&self, viewport_height: f32) -> (usize, usize) {
        // 可变模式使用测量缓存提供的前缀坐标。
        if self.variable_height {
            // 一次借用同时取得代际并执行可能需要的原范围算法。
            let measurements = self.measurement_cache.borrow();
            // 全部几何输入按值或原始位模式组成精确缓存键。
            let key = VirtualRangeCacheKey {
                measurement_generation: measurements.generation(),
                item_count: self.item_count,
                estimated_height_bits: self.item_height.to_bits(),
                scroll_offset_bits: self.scroll_offset.get().to_bits(),
                viewport_height_bits: viewport_height.to_bits(),
                overscan: self.overscan,
            };
            // 稳定帧直接复用原算法已经产生的半开范围。
            if let Some(entry) = self.range_cache.get().filter(|entry| entry.key == key) {
                return entry.range;
            }
            // 未命中仍完整执行原范围实现，保持所有边界和浮点语义。
            let range = virtual_list_index_range_with_measurements(
                self.item_count,
                self.item_height,
                &measurements,
                self.scroll_offset.get(),
                viewport_height,
                self.overscan,
            );
            // 保存唯一结果供相同事实的后续稳定帧复用。
            self.range_cache
                .set(Some(VirtualRangeCacheEntry { key, range }));
            return range;
        }
        // 固定模式保持原有等高范围契约。
        virtual_list_index_range(
            self.item_count,
            self.item_height,
            self.scroll_offset.get(),
            viewport_height,
            self.overscan,
        )
    }

    /// Returns true when the visible index window no longer matches prepared children.
    pub(crate) fn needs_child_refresh(
        &self,
        viewport_height: f32,
        mounted_children: usize,
    ) -> bool {
        let range = self.scroll_range(viewport_height);
        // 可变高度测量变化时，即使索引窗口相同也必须重新协调。
        let measurement_changed = self.variable_height
            && self.materialized_measurement_generation.get()
                != self.measurement_cache.borrow().generation();
        // 同时检查索引窗口、挂载数量和测量代际。
        self.materialized_range.get() != Some(range)
            || mounted_children != range.1.saturating_sub(range.0)
            || measurement_changed
    }

    pub(crate) fn configured_viewport_height(&self) -> f32 {
        // 初次物化也不能使用声明中的非有限或负高度。
        finite_virtual_size(self.fixed_height.unwrap_or(300.0))
    }

    pub(crate) fn mark_children_materialized(&self, range: (usize, usize)) {
        self.materialized_range.set(Some(range));
        // 记录本次窗口已经消费的测量代际。
        self.materialized_measurement_generation
            .set(self.measurement_cache.borrow().generation());
    }

    /// 返回归一化后的有限非负滚动偏移。
    pub fn scroll_offset(&self) -> f32 {
        // 对外隐藏任何来自旧状态或直接恢复的非法分量。
        finite_scroll_offset(self.scroll_offset.get())
    }

    /// 返回当前偏移占最大可滚动距离的零到一比例。
    pub fn scroll_ratio(&self, viewport_height: f32) -> f32 {
        // 使用与事件路径相同的有限内容边界。
        let max_scroll = self.max_scroll_offset_for_viewport(viewport_height);
        // 无可滚动距离时比例稳定为零。
        if max_scroll <= 0.0 {
            // 避免人为以一作为分母掩盖非法状态。
            return 0.0;
        }
        // 有效偏移先夹到内容范围，再计算零到一比例。
        (finite_scroll_offset(self.scroll_offset.get()).min(max_scroll) / max_scroll)
            .clamp(0.0, 1.0)
    }

    /// 返回当前已物化窗口的首个项目索引。
    pub fn visible_start(&self) -> usize {
        self.materialized_range
            .get()
            .map(|range| range.0)
            .unwrap_or(0)
    }

    /// 记录已物化项目的实际高度，并报告缓存是否发生变化。
    pub fn measure_item(&self, index: usize, height: f32) -> bool {
        // 固定模式不应意外改变既有等高布局。
        if !self.variable_height || index >= self.item_count {
            // 非可变模式或越界索引直接忽略测量。
            return false;
        }
        // 稳定帧中的相同测量无需重复计算可见锚点和前缀偏移。
        if !self
            .measurement_cache
            .borrow()
            .would_record_change(index, height)
        {
            // 非法或未变化输入保持原有无刷新语义。
            return false;
        }
        let anchor = self.visible_anchor_index();
        let old_anchor_offset = self
            .measurement_cache
            .borrow()
            .offset_for_index(anchor, self.item_height);
        let changed = self.measurement_cache.borrow_mut().record(index, height);
        // 预检与正式写入共享判定，期间没有可重入修改点。
        debug_assert!(changed);
        if changed && index < anchor {
            let new_anchor_offset = self
                .measurement_cache
                .borrow()
                .offset_for_index(anchor, self.item_height);
            let delta = new_anchor_offset as f64 - old_anchor_offset as f64;
            let adjusted = finite_virtual_coordinate(self.scroll_offset.get() as f64 + delta);
            self.scroll_offset.set(
                finite_scroll_offset(adjusted)
                    .min(self.max_scroll_offset_for_viewport(self.viewport_height())),
            );
        }
        changed
    }

    /// 返回可变行高模式下当前版本保存的实际项目高度。
    pub fn measured_item_height(&self, index: usize) -> Option<f32> {
        // 固定模式没有对外暴露的实际测量结果。
        if !self.variable_height || index >= self.item_count {
            // 非可变模式和越界索引没有缓存值。
            return None;
        }
        // 只返回当前版本已保存的实际高度。
        self.measurement_cache.borrow().get(index)
    }

    /// 清理当前版本的全部可变行高测量结果。
    pub fn invalidate_measurements(&self) {
        // 清空实际高度，后续项目回退到估算行高。
        self.measurement_cache.borrow_mut().clear();
    }

    // 计算当前视口的首个可见项目，作为测量重锚基准。
    fn visible_anchor_index(&self) -> usize {
        // 固定模式无需基于测量变化调整偏移。
        if !self.variable_height {
            return 0;
        }
        // 只计算可见窗口，不把 overscan 当成视觉锚点。
        self.measurement_cache.borrow().first_visible_index(
            self.item_count,
            self.item_height,
            self.scroll_offset.get(),
            self.viewport_height(),
        )
    }

    fn intrinsic_size(&self) -> Size {
        // 声明尺寸在进入约束求解前先满足有限非负规则。
        Size::new(
            finite_virtual_size(self.fixed_width.unwrap_or(300.0)),
            finite_virtual_size(self.fixed_height.unwrap_or(300.0)),
        )
    }

    // 计算当前模式下给定视口的最大有限滚动偏移。
    fn max_scroll_offset_for_viewport(&self, viewport_height: f32) -> f32 {
        // 总高度统一从可变或固定实现读取。
        finite_max_scroll_from_total(self.total_height(), viewport_height)
    }

    // 读取当前模式下某一项目的起点偏移。
    fn item_offset(&self, index: usize) -> f32 {
        // 可变模式用稀疏测量修正估算前缀。
        if self.variable_height {
            // 测量代际与估算值按位共同绑定精确偏移结果。
            let key = (
                self.measurement_cache.borrow().generation(),
                self.item_height.to_bits(),
            );
            // 稳定布局直接复用原算法已经完成最终舍入的 f32。
            if let Some(offset) = self.item_offset_cache.borrow_mut().get(key, index) {
                return offset;
            }
            // 未命中仍按原有基线和有序修正顺序计算，保持布局结果。
            let offset = self
                .measurement_cache
                .borrow()
                .offset_for_index(index, self.item_height);
            // 保存最终结果，后续命中不改变任何浮点累计顺序。
            self.item_offset_cache
                .borrow_mut()
                .insert(key, index, offset);
            return offset;
        }
        // 固定模式使用安全的等高乘法。
        finite_virtual_coordinate(index as f64 * finite_virtual_size(self.item_height) as f64)
    }

    // 读取当前模式下某一项目的实际或估算高度。
    fn item_height_for(&self, index: usize) -> f32 {
        // 可变模式优先使用已缓存的实际高度。
        if self.variable_height {
            // 借用缓存只覆盖本次 frame 计算。
            return self
                .measurement_cache
                .borrow()
                .height_for(index, self.item_height);
        }
        // 固定模式保持原有有限尺寸归一规则。
        finite_virtual_size(self.item_height)
    }

    fn push_scroll_delta(&self, dx: f32, dy: f32) {
        // 输入位移先清除非有限值并限制到安全坐标范围。
        let dx = finite_virtual_coordinate(finite_or_zero(dx) as f64);
        // 纵轴采用相同归一规则。
        let dy = finite_virtual_coordinate(finite_or_zero(dy) as f64);
        // 保留既有微小位移过滤阈值。
        if dx.abs() <= 0.01 && dy.abs() <= 0.01 {
            // 无有效位移时不修改累计状态。
            return;
        }
        // 读取此前尚未被脏区系统消费的累计位移。
        let current = self.scroll_delta_strip.get();
        // 使用 f64 累加并在写回前夹到有限坐标范围。
        let next_x = finite_virtual_coordinate(current.0 as f64 + dx as f64);
        // 纵轴同样避免两个有限 f32 相加溢出。
        let next_y = finite_virtual_coordinate(current.1 as f64 + dy as f64);
        // 保存可安全传播到脏区计算的累计位移。
        self.scroll_delta_strip.set((next_x, next_y));
    }
}

impl VirtualScrollBuilder {
    fn into_parts(self) -> (VirtualScroll, RenderHandlerRegistration) {
        (
            self.scroll,
            RenderHandlerRegistration::VirtualScrollItem(self.renderer),
        )
    }

    /// 设置构建器生成的虚拟列表项目总数。
    pub fn item_count(mut self, n: usize) -> Self {
        self.scroll.item_count = n;
        self
    }

    /// 设置固定行高或可变模式的未测量估算高度。
    pub fn item_height(mut self, height: f32) -> Self {
        self.scroll.item_height = height;
        self
    }

    /// 开启按实际测量结果排列项目的模式。
    pub fn variable_height(mut self) -> Self {
        // 固定行高仍作为初次物化项目的估算值。
        self.scroll.variable_height = true;
        self
    }

    /// 为本次声明绑定字体、宽度、主题或数据版本。
    pub fn measurement_version(mut self, version: u64) -> Self {
        // 重协调时由 VirtualScroll 统一执行版本失效。
        self.scroll.measurement_version = version;
        self
    }

    /// 设置可见区前后额外物化的项目数量。
    pub fn overscan(mut self, n: usize) -> Self {
        self.scroll.overscan = n;
        self
    }

    /// 设置构建器生成的虚拟列表视口尺寸。
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.scroll.fixed_width = Some(width);
        self.scroll.fixed_height = Some(height);
        self
    }
}

impl crate::ui::view::View for VirtualScrollBuilder {
    fn build(self) -> crate::ui::view::ViewNode {
        let (scroll, handler) = self.into_parts();
        let mut node = crate::ui::view::ViewNode::leaf(scroll);
        node.render_handlers.push(handler);
        node
    }
}

impl From<VirtualScrollBuilder> for crate::ui::view::ViewNode {
    fn from(builder: VirtualScrollBuilder) -> Self {
        crate::ui::view::View::build(builder)
    }
}

// 仅在测试构建中加载虚拟滚动动态子树契约。
#[cfg(test)]
// 显式指向 virtualization 目录中的同级测试文件。
#[path = "../../../../tests/unit/ui/virtualization/tests.rs"]
// 将回归测试隔离到独立文件，保持实现文件聚焦。
mod tests;

// 仅在库测试中编译 VirtualScroll 动态私有状态与完整捕获交接门禁。
#[cfg(test)]
// 显式指向 virtualization 目录中的同级动态测试文件。
#[path = "../../../../tests/unit/ui/virtualization/dynamic_capture_tests.rs"]
// 挂载生产 renderer 接线的行为测试模块。
mod dynamic_capture_tests;
