//! Shared dirty-region and present-damage geometry contracts.

use crate::core::geometry::Rect;
use std::collections::VecDeque;

const DIRTY_MERGE_THRESHOLD: usize = 16;
const MAX_PRESENT_DAMAGE_RECTS: usize = 64;
const MAX_PRESENT_HISTORY_FRAMES: usize = 256;
const MAX_TRACKED_PRESENT_IMAGES: usize = 8;

/// Dirty region tracking for incremental rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct DirtyRegion {
    /// 需要重绘的离散逻辑矩形。
    pub rects: Vec<Rect>,
    /// 是否要求重绘完整帧。
    pub full_frame: bool,
    /// 是否需要在绘制前清理目标区域。
    pub clear_required: bool,
}

impl DirtyRegion {
    /// 创建要求清理并重绘完整帧的脏区。
    pub fn full() -> Self {
        Self {
            rects: Vec::new(),
            full_frame: true,
            clear_required: true,
        }
    }

    /// 创建不包含任何无效区域的脏区。
    pub fn empty() -> Self {
        Self {
            rects: Vec::new(),
            full_frame: false,
            clear_required: false,
        }
    }

    /// 从单个有效矩形创建局部脏区。
    pub fn area(rect: Rect) -> Self {
        Self {
            rects: if rect.w > 0.0 && rect.h > 0.0 {
                vec![rect]
            } else {
                Vec::new()
            },
            full_frame: false,
            clear_required: true,
        }
    }

    /// 在现有矩形存储中重置为单个局部脏区，供帧内临时视图跨帧复用容量。
    pub(crate) fn reuse_area(&mut self, rect: Rect) {
        self.rects.clear();
        self.full_frame = false;
        self.clear_required = rect.w > 0.0 && rect.h > 0.0;
        if self.clear_required {
            self.rects.push(rect);
        }
    }

    /// 复用当前容量复制另一脏区的几何与清理状态。
    pub(crate) fn reuse_from(&mut self, source: &Self) {
        self.rects.clear();
        self.rects.extend_from_slice(&source.rects);
        self.full_frame = source.full_frame;
        self.clear_required = source.clear_required;
    }

    /// 将脏区重置为空状态。
    pub fn reset(&mut self) {
        *self = Self::empty();
    }

    /// 判断当前是否没有任何重绘或清理需求。
    pub fn is_empty(&self) -> bool {
        !self.full_frame && self.rects.is_empty() && !self.clear_required
    }

    /// 添加一个有效矩形，并在数量达到阈值时合并边界。
    pub fn add_rect(&mut self, rect: Rect) {
        if rect.w <= 0.0 || rect.h <= 0.0 || self.full_frame {
            return;
        }
        // 已有矩形覆盖新区域时无需增加几何。
        if self
            .rects
            .iter()
            .any(|existing| rect_contains(*existing, rect))
        {
            return;
        }
        // 新矩形完整覆盖的旧区域可原地移除；滚动 viewport 因而复用 exposed strip 槽位。
        self.rects
            .retain(|existing| !rect_contains(rect, *existing));
        self.clear_required = true;
        if self.rects.len() >= DIRTY_MERGE_THRESHOLD - 1 {
            let bounds = self.bounds().union(&rect);
            self.rects.clear();
            self.rects.push(bounds);
        } else {
            self.rects.push(rect);
        }
    }

    /// 返回所有离散脏矩形的包围矩形。
    pub fn bounds(&self) -> Rect {
        if self.rects.is_empty() {
            return Rect::zero();
        }
        let mut bounds = self.rects[0];
        for &rect in &self.rects[1..] {
            bounds = bounds.union(&rect);
        }
        bounds
    }

    /// 绘制/清屏用脏区：保留离散矩形，供逐矩形 clear 与父背景重绘。
    ///
    /// 历史上多块 dirty 会升为并集 AABB，以避免「只清离散条带、父背景却画进
    /// 空隙而子节点不重绘」。拆分路径改为每个脏矩形独立清屏，并在该矩形 clip
    /// 内先绘祖先背景再绘脏控件，因此不再并集。
    pub fn for_paint_clear(&self) -> DirtyRegion {
        self.clone()
    }

