//! Container widget — flexbox layout container with background/border.

use crate::ui::core::widget::WidgetCore;
use std::cell::Cell;

use crate::component;
use crate::core::{Constraints, EdgeInsets, Rect, Size};
use crate::draw::compositor::PicturePolicy;
use crate::draw::painting::PaintContext;
use crate::ui::layout::engine::{
    child_from_tree_with_constraints, BoxModel, FlexLayout, LayoutChild,
};
use crate::ui::layout::{AlignItems, FlexDirection, JustifyContent};
use crate::ui::style::{
    apply_style, BoxShadowDef, ColorValue, DisplayMode, Style, TypographyToken,
};
use crate::ui::traits::LayoutEngine;
use crate::ui::{ComponentId, WidgetTree};
use crate::ui::{SnapshotFields, SnapshotSource};

component! {
    /// Container — flexbox 布局容器，带背景/边框/圆角/阴影。
    ///
    /// 所有视觉效果统一通过 `style: Style` 配置。布局引擎从 `style.margin`
    /// 读取外边距参与盒模型计算。方向默认 Column（垂直堆叠）。
    ///
    /// 盒模型（与 Web CSS 一致）：
    /// - margin：外边距，由**父级** flex/grid 占用空间并偏移本节点 frame
    /// - padding：内边距，子内容在其内部排列（`content_rect` 从 frame 扣除）
    /// - border + border_radius：边框与圆角
    /// - frame = border-box（不含 margin）；勿在 content_rect 中再扣 margin
    /// - background：背景色（支持 hover/active 状态色）
    /// - box_shadow：盒阴影/辉光
    pub struct Container {
        /// 统一样式（所有视觉属性的唯一来源）
        pub style: Style,
        /// 缓存子节点内容尺寸（layout_children 后更新），
        /// 使 measure 在无固定 width/height 时能基于子节点内容估算尺寸。
        pub(crate) cached_content_size: Cell<Size>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        let intrinsic = self.intrinsic_size();
        let clamped = constraints.clamp(intrinsic);
        let cached = self.cached_content_size.get();
        let grow = self.style.flex_grow > 0.0;
        // 仅用内容缓存抵抗父级 max（Phase 2 结果）；纯 fixed 仍可被约束压小。
        let w = if cached.w > 0.0 && (self.style.width.is_some() || !grow) {
            clamped.w.max(cached.w)
        } else {
            clamped.w
        };
        let h = if cached.h > 0.0 && (self.style.height.is_some() || !grow) {
            clamped.h.max(cached.h)
        } else {
            clamped.h
        };
        Size::new(w, h)
    }

    flex_grow => (&self) -> f32 { self.style.flex_grow }

    flex_shrink => (&self) -> f32 { self.style.flex_shrink }

    align_self => (&self) -> Option<AlignItems> { self.style.align_self }

    grid_cell => (&self) -> Option<usize> { self.style.grid_cell }

    grid_column_span => (&self) -> u32 { self.style.grid_column_span }

    grid_row_span => (&self) -> u32 { self.style.grid_row_span }

    layout_margin => (&self) -> EdgeInsets { self.style.margin }

    picture_policy => (&self) -> PicturePolicy { PicturePolicy::Eligible }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        // frame 已是 border-box（父级已处理 margin）；背景画满 frame。
        let s = &self.style;
        let visual = BoxModel {
            margin: s.margin,
            border_width: s.border_width,
            padding: s.padding,
        }
        .visual_rect(frame);
        if visual.w <= 0.0 || visual.h <= 0.0 { return; }

        // 统一样式——绘制背景/边框/阴影/透明度
        apply_style(ctx, visual, s);
    }

    measure_children => (&self, frame: Rect, children: &[ComponentId], tree: &WidgetTree)
        -> Vec<LayoutChild>
    {
        let s = &self.style;
        let content_rect = BoxModel {
            margin: s.margin,
            border_width: s.border_width,
            padding: s.padding,
        }
        .content_rect(frame);
        let child_constraints = self.child_measure_constraints(content_rect);

        children
            .iter()
            .copied()
            .filter(|&cid| {
                let visible = tree.get(cid).map(|node| node.visible()).unwrap_or(true);
                if !visible {
                    crate::core::log::debug_fn(format!(
                        "[Container::measure_children] child {} is invisible, skipping",
                        cid
                    ));
                }
                visible
            })
            .map(|cid| child_from_tree_with_constraints(cid, tree, child_constraints))
            .collect()
    }

    layout_children => (&self, frame: Rect, children: &[LayoutChild], _tree: &WidgetTree)
        -> Vec<(ComponentId, Rect)>
    {
        if children.is_empty() { return Vec::new(); }

        let s = &self.style;

        // 统一的盒模型计算
        let box_model = BoxModel {
            margin: s.margin,
            border_width: s.border_width,
            padding: s.padding,
        };
        let content_rect = box_model.content_rect(frame);
        // 允许 0 尺寸 content_rect：首帧 / 未设高的 Row·Column 需走 Flex
        // bootstrap（intrinsic_main）才能用子项撑开并写入 cached_content_size（#165）。
        // 若此处直接 return，子节点 frame 会停在 (0,0)，表现为文字重叠。

        let main_axis_indefinite = matches!(
            s.flex_direction,
            crate::ui::style::FlexDirection::Column
                | crate::ui::style::FlexDirection::ColumnReverse
        ) && s.height.is_none()
            || matches!(
                s.flex_direction,
                crate::ui::style::FlexDirection::Row | crate::ui::style::FlexDirection::RowReverse
            ) && s.width.is_none();

        // 委托给统一的 FlexLayout 布局引擎
        let engine = FlexLayout {
            direction: convert_flex_direction(s.flex_direction),
            gap: s.gap,
            justify: convert_justify(s.justify_content),
            align: convert_align(s.align_items),
            wrap: s.flex_wrap,
            overflow_content: s.overflow_content,
            intrinsic_main: main_axis_indefinite,
        };
        // 与 Space 对齐：禁止子项 flex-shrink。定高 Card 若压缩 Label/wrap，
        // Phase 1 写回矮 frame，与 Phase 2 扩展振荡（106↔121）。
        let mut children_no_shrink = children.to_vec();
        for child in &mut children_no_shrink {
            child.flex_shrink = 0.0;
        }
        let output = engine.layout(content_rect, &children_no_shrink);

        // 缓存子节点内容尺寸
        self.cached_content_size.set(Size::new(
            output.total_size.w.max(0.0),
            output.total_size.h.max(0.0),
        ));

        children
            .iter()
            .zip(output.positions)
            .map(|(child, rect)| (child.id, rect))
            .collect()
    }
}

