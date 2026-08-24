//! 统一布局引擎 — FlexBox 和 Grid 的组件化封装。
//!
//! 将原本的纯函数 compute_flex_layout / compute_grid_layout 重新设计为
//! 可被组合使用的布局组件，提供统一的 LayoutEngine trait 接口。
//!
//! 盒模型计算也统一在此层，所有容器组件通过 BoxModel 获得一致的内框计算。

use super::flex::compute_flex_layout_into;
use super::grid::compute_grid_layout;
use super::{AlignItems, FlexChild, FlexComputeScratch, FlexDirection, FlexInput, JustifyContent};
use super::{GridChild, GridInput, GridTrack};
use crate::core::{EdgeInsets, Rect, Size, WidgetId};
use crate::draw::geometry::spatial::AABB3D;

// ── 统一盒模型 ────────────────────────────────────────────────────

/// 统一盒模型 — 所有容器共享的 margin/border/padding 计算。
///
/// 父布局先消费 margin；本层从 border-box frame 扣除 border 与 padding 得到内容区域。
#[derive(Debug, Clone, Copy)]
pub struct BoxModel {
    /// border-box 外侧由父布局消费的外边距。
    pub margin: EdgeInsets,
    /// border-box 内侧各边的边框厚度。
    pub border_width: EdgeInsets,
    /// 边框与内容区域之间的内边距。
    pub padding: EdgeInsets,
}

impl BoxModel {
    /// 不包含外边距、边框和内边距的零值盒模型。
    pub const ZERO: Self = Self {
        margin: EdgeInsets::zero(),
        border_width: EdgeInsets::zero(),
        padding: EdgeInsets::zero(),
    };

    /// 从子项 frame 计算内容区（扣除 border + padding）。
    ///
    /// `frame` 是父级 flex/grid 分配的 border-box：**不含 margin**
    ///（margin 已由父级在放置时计入间距）。此处再扣 margin 会双重缩进。
    pub fn content_rect(&self, frame: Rect) -> Rect {
        // 先把测量哨兵和非法 frame 收敛为可写入布局树的实际几何。
        let frame = normalize_layout_rect(frame);
        // border 只能是有限非负厚度。
        let border = normalize_non_negative_insets(self.border_width);
        // padding 只能是有限非负厚度。
        let padding = normalize_non_negative_insets(self.padding);
        // 横向边框总量也要防止有限大数相加溢出。
        let border_horizontal = finite_non_negative(border.horizontal());
        // 纵向边框总量也要防止有限大数相加溢出。
        let border_vertical = finite_non_negative(border.vertical());
        // 横向内边距总量也要防止有限大数相加溢出。
        let padding_horizontal = finite_non_negative(padding.horizontal());
        // 纵向内边距总量也要防止有限大数相加溢出。
        let padding_vertical = finite_non_negative(padding.vertical());
        // 计算后再次归一，阻止坐标加法或尺寸减法溢出到最终 frame。
        normalize_layout_rect(Rect::new(
            frame.x + border.left + padding.left,
            frame.y + border.top + padding.top,
            (frame.w - border_horizontal - padding_horizontal).max(0.0),
            (frame.h - border_vertical - padding_vertical).max(0.0),
        ))
    }

    /// 视觉区域 = border-box（与 frame 同；margin 在 frame 外由父级留白）。
    pub fn visual_rect(&self, frame: Rect) -> Rect {
        // 视觉入口同样不得重新物化无界哨兵或非有限 frame。
        normalize_layout_rect(frame)
    }
}

// ── 统一子节点信息 ─────────────────────────────────────────────────

/// 统一子节点布局信息，Flex 和 Grid 引擎共用。
///
/// grid_* 字段仅在 Grid 引擎中使用（Flex 引擎忽略）。
///
/// # Margin 参与布局
///
/// `margin` 占用了主轴和交叉轴的空间。布局引擎计算子节点位置时：
/// - 主轴方向：子节点的占位 = measured_size.main + margin.main_axis_sum
/// - 分配 frame 时：子节点起始位置 = cursor + margin.start (主轴方向)
/// - 分配 frame 时：交叉轴起始位置 = cross_offset + margin.cross_start
#[derive(Debug, Clone)]
pub struct LayoutChild {
    /// 子组件在布局树中的稳定标识。
    pub id: WidgetId,
    /// 子组件在当前约束下测得的自然尺寸。
    pub measured_size: Size,
    /// 主轴存在剩余空间时的伸展权重。
    pub flex_grow: f32,
    /// 主轴空间不足时的收缩权重。
    pub flex_shrink: f32,
    /// 外边距——参与布局计算，推开兄弟节点
    pub margin: crate::core::EdgeInsets,
    /// 子项交叉轴对齐覆盖；None 时继承父容器 align_items。
    pub align_self: Option<AlignItems>,
    /// Grid: 起始单元格索引
    pub grid_cell: Option<usize>,
    /// Grid: 列跨度
    pub grid_column_span: u32,
    /// Grid: 行跨度
    pub grid_row_span: u32,
}