    /// 判断指定矩形是否与任一脏区相交。
    pub fn intersects(&self, rect: Rect) -> bool {
        if self.full_frame {
            return true;
        }
        if rect.w <= 0.0 || rect.h <= 0.0 {
            return false;
        }
        self.rects
            .iter()
            .any(|&dirty| dirty.intersect(&rect).is_some())
    }

    /// 借用当前保存的离散脏矩形。
    pub fn rects(&self) -> &[Rect] {
        &self.rects
    }

    /// 移交局部脏矩形的底层分配，供帧生命周期继续复用同一所有权。
    pub(crate) fn into_rects(self) -> Vec<Rect> {
        self.rects
    }

    /// 接管已验证的局部矩形，并在原分配内恢复 `add_rect` 的去重与收敛语义。
    pub(crate) fn from_valid_rects(mut rects: Vec<Rect>) -> Self {
        let mut kept = 0;
        for read in 0..rects.len() {
            let rect = rects[read];
            if rects[..kept].contains(&rect) {
                continue;
            }
            if kept >= DIRTY_MERGE_THRESHOLD - 1 {
                let mut bounds = rect;
                for existing in &rects[..kept] {
                    bounds = bounds.union(existing);
                }
                rects[0] = bounds;
                kept = 1;
            } else {
                rects[kept] = rect;
                kept += 1;
            }
        }
        rects.truncate(kept);
        Self {
            clear_required: !rects.is_empty(),
            rects,
            full_frame: false,
        }
    }
}

fn rect_contains(outer: Rect, inner: Rect) -> bool {
    outer.x <= inner.x
        && outer.y <= inner.y
        && outer.x + outer.w >= inner.x + inner.w
        && outer.y + outer.h >= inner.y + inner.h
}

impl Default for DirtyRegion {
    fn default() -> Self {
        Self::empty()
    }
}

/// Rendering damage region.
#[derive(Debug, Clone, PartialEq)]
pub struct DamageRegion {
    /// 是否要求提交完整表面。
    pub full: bool,
    /// 局部提交使用的逻辑矩形。
    pub rects: Vec<Rect>,
}

impl DamageRegion {
    /// 创建完整表面损伤。
    pub fn full() -> Self {
        Self {
            full: true,
            rects: Vec::new(),
        }
    }

    /// 从离散逻辑矩形创建局部损伤。
    pub fn partial(rects: Vec<Rect>) -> Self {
        Self { full: false, rects }
    }

    /// 从单个矩形创建损伤，无效矩形会保守退化为完整损伤。
    pub fn from_rect(rect: Rect) -> Self {
        if rect.w <= 0.0 || rect.h <= 0.0 {
            Self::full()
        } else {
            Self::partial(vec![rect])
        }
    }

    /// 返回局部损伤的包围矩形，完整或空损伤返回空值。
    pub fn bounds(&self) -> Option<Rect> {
        if self.full || self.rects.is_empty() {
            return None;
        }
        let mut bounds = self.rects[0];
        for rect in &self.rects[1..] {
            bounds = bounds.union(rect);
        }
        Some(bounds)
    }

    /// Converts logical top-left damage into drawable buffer coordinates.
    ///
    /// Each minimum edge is rounded down, each maximum edge is rounded up,
    /// and the transformed result is clipped to the live drawable extent.
    /// Invalid surface metadata or damage degrades to [`PresentDamage::Full`].
    pub fn to_present_damage(&self, surface: PresentSurface) -> PresentDamage {
        if self.full || self.rects.is_empty() || !surface.is_valid() {
            PresentDamage::Full
        } else {
            let mut tuples = Vec::with_capacity(self.rects.len());
            for rect in &self.rects {
                match logical_rect_to_drawable(*rect, surface) {
                    Ok(Some(rect)) => tuples.push(rect),
                    Ok(None) => {}
                    Err(()) => return PresentDamage::Full,
                }
            }
            if tuples.is_empty() {
                PresentDamage::Full
            } else {
                PresentDamage::from_rects(tuples)
            }
        }
    }
}