/// 将 style::FlexDirection 转换为 layout::FlexDirection
fn convert_flex_direction(d: crate::ui::style::FlexDirection) -> FlexDirection {
    match d {
        crate::ui::style::FlexDirection::Row => FlexDirection::Row,
        crate::ui::style::FlexDirection::Column => FlexDirection::Column,
        crate::ui::style::FlexDirection::RowReverse => FlexDirection::RowReverse,
        crate::ui::style::FlexDirection::ColumnReverse => FlexDirection::ColumnReverse,
    }
}

/// 将 style::JustifyContent 转换为 layout::JustifyContent
fn convert_justify(j: crate::ui::style::JustifyContent) -> JustifyContent {
    match j {
        crate::ui::style::JustifyContent::Start => JustifyContent::Start,
        crate::ui::style::JustifyContent::Center => JustifyContent::Center,
        crate::ui::style::JustifyContent::End => JustifyContent::End,
        crate::ui::style::JustifyContent::SpaceBetween => JustifyContent::SpaceBetween,
        crate::ui::style::JustifyContent::SpaceAround => JustifyContent::SpaceAround,
        crate::ui::style::JustifyContent::SpaceEvenly => JustifyContent::SpaceEvenly,
        crate::ui::style::JustifyContent::Stretch => JustifyContent::Stretch,
    }
}