impl LayoutChild {
    /// 使用组件标识和自然尺寸创建默认布局子项。
    pub fn new(id: WidgetId, measured_size: Size) -> Self {
        Self {
            id,
            measured_size,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            margin: crate::core::EdgeInsets::zero(),
            align_self: None,
            grid_cell: None,
            grid_column_span: 1,
            grid_row_span: 1,
        }
    }

    /// 设置 Flex 伸展与收缩权重。
    pub fn with_flex(mut self, grow: f32, shrink: f32) -> Self {
        self.flex_grow = grow;
        self.flex_shrink = shrink;
        self
    }

    /// 将子项放入指定的 Grid 起始单元格。
    pub fn with_grid_cell(mut self, cell: usize) -> Self {
        self.grid_cell = Some(cell);
        self
    }

    /// 设置子项跨越的 Grid 列数与行数，最小值均为一。
    pub fn with_grid_span(mut self, columns: u32, rows: u32) -> Self {
        self.grid_column_span = columns.max(1);
        self.grid_row_span = rows.max(1);
        self
    }

    /// 主轴方向的外边距总和（用于布局占位计算）。
    pub fn margin_main(&self, direction: FlexDirection) -> f32 {
        match direction {
            FlexDirection::Row | FlexDirection::RowReverse => self.margin.left + self.margin.right,
            FlexDirection::Column | FlexDirection::ColumnReverse => {
                self.margin.top + self.margin.bottom
            }
        }
    }

    /// 起点方向的外边距（用于布局时设置子节点起始位置）。
    pub fn margin_start(&self, direction: FlexDirection) -> f32 {
        match direction {
            FlexDirection::Row => self.margin.left,
            FlexDirection::RowReverse => self.margin.right,
            FlexDirection::Column => self.margin.top,
            FlexDirection::ColumnReverse => self.margin.bottom,
        }
    }

    /// 交叉轴起始方向的外边距。
    pub fn margin_cross_start(&self, direction: FlexDirection) -> f32 {
        match direction {
            FlexDirection::Row | FlexDirection::RowReverse => self.margin.top,
            FlexDirection::Column | FlexDirection::ColumnReverse => self.margin.left,
        }
    }
}

// ── 布局引擎 trait ─────────────────────────────────────────────────

/// 统一布局引擎 trait — Flex 和 Grid 的公共抽象。
pub trait LayoutEngine {
    /// 在内容区域内计算子节点位置。
    fn layout(&self, content_rect: Rect, children: &[LayoutChild]) -> LayoutOutput;
}

/// 布局引擎的输出：子节点位置 + 内容总尺寸。
#[derive(Debug, Clone)]
pub struct LayoutOutput {
    /// 与输入子项顺序一致的 border-box 布局矩形。
    pub positions: Vec<Rect>,
    /// 布局内容在两个轴向上的总占用尺寸。
    pub total_size: Size,
}

/// 真实组件树在同一布局帧内跨容器复用的求解工作区。
#[doc(hidden)]
#[derive(Default)]
pub struct LayoutEngineScratch {
    pub(crate) layout_children: Vec<LayoutChild>,
    pub(crate) flex_children: Vec<FlexChild>,
    pub(crate) flex: FlexComputeScratch,
}

impl LayoutOutput {
    // 在共享引擎边界统一收敛最终 frame 与总尺寸。
    fn normalized(mut self) -> Self {
        // 逐项清除算法累加产生的非有限值或测量哨兵。
        for frame in &mut self.positions {
            // 保留有效几何，仅替换不能写入布局树的分量。
            *frame = normalize_layout_rect(*frame);
        }
        // 总尺寸也必须是有限非负的实际值。
        self.total_size = normalize_layout_size(self.total_size);
        // 返回保持子项数量和顺序不变的输出。
        self
    }
}