impl Default for DamageRegion {
    fn default() -> Self {
        Self::full()
    }
}

/// Screen damage submitted with a CPU present or GPU buffer swap.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PresentDamage {
    #[default]
    /// 提交完整缓冲区。
    Full,
    /// 提交物理缓冲区坐标中的离散矩形。
    Partial(Vec<(i32, i32, i32, i32)>),
}

impl PresentDamage {
    /// 从单个物理矩形创建提交损伤。
    pub fn single(x: i32, y: i32, w: i32, h: i32) -> Self {
        if w <= 0 || h <= 0 {
            Self::Full
        } else {
            Self::Partial(vec![(x, y, w, h)])
        }
    }

    /// 判断是否要求提交完整缓冲区。
    pub fn is_full(&self) -> bool {
        matches!(self, Self::Full)
    }

    // 按当前 Surface 保留能力与物理范围规范化最终呈现损伤。
    pub(crate) fn normalize_for_surface(
        // 消费不再允许 Adapter 替换的 damage 值。
        self,
        // 接收 Surface 实际承诺的跨呈现保留语义。
        coherency: PresentCoherency,
        // 接收当前 drawable 的物理宽度。
        width: u32,
        // 接收当前 drawable 的物理高度。
        height: u32,
    ) -> Self {
        // 不保留像素的 Surface 与无效尺寸都只能完整呈现。
        if coherency == PresentCoherency::FullOnly || width == 0 || height == 0 {
            // 保守回退不会丢失任何已绘制像素。
            return Self::Full;
        }
        // 完整 damage 不需要矩形投影。
        let Self::Partial(rects) = self else {
            // 保留显式完整呈现。
            return Self::Full;
        };
        // 复用唯一矩形数量、空区域与合并规则。
        let Some(rects) = normalize_present_rects(rects) else {
            // 超量或无法规范化的输入降级为完整呈现。
            return Self::Full;
        };
        // 使用 i64 计算边界，避免任意公开枚举输入触发 i32 溢出。
        let surface_width = i64::from(width);
        // 高度使用同一无损投影。
        let surface_height = i64::from(height);
        // 每个规范化矩形都必须完整位于当前 drawable 内。
        let all_inside = rects.iter().all(|&(x, y, rect_width, rect_height)| {
            // 先投影为足以容纳两个 i32 和的边界值。
            let (x, y, rect_width, rect_height) = (
                i64::from(x),
                i64::from(y),
                i64::from(rect_width),
                i64::from(rect_height),
            );
            // 统一使用左上原点、正尺寸和右下开区间边界。
            x >= 0
                && y >= 0
                && rect_width > 0
                && rect_height > 0
                && x + rect_width <= surface_width
                && y + rect_height <= surface_height
        });
        // 空集或任一越界项都不能装作窄呈现成功。
        if rects.is_empty() || !all_inside {
            // 不把平台私有裁切规则带入共享语义。
            return Self::Full;
        }
        // 只发布数量受控且完整位于当前 Surface 的规范矩形。
        Self::Partial(rects)
    }

    /// Conservative union used by swapchain-history repair planning.
    pub fn union(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Full, _) | (_, Self::Full) => Self::Full,
            (Self::Partial(lhs), Self::Partial(rhs)) => {
                let mut rects = Vec::with_capacity(lhs.len().saturating_add(rhs.len()));
                rects.extend_from_slice(lhs);
                rects.extend_from_slice(rhs);
                Self::from_rects(rects)
            }
        }
    }

    fn from_rects(rects: Vec<(i32, i32, i32, i32)>) -> Self {
        match normalize_present_rects(rects) {
            Some(rects) if !rects.is_empty() => Self::Partial(rects),
            _ => Self::Full,
        }
    }
}

/// Buffer-preservation proof available at a presentation boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PresentCoherency {
    /// No preservation or image-history proof; draw and present the full frame.
    #[default]
    FullOnly,
    /// One retained target whose pixels outside damage survive successful presents.
    RetainedBuffer,
    /// Multiple images whose stale regions must be repaired from per-image history.
    TrackedSwapchain,
}