/// 将 style::AlignItems 转换为 layout::AlignItems
fn convert_align(a: crate::ui::style::AlignItems) -> AlignItems {
    match a {
        crate::ui::style::AlignItems::Start => AlignItems::Start,
        crate::ui::style::AlignItems::Center => AlignItems::Center,
        crate::ui::style::AlignItems::End => AlignItems::End,
        crate::ui::style::AlignItems::Stretch => AlignItems::Stretch,
    }
}

impl Default for Container {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotSource for Container {
    fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Container {
            style: self.style.clone(),
        }
    }
}

impl Container {
    pub fn new() -> Self {
        Self {
            style: Style::container(),
            cached_content_size: Cell::new(Size::zero()),
        }
    }

    // ═══════════════════════════════════════════════════
    // 统一样式设置
    // ═══════════════════════════════════════════════════

    /// 批量设置 Style（替换所有现有值）。
    pub(crate) fn sync_from(&mut self, next: Self) {
        self.style = next.style;
    }

    pub fn style(mut self, s: Style) -> Self {
        self.style = s;
        self
    }

    /// 应用另一个 Style（非零/非默认值覆盖当前值）。
    pub fn apply_style(mut self, s: Style) -> Self {
        self.style = self.style.apply(s);
        self
    }

    // ═══════════════════════════════════════════════════
    // CSS 风格链式方法
    // ═══════════════════════════════════════════════════

    /// 设置背景色（`bg` 别名）。
    pub fn bg(mut self, c: impl Into<ColorValue>) -> Self {
        self.style.background = Some(c.into());
        self
    }

    /// 设置外边距。
    pub fn margin(mut self, m: EdgeInsets) -> Self {
        self.style.margin = m;
        self
    }

    /// 设置内边距（`p` 别名）。
    pub fn padding(mut self, p: EdgeInsets) -> Self {
        self.style.padding = p;
        self
    }

    /// 设置内边距（简写）。
    pub fn p(mut self, p: EdgeInsets) -> Self {
        self.style.padding = p;
        self
    }

    /// 设置边框。
    pub fn border(mut self, color: impl Into<ColorValue>, width: f32) -> Self {
        self.style.border_color = Some(color.into());
        self.style.border_width = EdgeInsets::uniform(width);
        self
    }

    /// 设置圆角。
    pub fn rounded(mut self, r: f32) -> Self {
        self.style.border_radius = r;
        self
    }

    /// 设置固定宽度。
    pub fn w(mut self, v: f32) -> Self {
        self.style.width = Some(v);
        self
    }

    /// 设置固定高度。
    pub fn h(mut self, v: f32) -> Self {
        self.style.height = Some(v);
        self
    }