/// 从已放置子 frame 与物理尾侧 margin 计算内容占用尺寸。
pub(crate) fn content_size_from_children(
    origin: Rect,
    positions: &[Rect],
    children: &[LayoutChild],
) -> Size {
    // 横向内容范围保留可见 frame，并把正右 margin 计入占位末端。
    let content_w = positions
        .iter()
        .zip(children)
        .map(|(rect, child)| {
            // 负右 margin 只改变兄弟推进，不得裁掉子项自身可见宽度。
            let trailing_margin = finite_or_zero(child.margin.right).max(0.0);
            // 将绝对坐标末端转换为相对内容原点的有限非负尺寸。
            finite_non_negative(rect.x + rect.w + trailing_margin - origin.x)
        })
        .fold(0.0, f32::max);
    // 纵向内容范围保留可见 frame，并把正下 margin 计入占位末端。
    let content_h = positions
        .iter()
        .zip(children)
        .map(|(rect, child)| {
            // 负下 margin 只改变兄弟推进，不得裁掉子项自身可见高度。
            let trailing_margin = finite_or_zero(child.margin.bottom).max(0.0);
            // 将绝对坐标末端转换为相对内容原点的有限非负尺寸。
            finite_non_negative(rect.y + rect.h + trailing_margin - origin.y)
        })
        .fold(0.0, f32::max);
    // 返回可供容器下一轮 measure 使用的物理内容尺寸。
    Size::new(content_w, content_h)
}

// 把坐标值限制为有限且不是 f32::MAX 测量哨兵的实际值。
pub(crate) fn finite_or_zero(value: f32) -> f32 {
    // f32::MAX 及其负值不能进入实际 frame，非有限值同样回退为零。
    if value.is_finite() && value.abs() < f32::MAX {
        // 有效坐标保留原值，允许布局溢出产生有限负位置。
        value
    } else {
        // 非法坐标采用稳定零值。
        0.0
    }
}

// 把尺寸值限制为有限、非负且不是无界哨兵的实际值。
pub(crate) fn finite_non_negative(value: f32) -> f32 {
    // 先清除非有限值和测量哨兵，再钳制负尺寸。
    finite_or_zero(value).max(0.0)
}

// 归一最终或待求解的矩形。
pub(crate) fn normalize_layout_rect(rect: Rect) -> Rect {
    // Rect::new 继续承担 NaN 安全构造，传入值已满足布局层更强契约。
    Rect::new(
        finite_or_zero(rect.x),
        finite_or_zero(rect.y),
        finite_non_negative(rect.w),
        finite_non_negative(rect.h),
    )
}

// 归一最终或待求解的尺寸。
pub(crate) fn normalize_layout_size(size: Size) -> Size {
    // 两个轴都不得把测量阶段的无界哨兵写入实际输出。
    Size::new(finite_non_negative(size.w), finite_non_negative(size.h))
}

// 归一允许负值语义的外边距。
pub(crate) fn normalize_margin(insets: EdgeInsets) -> EdgeInsets {
    // 负外边距保持既有语义，只清除非有限值和无界哨兵。
    EdgeInsets::new(
        finite_or_zero(insets.left),
        finite_or_zero(insets.top),
        finite_or_zero(insets.right),
        finite_or_zero(insets.bottom),
    )
}

// 归一不允许负值语义的 border/padding。
fn normalize_non_negative_insets(insets: EdgeInsets) -> EdgeInsets {
    // 各边独立钳制，避免一条非法边污染另一轴的内容尺寸。
    EdgeInsets::new(
        finite_non_negative(insets.left),
        finite_non_negative(insets.top),
        finite_non_negative(insets.right),
        finite_non_negative(insets.bottom),
    )
}

/// 3D 感知的布局输出：子节点位置（含 z 深度） + 内容总尺寸。
///
/// `z` 用于 3D 空间中的深度排序和透视变换。
/// 默认 z=0。
// 原公开面遗留（SMC-04 模块收口后无内部消费方；保留供外部集成）。
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct LayoutOutput3D {
    pub positions: Vec<Rect>,
    pub z_values: Vec<f32>,
    pub total_size: Size,
}

