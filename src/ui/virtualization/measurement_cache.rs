//! 可变行高虚拟列表的稀疏测量缓存。

// 以有序表保存已经物化并完成测量的绝对索引。
use std::collections::BTreeMap;

// 限制一次刷新可创建的虚拟行数量。
const MAX_MATERIALIZED_ITEMS: usize = 4_096;
// 为累计坐标保留充足算术余量。
const MAX_VIRTUAL_EXTENT: f32 = f32::MAX / 4.0;

// 把有效正度量提升为 f64，供索引与总高度计算使用。
fn positive_measurement(value: f32) -> Option<f64> {
    // f32::MAX 是布局层的无界测量哨兵，不能作为实际行高或视口。
    (value.is_finite() && value > 0.0 && value < f32::MAX).then_some(value as f64)
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

// 保存可变行高的稀疏测量结果，并按版本支持整体失效。
#[derive(Debug, Clone, Default)]
/// 按绝对索引保存可变行高，并用估算值补齐未测量项目的稀疏缓存。
pub struct VirtualListMeasurementCache {
    // 只为已经物化并完成测量的绝对索引分配存储。
    measurements: BTreeMap<usize, f32>,
    // 版本由调用方绑定字体、宽度、主题和数据语义。
    version: u64,
    // 每次测量结构变化都会递增，用于避免重复协调。
    generation: u64,
}

impl VirtualListMeasurementCache {
    // 创建空的可变行高缓存。
    /// 创建版本为零且不含任何实际测量的缓存。
    pub fn new() -> Self {
        // 使用默认状态保持缓存不占用数据项数组空间。
        Self::default()
    }

    // 返回当前测量版本。
    /// 返回调用方设置的测量失效版本。
    pub fn version(&self) -> u64 {
        // 版本值只作为失效边界，不参与几何计算。
        self.version
    }

    // 返回当前测量结构版本。
    /// 返回测量映射发生实际变化时递增的结构代数。
    pub fn generation(&self) -> u64 {
        // 结构版本只用于判断物化窗口是否需要复核。
        self.generation
    }

    // 返回已经成功记录的项目数量。
    /// 返回已记录有效实际高度的项目数量。
    pub fn len(&self) -> usize {
        // 稀疏缓存长度只统计有效正高度。
        self.measurements.len()
    }

    // 判断缓存是否没有有效项目。
    /// 返回是否没有任何有效实际高度。
    pub fn is_empty(&self) -> bool {
        // 直接复用有序表的空状态。
        self.measurements.is_empty()
    }

    // 读取某一绝对索引的已测量高度。
    /// 返回指定绝对索引已记录的有效实际高度。
    pub fn get(&self, index: usize) -> Option<f32> {
        // 缓存只暴露已经通过有限正值校验的结果。
        self.measurements.get(&index).copied()
    }

    // 返回本次输入归一后是否会实际改变指定索引。
    fn changed_measurement(&self, index: usize, height: f32) -> Option<f32> {
        // 与正式写入共享同一有限正值规则，避免预检和提交产生分歧。
        let height = positive_measurement(height).map(|value| value as f32)?;
        // 相同测量无需触发任何锚点或缓存刷新工作。
        (self.measurements.get(&index).copied() != Some(height)).then_some(height)
    }

    // 在不修改缓存的前提下判断一次测量是否会产生结构变化。
    pub(crate) fn would_record_change(&self, index: usize, height: f32) -> bool {
        // 复用正式写入判定，稳定帧可在昂贵的锚点计算前退出。
        self.changed_measurement(index, height).is_some()
    }

    // 记录一个有限正的项目高度，并报告缓存是否发生变化。
    /// 记录有限正高度，并在新增或改变测量时返回 `true`。
    pub fn record(&mut self, index: usize, height: f32) -> bool {
        // 非法或相同测量不覆盖旧值，也不制造额外物化刷新。
        let Some(height) = self.changed_measurement(index, height) else {
            // 已有结果仍然有效，非法输入继续回退到旧值或估算值。
            return false;
        };
        // 保存新的有效测量。
        self.measurements.insert(index, height);
        // 高度变化会影响后续前缀坐标。
        self.generation = self.generation.wrapping_add(1);
        // 告知调用方缓存结构已经改变。
        true
    }

    // 删除一个项目的测量结果，并报告是否确实删除了缓存项。
    /// 删除指定绝对索引的测量，并在确实存在时返回 `true`。
    pub fn invalidate(&mut self, index: usize) -> bool {
        // 删除后该项目回退到估算高度。
        let removed = self.measurements.remove(&index).is_some();
        // 只有确实删除时才触发窗口复核。
        if removed {
            // 删除项目会改变后续所有前缀坐标。
            self.generation = self.generation.wrapping_add(1);
        }
        // 返回本次缓存是否确实发生变化。
        removed
    }

    // 清空所有已测量高度。
    /// 清空全部实际高度，并在原缓存非空时推进结构代数。
    pub fn clear(&mut self) {
        // 版本变化和显式失效都复用同一清理路径。
        if !self.measurements.is_empty() {
            // 清空非空缓存会改变后续所有前缀坐标。
            self.generation = self.generation.wrapping_add(1);
        }
        // 删除全部旧测量结果。
        self.measurements.clear();
    }

    // 绑定新的字体、宽度、主题或数据版本并清理旧测量。
    /// 切换失效版本并清空旧测量；版本改变时返回 `true`。
    pub fn set_version(&mut self, version: u64) -> bool {
        // 相同版本不应破坏仍可复用的测量。
        if self.version == version {
            // 没有版本切换就保留缓存。
            return false;
        }
        // 保存新的失效边界。
        self.version = version;
        // 旧版本的高度不能继续参与前缀坐标。
        self.clear();
        // 告知调用方需要重新锚定和物化。
        true
    }

    // 丢弃已经超出新数据长度的绝对索引。
    pub(crate) fn retain_item_count(&mut self, item_count: usize) {
        // 数据缩短后不保留无效尾部测量。
        let old_len = self.measurements.len();
        // 只保留新数据仍然拥有的绝对索引。
        self.measurements.retain(|index, _| *index < item_count);
        // 尾部裁剪会影响后续总高度和前缀坐标。
        if self.measurements.len() != old_len {
            // 记录一次结构变化供物化协调器复核。
            self.generation = self.generation.wrapping_add(1);
        }
    }

    // 根据估算高度读取一个项目的有效高度。
    /// 返回实际高度，未测量时回退到有限正的估算高度或零。
    pub fn height_for(&self, index: usize, estimated_height: f32) -> f32 {
        // 已测量项目优先使用实际高度。
        if let Some(height) = self.get(index) {
            // 返回已经校验过的缓存值。
            return height;
        }
        // 未测量项目只接受有限正的估算高度。
        positive_measurement(estimated_height)
            .map(|value| value as f32)
            .unwrap_or(0.0)
    }

    // 计算从内容起点到指定项目起点的有限偏移。
    /// 计算指定绝对索引起点的有限累计偏移。
    pub fn offset_for_index(&self, index: usize, estimated_height: f32) -> f32 {
        // 无效估算值无法为未知项目建立单调坐标。
        let Some(estimated_height) = positive_measurement(estimated_height) else {
            // 与固定行高协议保持相同的空结果语义。
            return 0.0;
        };
        // 先按估算高度计算完整基线，再叠加稀疏测量修正量。
        let mut offset = index as f64 * estimated_height;
        // 有序遍历所有位于目标索引之前的实际测量。
        for height in self
            .measurements
            .range(..index)
            .map(|(_, height)| *height as f64)
        {
            // 实际高度替换同一索引的估算高度。
            offset += height - estimated_height;
        }
        // 坐标最终必须落在安全的有限虚拟范围。
        finite_virtual_coordinate(offset)
    }

    // 计算所有项目的有限总高度。
    /// 根据实际测量和估算高度计算指定项目数的有限总高度。
    pub fn total_height(&self, item_count: usize, estimated_height: f32) -> f32 {
        // item_count 的前缀起点已经包含所有项目高度。
        self.offset_for_index(item_count, estimated_height)
    }

    // 计算指定滚动状态下的首个可见项目。
    /// 返回给定滚动位置和视口下首个可见项目的绝对索引。
    pub fn first_visible_index(
        &self,
        item_count: usize,
        estimated_height: f32,
        scroll_offset: f32,
        viewport_height: f32,
    ) -> usize {
        // 复用范围计算并关闭 overscan，得到视觉锚点。
        virtual_list_index_range_with_measurements(
            item_count,
            estimated_height,
            self,
            scroll_offset,
            viewport_height,
            0,
        )
        .0
    }

    // 找到覆盖给定偏移的首个项目索引。
    fn index_at_offset(&self, item_count: usize, estimated_height: f32, offset: f64) -> usize {
        // 使用下界二分避免逐项扫描超大数据集。
        let mut low = 0usize;
        // 末端以 item_count 表示空的尾部位置。
        let mut high = item_count;
        // 查找最后一个起点不超过目标偏移的项目。
        while low < high {
            // 使用差值计算避免索引相加溢出。
            let middle = low + (high - low) / 2;
            // 读取候选项目后一个起点作为分界。
            let next_start = middle.saturating_add(1);
            // 目标正好落在边界时进入后一个项目。
            if self.offset_for_index(next_start, estimated_height) as f64 <= offset {
                // 保留已经完全越过目标的前缀。
                low = next_start;
            } else {
                // 当前候选仍可能是首个覆盖项目。
                high = middle;
            }
        }
        // 返回不超过数据长度的稳定索引。
        low.min(item_count)
    }

    // 找到第一个起点不小于给定偏移的项目索引。
    fn lower_bound_offset(&self, item_count: usize, estimated_height: f32, offset: f64) -> usize {
        // 使用有序前缀坐标执行二分下界查找。
        let mut low = 0usize;
        // item_count 表示可能的尾部下界。
        let mut high = item_count;
        // 查找第一个起点达到开区间尾部的项目。
        while low < high {
            // 使用差值计算避免索引相加溢出。
            let middle = low + (high - low) / 2;
            // 当前项目起点严格落在尾部之前时继续向右。
            if (self.offset_for_index(middle, estimated_height) as f64) < offset {
                // 当前项目不能作为开区间尾端。
                low = middle.saturating_add(1);
            } else {
                // 当前项目可能是首个满足下界的项目。
                high = middle;
            }
        }
        // 返回不超过数据长度的稳定索引。
        low.min(item_count)
    }
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

// 使用稀疏测量缓存计算可变行高列表的物化范围。
/// 计算包含可见项和 overscan、且不超过物化预算的绝对索引半开区间。
pub fn virtual_list_index_range_with_measurements(
    item_count: usize,
    estimated_height: f32,
    measurements: &VirtualListMeasurementCache,
    scroll_offset: f32,
    viewport_height: f32,
    overscan: usize,
) -> (usize, usize) {
    // 空列表没有可物化内容。
    if item_count == 0 {
        // 返回规范空区间。
        return (0, 0);
    }
    // 未知项目必须依靠有限正估算高度建立单调坐标。
    let Some(_) = positive_measurement(estimated_height) else {
        // 非法估算高度统一返回有限空结果。
        return (0, 0);
    };
    // 无有效可见面积时 overscan 也不得单独触发行物化。
    let Some(viewport_height) = positive_measurement(viewport_height) else {
        // 非法或非正视口统一返回有限空结果。
        return (0, 0);
    };
    // 使用测量缓存得到有限内容总高度。
    let total_height = measurements.total_height(item_count, estimated_height) as f64;
    // 空或非法总高度无法形成可见窗口。
    if total_height <= 0.0 {
        // 与固定行高入口保持一致的空结果语义。
        return (0, 0);
    }
    // 滚动状态受内容末端和共享虚拟坐标预算共同约束。
    let max_offset = (total_height - viewport_height)
        .max(0.0)
        .min(MAX_VIRTUAL_EXTENT as f64);
    // 非有限恢复值回到原点，超范围有限值夹到内容末端。
    let scroll_offset = if scroll_offset.is_finite() && scroll_offset > 0.0 {
        // 有限正偏移先限制到内容末端。
        (scroll_offset as f64).min(max_offset)
    } else if scroll_offset == f32::INFINITY {
        // 正无穷表示恢复到内容末端。
        max_offset
    } else {
        // NaN、负值和负无穷统一回到内容原点。
        0.0
    };
    // 找到首个覆盖滚动起点的项目。
    let first = measurements.index_at_offset(item_count, estimated_height, scroll_offset);
    // 视口尾部采用开区间，避免刚好落在边界时多物化一行。
    let viewport_end = (scroll_offset + viewport_height).min(total_height);
    // 找到第一个起点达到视口尾部的项目。
    let last = measurements.lower_bound_offset(item_count, estimated_height, viewport_end);
    // 可见区和 overscan 统一经过固定预算分配。
    materialization_window(item_count, first, last.max(first), overscan)
}