/// Mapping from logical top-left coordinates into the drawable buffer.
///
/// Mirrored variants follow the Vulkan convention: horizontal mirror first,
/// then clockwise rotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PresentTransform {
    #[default]
    /// 不转换逻辑坐标。
    Identity,
    /// 顺时针旋转九十度。
    Rotate90,
    /// 顺时针旋转一百八十度。
    Rotate180,
    /// 顺时针旋转二百七十度。
    Rotate270,
    /// 水平镜像。
    HorizontalMirror,
    /// 水平镜像后顺时针旋转九十度。
    HorizontalMirrorRotate90,
    /// 水平镜像后顺时针旋转一百八十度。
    HorizontalMirrorRotate180,
    /// 水平镜像后顺时针旋转二百七十度。
    HorizontalMirrorRotate270,
}

impl PresentTransform {
    fn swaps_axes(self) -> bool {
        matches!(
            self,
            Self::Rotate90
                | Self::Rotate270
                | Self::HorizontalMirrorRotate90
                | Self::HorizontalMirrorRotate270
        )
    }
}

/// Live surface metadata required for safe logical-to-drawable damage mapping.
#[derive(Debug, Clone, Copy)]
pub struct PresentSurface {
    /// 可绘制缓冲区的物理宽度。
    pub drawable_width: i32,
    /// 可绘制缓冲区的物理高度。
    pub drawable_height: i32,
    /// 逻辑坐标到物理像素的缩放比例。
    pub device_pixel_ratio: f32,
    /// 逻辑坐标到缓冲区的方向转换。
    pub transform: PresentTransform,
    /// Changes whenever the native surface is rebuilt, even at the same extent.
    pub generation: u64,
}

impl PresentSurface {
    /// 创建一份完整的呈现表面元数据。
    pub const fn new(
        drawable_width: i32,
        drawable_height: i32,
        device_pixel_ratio: f32,
        transform: PresentTransform,
        generation: u64,
    ) -> Self {
        Self {
            drawable_width,
            drawable_height,
            device_pixel_ratio,
            transform,
            generation,
        }
    }

    /// 创建不带方向转换的呈现表面元数据。
    pub const fn identity(
        drawable_width: i32,
        drawable_height: i32,
        device_pixel_ratio: f32,
        generation: u64,
    ) -> Self {
        Self::new(
            drawable_width,
            drawable_height,
            device_pixel_ratio,
            PresentTransform::Identity,
            generation,
        )
    }

    /// 判断尺寸和像素比例是否可用于损伤映射。
    pub fn is_valid(self) -> bool {
        self.drawable_width > 0
            && self.drawable_height > 0
            && self.device_pixel_ratio.is_finite()
            && self.device_pixel_ratio > 0.0
    }

    fn untransformed_extent(self) -> (f32, f32) {
        if self.transform.swaps_axes() {
            (self.drawable_height as f32, self.drawable_width as f32)
        } else {
            (self.drawable_width as f32, self.drawable_height as f32)
        }
    }
}

impl PartialEq for PresentSurface {
    fn eq(&self, other: &Self) -> bool {
        self.drawable_width == other.drawable_width
            && self.drawable_height == other.drawable_height
            && self.device_pixel_ratio.to_bits() == other.device_pixel_ratio.to_bits()
            && self.transform == other.transform
            && self.generation == other.generation
    }
}

impl Eq for PresentSurface {}

/// Identity of the drawable swapchain image acquired for the current frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PresentImage {
    /// 当前取得的交换链图像索引。
    pub index: usize,
    /// 交换链包含的图像总数。
    pub image_count: usize,
}

impl PresentImage {
    /// 创建交换链图像身份。
    pub const fn new(index: usize, image_count: usize) -> Self {
        Self { index, image_count }
    }

    fn is_valid(self) -> bool {
        self.image_count > 0
            && self.image_count <= MAX_TRACKED_PRESENT_IMAGES
            && self.index < self.image_count
    }
}