// 原公开面遗留（SMC-04 模块收口后无内部消费方；保留供外部集成）。
#[allow(dead_code)]
impl LayoutOutput3D {
    /// 从 LayoutOutput + z 值创建。
    pub(crate) fn from_2d(output: &LayoutOutput, z: f32) -> Self {
        let count = output.positions.len();
        Self {
            positions: output.positions.clone(),
            z_values: vec![z; count],
            total_size: output.total_size,
        }
    }

    /// 每个子节点独立 z 值。
    pub(crate) fn from_2d_zipped(output: &LayoutOutput, z_values: Vec<f32>) -> Self {
        Self {
            positions: output.positions.clone(),
            z_values,
            total_size: output.total_size,
        }
    }

    /// 转换为 AABB3D 列表（所有盒子 d=0）。
    pub(crate) fn to_aabbs(&self) -> Vec<AABB3D> {
        self.positions
            .iter()
            .zip(self.z_values.iter())
            .map(|(r, z)| AABB3D::from_rect_z(r.x, r.y, r.w, r.h, *z, 0.0))
            .collect()
    }
}

// ── FlexBox 布局引擎 ───────────────────────────────────────────────

/// FlexBox 布局引擎组件。
///
/// 封装 FlexInput 构建和 compute_flex_layout 调用，
/// 可被任何容器 widget 组合使用。
#[derive(Debug, Clone)]
pub struct FlexLayout {
    /// 子项排列使用的主轴方向。
    pub direction: FlexDirection,
    /// 相邻子项之间的固定间距。
    pub gap: f32,
    /// 子项在主轴剩余空间中的分布方式。
    pub justify: JustifyContent,
    /// 子项在交叉轴上的默认对齐方式。
    pub align: AlignItems,
    /// 是否允许子项换行或换列。
    pub wrap: bool,
    /// 允许内容溢出（跳过 flex-shrink，子节点按自然尺寸流式堆叠）
    pub overflow_content: bool,
    /// 主轴无显式尺寸时由子项撑开，仍走标准 flex-grow 路径。
    pub intrinsic_main: bool,
    /// 交叉轴无显式尺寸时由子项自然外尺寸撑开。
    pub intrinsic_cross: bool,
}

impl FlexLayout {
    /// 创建垂直排列、无间距且默认拉伸子项的 Flex 布局。
    pub fn new() -> Self {
        Self {
            direction: FlexDirection::Column,
            gap: 0.0,
            justify: JustifyContent::Start,
            align: AlignItems::Stretch,
            wrap: false,
            overflow_content: false,
            intrinsic_main: false,
            // 默认保持父级已确定交叉轴的兼容语义。
            intrinsic_cross: false,
        }
    }

    /// 便捷构造：垂直堆叠。
    pub fn column() -> Self {
        Self::new()
    }

    /// 便捷构造：水平排列。
    pub fn row() -> Self {
        Self {
            direction: FlexDirection::Row,
            ..Self::new()
        }
    }

    /// 设置子项排列使用的主轴方向。
    pub fn with_direction(mut self, d: FlexDirection) -> Self {
        self.direction = d;
        self
    }
    /// 设置相邻子项之间的固定间距。
    pub fn with_gap(mut self, g: f32) -> Self {
        self.gap = g;
        self
    }
    /// 设置子项在主轴剩余空间中的分布方式。
    pub fn with_justify(mut self, j: JustifyContent) -> Self {
        self.justify = j;
        self
    }
    /// 设置子项在交叉轴上的默认对齐方式。
    pub fn with_align(mut self, a: AlignItems) -> Self {
        self.align = a;
        self
    }
}