    /// 同时设置宽高。
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.style.width = Some(w);
        self.style.height = Some(h);
        self
    }

    /// 设置文字颜色。
    pub fn color(mut self, c: impl Into<ColorValue>) -> Self {
        self.style.color = c.into();
        self
    }

    /// 设置字号。
    pub fn fs(mut self, s: impl Into<TypographyToken>) -> Self {
        self.style.font_size = s.into();
        self
    }

    /// 设置显示模式（Flex / None）。
    pub fn display(mut self, d: DisplayMode) -> Self {
        self.style.display = d;
        self
    }

    /// 设置 flex 方向。
    pub fn direction(mut self, d: crate::ui::style::FlexDirection) -> Self {
        self.style.flex_direction = d;
        self
    }

    /// 设置子项间距。
    pub fn gap(mut self, g: f32) -> Self {
        self.style.gap = g;
        self
    }

    /// 设置主轴对齐。
    pub fn justify(mut self, j: crate::ui::style::JustifyContent) -> Self {
        self.style.justify_content = j;
        self
    }

    /// 设置交叉轴对齐。
    pub fn align(mut self, a: crate::ui::style::AlignItems) -> Self {
        self.style.align_items = a;
        self
    }

    /// 设置 flex-grow。
    pub fn flex_grow(mut self, v: f32) -> Self {
        self.style.flex_grow = v;
        self
    }

    /// 设置 flex-shrink。
    pub fn flex_shrink(mut self, v: f32) -> Self {
        self.style.flex_shrink = v;
        self
    }

    /// 设置透明度。
    pub fn opacity(mut self, o: f32) -> Self {
        self.style.opacity = o;
        self
    }

    /// 设置盒阴影/辉光。
    pub fn shadow(mut self, s: BoxShadowDef) -> Self {
        self.style.box_shadow = Some(s);
        self
    }

    /// 设置可见性。
    pub fn visible(mut self, v: bool) -> Self {
        self.style.visible = v;
        self
    }

    /// 便捷方法：单独设置内边距。
    pub fn pad(mut self, p: EdgeInsets) -> Self {
        self.style.padding = p;
        self
    }

    /// 便捷方法：设置 flex 方向。
    pub fn dir(mut self, d: FlexDirection) -> Self {
        self.style.flex_direction = match d {
            FlexDirection::Row => crate::ui::style::FlexDirection::Row,
            FlexDirection::Column => crate::ui::style::FlexDirection::Column,
            FlexDirection::RowReverse => crate::ui::style::FlexDirection::RowReverse,
            FlexDirection::ColumnReverse => crate::ui::style::FlexDirection::ColumnReverse,
        };
        self
    }

    /// 允许内容溢出（跳过 flex-shrink，用于可滚动容器）。
    pub fn overflow_content(mut self) -> Self {
        self.style.overflow_content = true;
        self
    }

    fn child_measure_constraints(&self, content_rect: Rect) -> Constraints {
        let is_row = matches!(
            self.style.flex_direction,
            crate::ui::style::FlexDirection::Row | crate::ui::style::FlexDirection::RowReverse
        );
        // 与 Space 对齐：主轴始终 MAX。定高 Column 若用 content_rect.h 钳子项，
        // measure 会把 wrap 内容从 121 压回 106，Phase 1 写回后与 Phase 2 振荡；
        // ScrollView 内又无 parent_cap，会打满 converge。交叉轴在有明确尺寸时
        // 仍约束，供 wrap 计算行宽。
        let max_w = if is_row || self.style.width.is_none() {
            f32::MAX
        } else {
            content_rect.w
        };
        let max_h = if !is_row || self.style.height.is_none() {
            f32::MAX
        } else {
            content_rect.h
        };
        Constraints::loose(Size::new(max_w, max_h))
    }

    fn intrinsic_size(&self) -> Size {
        let bh = self.style.border_width.horizontal();
        let bv = self.style.border_width.vertical();
        let cached = self.cached_content_size.get();
        // flex_grow 子项必须以 0 为 basis，让父级分配确定空间；
        // 否则窗口缩小后仍用上一轮 cached 内容高/宽，ScrollView 视口被撑满 → 无滚动条。
        let grow = self.style.flex_grow > 0.0;
        let content_w = if cached.w > 0.0 {
            cached.w + self.style.padding.horizontal()
        } else {
            0.0
        };
        let content_h = if cached.h > 0.0 {
            cached.h + self.style.padding.vertical()
        } else {
            0.0
        };
        // 显式宽高是下限：内容 / Phase 2 撑开后不得再向父级低报，否则与扩展振荡。
        let effective_w = match self.style.width {
            Some(w) => {
                if content_w > 0.0 {
                    w.max(content_w)
                } else {
                    w
                }
            }
            None => {
                if grow {
                    0.0
                } else {
                    content_w
                }
            }
        };
        let effective_h = match self.style.height {
            Some(h) => {
                if content_h > 0.0 {
                    h.max(content_h)
                } else {
                    h
                }
            }
            None => {
                if grow {
                    0.0
                } else {
                    content_h
                }
            }
        };
        Size::new(effective_w + bh, effective_h + bv)
    }
}