/// Required draw repair and compositor damage for one presentation.
///
/// A tracked swapchain caller must arrange for every pixel in `draw_damage`
/// to be current in the acquired image before submitting `present_damage`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentDamagePlan {
    /// 调用方必须修复或重绘的区域。
    pub draw_damage: PresentDamage,
    /// 提交给原生呈现接口的损伤区域。
    pub present_damage: PresentDamage,
}

impl PresentDamagePlan {
    /// 创建完整绘制和完整提交计划。
    pub fn full() -> Self {
        Self {
            draw_damage: PresentDamage::Full,
            present_damage: PresentDamage::Full,
        }
    }

    fn partial(damage: PresentDamage) -> Self {
        Self {
            draw_damage: damage.clone(),
            present_damage: damage,
        }
    }
}

/// 已绑定本帧 surface、image 与原始损伤的内部呈现计划。
///
/// 原生 present 失败时直接丢弃；只有成功路径可以把它交给
/// [`PresentDamageTracker::commit_prepared`] 推进历史。
#[derive(Debug)]
pub(crate) struct PreparedPresentDamage {
    plan: PresentDamagePlan,
    commit: PreparedPresentCommit,
}

/// 原生 present 成功后唯一可消费的历史提交事实。
#[derive(Debug)]
pub(crate) struct PreparedPresentCommit {
    coherency: PresentCoherency,
    surface: PresentSurface,
    image: Option<PresentImage>,
    current: PresentDamage,
}

impl PreparedPresentDamage {
    /// 拆出可移动的损伤计划与成功后唯一可消费的提交令牌。
    pub(crate) fn into_parts(self) -> (PresentDamagePlan, PreparedPresentCommit) {
        (self.plan, self.commit)
    }
}

#[derive(Debug, Clone)]
struct CommittedPresentDamage {
    sequence: u64,
    damage: PresentDamage,
}

/// Successful-present history for retained and multi-image targets.
///
/// Call [`Self::plan`] before drawing/presenting and call [`Self::commit`]
/// only after the native present succeeds. A missing image identity, changed
/// surface signature, or invalid metadata always produces a full plan.
#[derive(Debug, Clone, Default)]
pub struct PresentDamageTracker {
    surface: Option<PresentSurface>,
    coherency: Option<PresentCoherency>,
    sequence: u64,
    image_sequences: Vec<Option<u64>>,
    history: VecDeque<CommittedPresentDamage>,
}

impl PresentDamageTracker {
    /// 创建尚无任何成功呈现历史的跟踪器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 清除表面、图像和损伤历史。
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// 根据缓冲保留能力和成功呈现历史规划本帧损伤。
    pub fn plan(
        &self,
        coherency: PresentCoherency,
        surface: PresentSurface,
        image: Option<PresentImage>,
        logical_damage: &DamageRegion,
    ) -> PresentDamagePlan {
        if coherency == PresentCoherency::FullOnly || !surface.is_valid() {
            return PresentDamagePlan::full();
        }
        let current = logical_damage.to_present_damage(surface);
        self.plan_present_damage(coherency, surface, image, current)
    }

    /// 一次完成逻辑损伤转换，并把成功提交所需事实绑定到不可克隆令牌。
    pub(crate) fn prepare(
        &self,
        coherency: PresentCoherency,
        surface: PresentSurface,
        image: Option<PresentImage>,
        logical_damage: &DamageRegion,
    ) -> PreparedPresentDamage {
        let current = if coherency == PresentCoherency::FullOnly || !surface.is_valid() {
            PresentDamage::Full
        } else {
            logical_damage.to_present_damage(surface)
        };
        let plan = self.plan_present_damage(coherency, surface, image, current.clone());
        PreparedPresentDamage {
            plan,
            commit: PreparedPresentCommit {
                coherency,
                surface,
                image,
                current,
            },
        }
    }