impl Default for FlexLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl FlexLayout {
    /// 把布局结果写入调用方持有的工作区，避免稳定布局逐容器申请数组。
    pub(crate) fn layout_into(
        &self,
        content_rect: Rect,
        children: &[LayoutChild],
        scratch: &mut LayoutEngineScratch,
    ) -> Size {
        let content_rect = normalize_layout_rect(content_rect);
        if children.is_empty() {
            scratch.flex.child_rects.clear();
            let mut total_size = Size::new(content_rect.w, content_rect.h);
            if self.intrinsic_main || self.overflow_content {
                match self.direction {
                    FlexDirection::Row | FlexDirection::RowReverse => total_size.w = 0.0,
                    FlexDirection::Column | FlexDirection::ColumnReverse => total_size.h = 0.0,
                }
            }
            return normalize_layout_size(total_size);
        }

        if self.overflow_content && !self.wrap {
            let output = overflow_layout(self, content_rect, children).normalized();
            scratch.flex.child_rects = output.positions;
            return output.total_size;
        }

        scratch.flex_children.clear();
        scratch
            .flex_children
            .extend(children.iter().map(|child| FlexChild {
                flex_grow: if self.overflow_content {
                    0.0
                } else {
                    finite_non_negative(child.flex_grow)
                },
                flex_shrink: if self.overflow_content {
                    0.0
                } else {
                    finite_non_negative(child.flex_shrink)
                },
                align_self: child.align_self,
                measured_size: normalize_layout_size(child.measured_size),
                margin: normalize_margin(child.margin),
                ..FlexChild::default()
            }));
        let input = FlexInput {
            direction: self.direction,
            wrap: self.wrap,
            gap: finite_or_zero(self.gap),
            padding: crate::core::EdgeInsets::zero(),
            container: content_rect,
            children: &scratch.flex_children,
            justify_content: if self.overflow_content && self.justify == JustifyContent::Stretch {
                JustifyContent::Start
            } else {
                self.justify
            },
            align_items: self.align,
            intrinsic_main: self.intrinsic_main || self.overflow_content,
            intrinsic_cross: self.intrinsic_cross,
        };
        let total_size = compute_flex_layout_into(&input, &mut scratch.flex);
        for frame in &mut scratch.flex.child_rects {
            *frame = normalize_layout_rect(*frame);
        }
        normalize_layout_size(total_size)
    }
}

impl LayoutEngine for FlexLayout {
    fn layout(&self, content_rect: Rect, children: &[LayoutChild]) -> LayoutOutput {
        let mut scratch = LayoutEngineScratch::default();
        let total_size = self.layout_into(content_rect, children, &mut scratch);
        LayoutOutput {
            positions: std::mem::take(&mut scratch.flex.child_rects),
            total_size,
        }
    }
}

