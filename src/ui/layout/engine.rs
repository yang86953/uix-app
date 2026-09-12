//! 统一布局引擎 — FlexBox 和 Grid 的组件化封装。
//!
//! 将原本的纯函数 compute_flex_layout / compute_grid_layout 重新设计为
//! 可被组合使用的布局组件，提供统一的 LayoutEngine trait 接口。
//!
//! 盒模型计算也统一在此层，所有容器组件通过 BoxModel 获得一致的内框计算。

use super::flex::compute_flex_layout_into;
use super::grid::{GridComputeScratch, compute_grid_layout_into};
use super::{AlignItems, FlexChild, FlexComputeScratch, FlexDirection, FlexInput, JustifyContent};
use super::{GridChild, GridInput, GridTrack};
use crate::core::{EdgeInsets, Rect, Size, WidgetId};

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

    /// 内容固有尺寸转换为 border-box；空内容也保留 border 与 padding，绝不加 margin。
    pub fn border_box_size(&self, content: Size) -> Size {
        let content = normalize_layout_size(content);
        let border = normalize_non_negative_insets(self.border_width);
        let padding = normalize_non_negative_insets(self.padding);
        normalize_layout_size(Size::new(
            content.w + border.horizontal() + padding.horizontal(),
            content.h + border.vertical() + padding.vertical(),
        ))
    }

    // 声明宽高统一指 border-box；零仍保留 UIX 既有 auto 哨兵语义。
    pub fn preferred_size(
        &self,
        content: Size,
        width: Option<f32>,
        height: Option<f32>,
    ) -> Size {
        let natural = self.border_box_size(content);
        let minimum = self.border_box_size(Size::zero());
        let declared = |value: Option<f32>| {
            value.filter(|value| value.is_finite() && *value > 0.0 && *value < f32::MAX)
        };
        Size::new(
            declared(width).unwrap_or(natural.w).max(minimum.w),
            declared(height).unwrap_or(natural.h).max(minimum.h),
        )
    }

    // 零内容基值仍占用不可压缩的 padding 与 border，仅由已知父主轴选择分量。
    pub fn flex_basis(
        &self,
        direction: FlexDirection,
        grows: bool,
        width: Option<f32>,
        height: Option<f32>,
    ) -> Option<f32> {
        let (declared, minimum) = match direction {
            FlexDirection::Row | FlexDirection::RowReverse => {
                (width, self.border_box_size(Size::zero()).w)
            }
            FlexDirection::Column | FlexDirection::ColumnReverse => {
                (height, self.border_box_size(Size::zero()).h)
            }
        };
        (grows
            && !declared.is_some_and(|value| value.is_finite() && value > 0.0 && value < f32::MAX))
        .then_some(minimum)
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
    /// 独立的父主轴弹性基值；None 使用 measured_size 的对应分量。
    pub flex_basis: Option<f32>,
    /// border-box 不可收缩的最小尺寸（例如 padding + border）。
    pub min_size: Size,
    /// border-box 不可拉伸超过的最大尺寸；无上限用 [`Size::infinite`]。
    pub max_size: Size,
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
            flex_basis: None,
            min_size: Size::zero(),
            max_size: Size::infinite(),
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
    pub flex_children: Vec<FlexChild>,
    pub flex: FlexComputeScratch,
    grid: Option<Box<GridLayoutScratch>>,
}

/// 仅在组件树实际包含 Grid 时创建的树级求解工作区。
#[derive(Default)]
pub struct GridLayoutScratch {
    pub layout_children: Vec<LayoutChild>,
    pub visual_order: Vec<usize>,
    pub children: Vec<GridChild>,
    pub compute: GridComputeScratch,
}

impl LayoutEngineScratch {
    /// 惰性取得 Grid 工作区，避免普通 Flex 树承担全部 Grid 缓冲头。
    pub fn grid(&mut self) -> &mut GridLayoutScratch {
        self.grid
            .get_or_insert_with(|| Box::new(GridLayoutScratch::default()))
    }
}