    fn plan_present_damage(
        &self,
        coherency: PresentCoherency,
        surface: PresentSurface,
        image: Option<PresentImage>,
        current: PresentDamage,
    ) -> PresentDamagePlan {
        if coherency == PresentCoherency::FullOnly || !surface.is_valid() {
            return PresentDamagePlan::full();
        }
        if current.is_full() || self.surface != Some(surface) || self.coherency != Some(coherency) {
            return PresentDamagePlan::full();
        }

        match coherency {
            PresentCoherency::FullOnly => PresentDamagePlan::full(),
            PresentCoherency::RetainedBuffer => PresentDamagePlan::partial(current),
            PresentCoherency::TrackedSwapchain => {
                let Some(image) = image.filter(|image| image.is_valid()) else {
                    return PresentDamagePlan::full();
                };
                if self.image_sequences.len() != image.image_count {
                    return PresentDamagePlan::full();
                }
                let Some(last_sequence) = self.image_sequences[image.index] else {
                    return PresentDamagePlan::full();
                };
                let mut required = current;
                for committed in self
                    .history
                    .iter()
                    .filter(|committed| committed.sequence > last_sequence)
                {
                    required = required.union(&committed.damage);
                    if required.is_full() {
                        return PresentDamagePlan::full();
                    }
                }
                PresentDamagePlan::partial(required)
            }
        }
    }

    /// Records the logical changes from a successful native present.
    ///
    /// The caller must have honored the `draw_damage` returned by the matching
    /// [`Self::plan`]. Failed or skipped presents must not call this method.
    pub fn commit(
        &mut self,
        coherency: PresentCoherency,
        surface: PresentSurface,
        image: Option<PresentImage>,
        logical_damage: &DamageRegion,
    ) {
        if coherency == PresentCoherency::FullOnly || !surface.is_valid() {
            self.reset();
            return;
        }

        let current = logical_damage.to_present_damage(surface);
        self.commit_present_damage(coherency, surface, image, current);
    }

    /// 仅在匹配的原生 present 成功后消费准备令牌并推进历史。
    pub(crate) fn commit_prepared(&mut self, prepared: PreparedPresentCommit) {
        self.commit_present_damage(
            prepared.coherency,
            prepared.surface,
            prepared.image,
            prepared.current,
        );
    }

    fn commit_present_damage(
        &mut self,
        coherency: PresentCoherency,
        surface: PresentSurface,
        image: Option<PresentImage>,
        current: PresentDamage,
    ) {
        if coherency == PresentCoherency::FullOnly || !surface.is_valid() {
            self.reset();
            return;
        }
        let state_changed = self.surface != Some(surface) || self.coherency != Some(coherency);
        if state_changed {
            self.reset();
            self.surface = Some(surface);
            self.coherency = Some(coherency);
        }

        match coherency {
            PresentCoherency::FullOnly => self.reset(),
            PresentCoherency::RetainedBuffer => {
                self.sequence = 0;
                self.image_sequences.clear();
                self.history.clear();
            }
            PresentCoherency::TrackedSwapchain => {
                let Some(image) = image.filter(|image| image.is_valid()) else {
                    self.reset();
                    return;
                };
                if self.image_sequences.len() != image.image_count {
                    self.sequence = 0;
                    self.image_sequences = vec![None; image.image_count];
                    self.history.clear();
                }
                let Some(sequence) = self.sequence.checked_add(1) else {
                    self.sequence = 0;
                    self.image_sequences.fill(None);
                    self.history.clear();
                    return;
                };
                self.sequence = sequence;
                self.history.push_back(CommittedPresentDamage {
                    sequence,
                    damage: current,
                });
                self.image_sequences[image.index] = Some(sequence);

                if let Some(oldest_required) = self.image_sequences.iter().flatten().copied().min()
                {
                    while self
                        .history
                        .front()
                        .is_some_and(|committed| committed.sequence <= oldest_required)
                    {
                        self.history.pop_front();
                    }
                }
                if self.history.len() > MAX_PRESENT_HISTORY_FRAMES {
                    // Losing bounded history invalidates every image proof.
                    self.sequence = 0;
                    self.image_sequences.fill(None);
                    self.history.clear();
                }
            }
        }
    }
}