/// 溢出模式：保留自然主轴尺寸，同时遵守分布、对齐与反向语义。
fn overflow_layout(
    engine: &FlexLayout,
    content_rect: Rect,
    children: &[LayoutChild],
) -> LayoutOutput {
    let is_row = matches!(
        engine.direction,
        FlexDirection::Row | FlexDirection::RowReverse
    );
    let is_reverse = matches!(
        engine.direction,
        FlexDirection::RowReverse | FlexDirection::ColumnReverse
    );
    let count = children.len();
    let gap = finite_or_zero(engine.gap);

    let container_main = if is_row {
        content_rect.w
    } else {
        content_rect.h
    };
    let container_cross = if is_row {
        content_rect.h
    } else {
        content_rect.w
    };

    let total_margin_main: f32 = children
        .iter()
        .map(|child| finite_or_zero(child.margin_main(engine.direction)))
        .sum();
    let total_main: f32 = children
        .iter()
        .map(|child| {
            finite_non_negative(if is_row {
                child.measured_size.w
            } else {
                child.measured_size.h
            })
        })
        .sum::<f32>()
        + total_margin_main
        + gap * (count as f32 - 1.0).max(0.0);
    // 固有交叉轴占位必须包含子项自然尺寸与两侧 margin。
    let max_child_cross = children
        .iter()
        .map(|child| {
            // 按布局方向读取子项自然交叉轴尺寸。
            let cross = finite_non_negative(if is_row {
                child.measured_size.h
            } else {
                child.measured_size.w
            });
            // 按布局方向读取有限交叉轴 margin 总量。
            let margin = finite_or_zero(if is_row {
                child.margin.vertical()
            } else {
                child.margin.horizontal()
            });
            // 外尺寸不得因负 margin 或异常加法变为非法值。
            finite_non_negative(cross + margin)
        })
        .fold(0.0, f32::max);
    // 零交叉轴 bootstrap 由子项自然外尺寸撑开。
    let effective_cross = if container_cross > 0.0 {
        container_cross
    } else {
        max_child_cross
    };
    // 总交叉尺寸同时覆盖父级分配与实际自然内容。
    let total_cross = effective_cross.max(max_child_cross);
    let total_size = if is_row {
        Size::new(total_main, total_cross)
    } else {
        Size::new(total_cross, total_main)
    };

    // 零尺寸 bootstrap 不偏移自然内容；实际容器保留负剩余空间供 Center/End 对齐溢出内容。
    let remaining_main = if container_main <= 1.0 {
        // 首次测量沿用自然内容起点，避免无约束对齐生成负坐标。
        0.0
    } else {
        // 实际容器内保留有限差值，让分布器区分正负剩余空间。
        finite_or_zero(container_main - total_main)
    };
    // 与标准 Flex 共享 gap 和起始偏移计算，避免两条路径语义漂移。
    let (effective_gap, start_offset) =
        super::flex::compute_justify(remaining_main, count, gap, engine.justify);
    // 反向布局也先按逻辑顺序正向放置，最后统一镜像。
    let mut cursor = start_offset;
    let mut positions = Vec::with_capacity(count);

    for child in children {
        let main = finite_non_negative(if is_row {
            child.measured_size.w
        } else {
            child.measured_size.h
        });
        let cross_size = if is_row {
            finite_non_negative(child.measured_size.h)
        } else {
            finite_non_negative(child.measured_size.w)
        };
        let margin_cross = if is_row {
            finite_or_zero(child.margin.vertical())
        } else {
            finite_or_zero(child.margin.horizontal())
        };

        let cross_align = child.align_self.unwrap_or(engine.align);
        // 交叉轴先扣除两侧 margin，再在剩余区域内执行对齐。
        let available_cross = (effective_cross - margin_cross).max(0.0);
        let child_cross = if cross_align == AlignItems::Stretch {
            // 已知交叉轴时填满可用区，bootstrap 时至少保留自然尺寸。
            if container_cross <= 1.0 {
                available_cross.max(cross_size)
            } else {
                available_cross
            }
        } else {
            cross_size
        };

        let cross_offset = match cross_align {
            AlignItems::Start => 0.0,
            AlignItems::Center => (available_cross - child_cross) / 2.0,
            AlignItems::End => available_cross - child_cross,
            AlignItems::Stretch => 0.0,
        };

        let (x, y, w, h) = if is_row {
            (
                content_rect.x + cursor + finite_or_zero(child.margin_start(engine.direction)),
                content_rect.y
                    + cross_offset
                    + finite_or_zero(child.margin_cross_start(engine.direction)),
                main,
                child_cross,
            )
        } else {
            (
                content_rect.x
                    + cross_offset
                    + finite_or_zero(child.margin_cross_start(engine.direction)),
                content_rect.y + cursor + finite_or_zero(child.margin_start(engine.direction)),
                child_cross,
                main,
            )
        };

        positions.push(Rect::new(x, y, w, h));

        let occupied_main = main + finite_or_zero(child.margin_main(engine.direction));
        cursor += occupied_main + effective_gap;
    }

    // 与标准 Flex 一致，反向方向沿容器主轴镜像已经完成的逻辑顺序。
    if is_reverse {
        // 溢出路径在零尺寸 bootstrap 始终以自然内容长度镜像，已分配 frame 使用实际主轴。
        let main_extent = if container_main <= 1.0 {
            // overflow_content 已承诺自然主轴，无需额外依赖 intrinsic_main 标记。
            finite_non_negative(total_main)
        } else {
            // 非零实际 frame 必须与前面的 justify 使用同一镜像边界。
            container_main
        };
        // 逐项镜像可同时保留 justify-content、gap 与方向侧 margin 的语义。
        for rect in &mut positions {
            if is_row {
                // 水平反向布局沿内容区右边界镜像。
                rect.x = content_rect.x + main_extent - (rect.x - content_rect.x) - rect.w;
            } else {
                // 垂直反向布局沿内容区下边界镜像。
                rect.y = content_rect.y + main_extent - (rect.y - content_rect.y) - rect.h;
            }
        }
    }

    LayoutOutput {
        positions,
        total_size,
    }
}

// ── Grid 布局引擎 ─────────────────────────────────────────────────

/// Grid 布局引擎组件。
///
/// 封装 GridInput 构建和 compute_grid_layout 调用，
/// 可被任何容器 widget 组合使用。
#[derive(Debug, Clone)]
pub struct GridLayout {
    /// 按声明顺序参与网格计算的列轨定义。
    pub columns: Vec<GridTrack>,
    /// 按声明顺序参与网格计算的行轨定义。
    pub rows: Vec<GridTrack>,
    /// 相邻列轨之间的固定间距。
    pub col_gap: f32,
    /// 相邻行轨之间的固定间距。
    pub row_gap: f32,
    /// 子项在单元格交叉轴上的默认对齐方式。
    pub align_items: AlignItems,
    /// 子项在单元格主轴上的默认对齐方式。
    pub justify_items: JustifyContent,
    /// 整组列轨在父级水平剩余空间中的对齐方式。
    pub justify_content: JustifyContent,
}