/// 从已放置子 frame 与物理尾侧 margin 计算内容占用尺寸。
pub fn content_size_from_children(
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
pub fn finite_or_zero(value: f32) -> f32 {
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
pub fn finite_non_negative(value: f32) -> f32 {
    // 先清除非有限值和测量哨兵，再钳制负尺寸。
    finite_or_zero(value).max(0.0)
}

// 把布局坐标吸附到千分之一像素网格。
//
// 收敛循环以 `frame == new_frame` 的精确比较判定稳定；不同求和顺序产生的
// 1e-6 级浮点噪声会穿透归一并让 `any_change` 永真，布局每帧打满收敛上限，
// 最后一轮的位置调整失去后续修正机会，部分节点 frame 停留在中间态
// （uix-app#91）。千分之一像素远低于渲染可感知阈值，只吞噬比较噪声。
fn snap_subpixel(value: f32) -> f32 {
    let snapped = (value * 1000.0).round() / 1000.0;
    // 抖动值仍需经过有限值清洗，防止 NaN/无穷穿透吸附。
    finite_or_zero(snapped)
}

// 归一最终或待求解的矩形。
pub(crate) fn normalize_layout_rect(rect: Rect) -> Rect {
    // Rect::new 继续承担 NaN 安全构造，传入值已满足布局层更强契约。
    Rect::new(
        snap_subpixel(finite_or_zero(rect.x)),
        snap_subpixel(finite_or_zero(rect.y)),
        snap_subpixel(finite_non_negative(rect.w)),
        snap_subpixel(finite_non_negative(rect.h)),
    )
}

// 归一最终或待求解的尺寸。
pub fn normalize_layout_size(size: Size) -> Size {
    // 两个轴都不得把测量阶段的无界哨兵写入实际输出。
    Size::new(finite_non_negative(size.w), finite_non_negative(size.h))
}

// 归一允许负值语义的外边距。
pub fn normalize_margin(insets: EdgeInsets) -> EdgeInsets {
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
    pub fn layout_into(
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
            if self.intrinsic_cross {
                match self.direction {
                    FlexDirection::Row | FlexDirection::RowReverse => total_size.h = 0.0,
                    FlexDirection::Column | FlexDirection::ColumnReverse => total_size.w = 0.0,
                }
            }
            return normalize_layout_size(total_size);
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
                // 溢出模式保留自然主尺寸，不读取正常流 grow 的零基值。
                flex_basis: if self.overflow_content {
                    None
                } else {
                    child.flex_basis
                },
                min_size: normalize_layout_size(child.min_size),
                // 上限保留无界哨兵，由求解器按轴归一。
                max_size: child.max_size,
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
        let mut grid_children = Vec::new();
        let mut scratch = GridComputeScratch::default();
        let total_size = self.layout_with_tracks_into(
            content_rect,
            columns,
            rows,
            children,
            &mut grid_children,
            &mut scratch,
        );
        LayoutOutput {
            positions: std::mem::take(&mut scratch.child_rects),
            total_size,
        }
    }

    pub fn layout_with_tracks_into(
        &self,
        content_rect: Rect,
        columns: &[GridTrack],
        rows: &[GridTrack],
        children: &[LayoutChild],
        grid_children: &mut Vec<GridChild>,
        scratch: &mut GridComputeScratch,
    ) -> Size {
        // Grid 与 Flex 共享同一实际 frame 输入边界。
        let content_rect = normalize_layout_rect(content_rect);
        if columns.is_empty() || children.is_empty() {
            // 空 track 或空子集不能把父级无界哨兵直传到输出。
            scratch.child_rects.clear();
            return Size::new(content_rect.w, content_rect.h);
        }

        grid_children.clear();
        grid_children.extend(children.iter().map(|c| GridChild {
            cell: c.grid_cell,
            col_span: c.grid_column_span,
            row_span: c.grid_row_span,
            // 子项测量哨兵不能进入 cell 尺寸与对齐算术。
            measured_size: normalize_layout_size(c.measured_size),
            min_size: normalize_layout_size(c.min_size),
            // 上限保留无界哨兵，由单元格对齐按轴归一。
            max_size: c.max_size,
            // 外边距保留有限负值语义并清除非法分量。
            margin: normalize_margin(c.margin),
            // Grid 交叉轴继承公开 LayoutChild 的逐项对齐覆盖。
            align: c.align_self,
            justify: None,
        }));

        let input = GridInput {
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
            children: grid_children,
            align_items: self.align_items,
            justify_items: self.justify_items,
            // 把整组列轨对齐声明传给纯求解器。
            justify_content: self.justify_content,
        };
        let total_size = compute_grid_layout_into(&input, scratch);

        // Grid 求解结果在公开边界执行最终有限化。
        for frame in &mut scratch.child_rects {
            *frame = normalize_layout_rect(*frame);
        }
        normalize_layout_size(total_size)
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