fn logical_rect_to_drawable(
    rect: Rect,
    surface: PresentSurface,
) -> Result<Option<(i32, i32, i32, i32)>, ()> {
    if !rect.x.is_finite() || !rect.y.is_finite() || !rect.w.is_finite() || !rect.h.is_finite() {
        return Err(());
    }
    if rect.w <= 0.0 || rect.h <= 0.0 {
        return Ok(None);
    }

    let dpr = surface.device_pixel_ratio;
    let x0 = rect.x * dpr;
    let y0 = rect.y * dpr;
    let x1 = (rect.x + rect.w) * dpr;
    let y1 = (rect.y + rect.h) * dpr;
    if !x0.is_finite() || !y0.is_finite() || !x1.is_finite() || !y1.is_finite() {
        return Err(());
    }

    let (source_w, source_h) = surface.untransformed_extent();
    let (tx0, ty0, tx1, ty1) = match surface.transform {
        PresentTransform::Identity => (x0, y0, x1, y1),
        PresentTransform::Rotate90 => (source_h - y1, x0, source_h - y0, x1),
        PresentTransform::Rotate180 => (source_w - x1, source_h - y1, source_w - x0, source_h - y0),
        PresentTransform::Rotate270 => (y0, source_w - x1, y1, source_w - x0),
        PresentTransform::HorizontalMirror => (source_w - x1, y0, source_w - x0, y1),
        PresentTransform::HorizontalMirrorRotate90 => {
            (source_h - y1, source_w - x1, source_h - y0, source_w - x0)
        }
        PresentTransform::HorizontalMirrorRotate180 => (x0, source_h - y1, x1, source_h - y0),
        PresentTransform::HorizontalMirrorRotate270 => (y0, x0, y1, x1),
    };

    let drawable_w = surface.drawable_width as f32;
    let drawable_h = surface.drawable_height as f32;
    let left = tx0.min(tx1).floor().clamp(0.0, drawable_w) as i32;
    let top = ty0.min(ty1).floor().clamp(0.0, drawable_h) as i32;
    let right = tx0.max(tx1).ceil().clamp(0.0, drawable_w) as i32;
    let bottom = ty0.max(ty1).ceil().clamp(0.0, drawable_h) as i32;
    if right <= left || bottom <= top {
        Ok(None)
    } else {
        Ok(Some((left, top, right - left, bottom - top)))
    }
}

fn normalize_present_rects(
    mut rects: Vec<(i32, i32, i32, i32)>,
) -> Option<Vec<(i32, i32, i32, i32)>> {
    rects.retain(|&(_, _, w, h)| w > 0 && h > 0);
    if rects.len() > MAX_PRESENT_DAMAGE_RECTS {
        return None;
    }
    let mut i = 0;
    while i < rects.len() {
        let mut j = i + 1;
        while j < rects.len() {
            if let Some(union) = union_if_touching(rects[i], rects[j]) {
                rects[i] = union;
                rects.swap_remove(j);
                j = i + 1;
            } else {
                j += 1;
            }
        }
        i += 1;
    }
    rects.sort_unstable_by_key(|&(x, y, w, h)| (y, x, h, w));
    Some(rects)
}

fn union_if_touching(
    lhs: (i32, i32, i32, i32),
    rhs: (i32, i32, i32, i32),
) -> Option<(i32, i32, i32, i32)> {
    let (lx, ly, lw, lh) = lhs;
    let (rx, ry, rw, rh) = rhs;
    let l_right = lx.saturating_add(lw);
    let l_bottom = ly.saturating_add(lh);
    let r_right = rx.saturating_add(rw);
    let r_bottom = ry.saturating_add(rh);
    if l_right < rx || r_right < lx || l_bottom < ry || r_bottom < ly {
        return None;
    }
    let x0 = lx.min(rx);
    let y0 = ly.min(ry);
    let x1 = l_right.max(r_right);
    let y1 = l_bottom.max(r_bottom);
    Some((x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0)))
}

// 验证 tracked swapchain 的首帧、历史修复、失败提交与重建门禁。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../tests/unit/core/damage/mod__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