impl GridLayout {
    /// 创建不含轨道和间距的默认 Grid 布局。
    pub fn new() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            col_gap: 0.0,
            row_gap: 0.0,
            align_items: AlignItems::Stretch,
            justify_items: JustifyContent::Start,
            // 默认不移动或分散列轨组。
            justify_content: JustifyContent::Start,
        }
    }

    /// 设置按声明顺序参与计算的列轨。
    pub fn with_columns(mut self, cols: Vec<GridTrack>) -> Self {
        self.columns = cols;
        self
    }
    /// 设置按声明顺序参与计算的行轨。
    pub fn with_rows(mut self, rows: Vec<GridTrack>) -> Self {
        self.rows = rows;
        self
    }
    /// 同时设置列间距与行间距。
    pub fn with_gap(mut self, col_gap: f32, row_gap: f32) -> Self {
        self.col_gap = col_gap;
        self.row_gap = row_gap;
        self
    }
    /// 设置子项在单元格交叉轴上的默认对齐方式。
    pub fn with_align(mut self, a: AlignItems) -> Self {
        self.align_items = a;
        self
    }
    /// 设置子项在单元格主轴上的默认对齐方式。
    pub fn with_justify(mut self, j: JustifyContent) -> Self {
        self.justify_items = j;
        self
    }

    /// 设置整组 Grid 列轨在父级水平剩余空间中的对齐方式。
    pub fn with_content_justify(mut self, justify: JustifyContent) -> Self {
        // 内容对齐独立于单元格内子项对齐。
        self.justify_content = justify;
        // 返回更新后的布局构建器。
        self
    }

    pub(crate) fn layout_with_tracks(
        &self,
        content_rect: Rect,
        columns: &[GridTrack],
        rows: &[GridTrack],
        children: &[LayoutChild],
    ) -> LayoutOutput {
        // Grid 与 Flex 共享同一实际 frame 输入边界。
        let content_rect = normalize_layout_rect(content_rect);
        if columns.is_empty() || children.is_empty() {
            // 空 track 或空子集不能把父级无界哨兵直传到输出。
            return LayoutOutput {
                positions: Vec::new(),
                total_size: Size::new(content_rect.w, content_rect.h),
            }
            .normalized();
        }

        let grid_children: Vec<GridChild> = children
            .iter()
            .map(|c| GridChild {
                cell: c.grid_cell,
                col_span: c.grid_column_span,
                row_span: c.grid_row_span,
                // 子项测量哨兵不能进入 cell 尺寸与对齐算术。
                measured_size: normalize_layout_size(c.measured_size),
                // 外边距保留有限负值语义并清除非法分量。
                margin: normalize_margin(c.margin),
                // Grid 交叉轴继承公开 LayoutChild 的逐项对齐覆盖。
                align: c.align_self,
                justify: None,
            })
            .collect();

        let output = compute_grid_layout(&GridInput {
            container: content_rect,
            // 纯求解器逐值归一 track，保持普通布局继续借用列定义。
            columns,
            // 纯求解器只复制有界窗口内的显式行，并按需追加隐式 Auto 行。
            rows,
            // Grid gap 语义为非负距离。
            col_gap: finite_non_negative(self.col_gap),
            // Grid gap 语义为非负距离。
            row_gap: finite_non_negative(self.row_gap),
            padding: crate::core::EdgeInsets::zero(),
            children: &grid_children,
            align_items: self.align_items,
            justify_items: self.justify_items,
            // 把整组列轨对齐声明传给纯求解器。
            justify_content: self.justify_content,
        });

        // Grid 求解结果在公开边界执行最终有限化。
        LayoutOutput {
            positions: output.child_rects,
            total_size: output.total_size,
        }
        .normalized()
    }
}

impl Default for GridLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutEngine for GridLayout {
    fn layout(&self, content_rect: Rect, children: &[LayoutChild]) -> LayoutOutput {
        self.layout_with_tracks(content_rect, &self.columns, &self.rows, children)
    }
}
