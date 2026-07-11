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
        cached_content_size: Cell<Size>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
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
        let output = engine.layout(content_rect, children);

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
        let main_indefinite = if is_row {
            self.style.width.is_none()
        } else {
            self.style.height.is_none()
        };
        let cross_indefinite = if is_row {
            self.style.height.is_none()
        } else {
            self.style.width.is_none()
        };

        let max_w = if (is_row && main_indefinite) || (!is_row && cross_indefinite) {
            f32::MAX
        } else {
            content_rect.w
        };
        let max_h = if (!is_row && main_indefinite) || (is_row && cross_indefinite) {
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
        let effective_w = self.style.width.unwrap_or_else(|| {
            if grow {
                0.0
            } else if cached.w > 0.0 {
                cached.w + self.style.padding.horizontal()
            } else {
                0.0
            }
        });
        let effective_h = self.style.height.unwrap_or_else(|| {
            if grow {
                0.0
            } else if cached.h > 0.0 {
                cached.h + self.style.padding.vertical()
            } else {
                0.0
            }
        });
        Size::new(effective_w + bh, effective_h + bv)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::core::widget::WidgetCore;
    use crate::ui::traits::WidgetLayout;

    #[test]
    fn measure_clamps_container_intrinsic_size() {
        let measured = Container::new()
            .size(80.0, 30.0)
            .measure(Constraints::loose(Size::new(50.0, 40.0)));

        assert_eq!(measured, Size::new(50.0, 30.0));
    }

    #[test]
    fn measure_excludes_margin_from_intrinsic_size() {
        let measured = Container::new()
            .size(80.0, 24.0)
            .margin(EdgeInsets::new(1.0, 2.0, 3.0, 4.0))
            .measure(Constraints::unconstrained());

        assert_eq!(measured, Size::new(80.0, 24.0));
    }

    #[test]
    fn content_rect_does_not_subtract_margin_again() {
        // 父级已把 margin 算进子项 frame 起点；content_rect 只扣 border+padding。
        let box_model = BoxModel {
            margin: EdgeInsets::new(12.0, 0.0, 12.0, 0.0),
            border_width: EdgeInsets::zero(),
            padding: EdgeInsets::new(8.0, 4.0, 8.0, 4.0),
        };
        let frame = Rect::new(12.0, 10.0, 100.0, 32.0);
        assert_eq!(
            box_model.content_rect(frame),
            Rect::new(20.0, 14.0, 84.0, 24.0)
        );
        assert_eq!(box_model.visual_rect(frame), frame);
    }

    #[test]
    fn measure_uses_cached_content_height_without_explicit_height() {
        let container = Container::new().w(120.0);
        container.cached_content_size.set(Size::new(100.0, 48.0));

        let measured = container.measure(Constraints::unconstrained());

        assert_eq!(measured, Size::new(120.0, 48.0));
    }

    #[test]
    fn measure_children_filters_hidden_and_preserves_layout_metadata() {
        let mut tree = WidgetTree::new();
        let host = tree.set_root(Box::new(Container::new()));
        let visible = tree.add_child(
            host,
            Box::new(
                Container::new()
                    .style(
                        Style::container()
                            .with_grid_cell(2)
                            .with_grid_column_span(3)
                            .with_grid_row_span(4),
                    )
                    .size(20.0, 10.0)
                    .flex_grow(2.0)
                    .flex_shrink(0.25),
            ),
        );
        let hidden = tree.add_child(host, Box::new(Container::new().size(30.0, 12.0)));
        tree.get_mut(hidden).unwrap().set_visible(false);

        let measured = Container::new().size(100.0, 60.0).measure_children(
            Rect::new(0.0, 0.0, 100.0, 60.0),
            &[visible, hidden],
            &tree,
        );

        assert_eq!(measured.len(), 1);
        assert_eq!(measured[0].id, visible);
        assert_eq!(measured[0].measured_size, Size::new(20.0, 10.0));
        assert_eq!(measured[0].flex_grow, 2.0);
        assert_eq!(measured[0].flex_shrink, 0.25);
        assert_eq!(measured[0].grid_cell, Some(2));
        assert_eq!(measured[0].grid_column_span, 3);
        assert_eq!(measured[0].grid_row_span, 4);
    }

    #[test]
    fn zero_height_row_bootstraps_from_children() {
        use crate::ui::view::adapter::ViewAdapter;
        use crate::ui::view::{label, row};

        // 未设高的 Row 在 frame.h=0 时仍须布局子项并缓存 intrinsic 高度
        let mut tree = ViewAdapter::build(row([label("A"), label("B")]));
        let root_id = tree.root_id().expect("root");
        if let Some(root) = tree.get_mut(root_id) {
            root.set_frame(Rect::new(0.0, 0.0, 200.0, 0.0));
        }
        let children = tree.get(root_id).expect("root").children().to_vec();
        let positions = tree.get(root_id).expect("root").layout_children(
            Rect::new(0.0, 0.0, 200.0, 0.0),
            &children,
            &tree,
        );
        assert_eq!(positions.len(), 2, "zero-height row must place children");
        assert!(
            positions.iter().all(|(_, r)| r.h > 0.0),
            "children should get intrinsic height, got {positions:?}"
        );
        let container = tree
            .get(root_id)
            .expect("root")
            .component()
            .as_any()
            .downcast_ref::<Container>()
            .expect("Container");
        assert!(
            container.cached_content_size.get().h > 0.0,
            "cached_content_size must update after zero-height bootstrap"
        );
    }

    /// 复现首页快捷导航：不定高 column_fit 放在 row 里时，标题/描述不得重叠。
    #[test]
    fn column_fit_nav_tile_in_row_does_not_overlap_labels() {
        use crate::ui::layout::AlignItems;
        use crate::ui::view::adapter::ViewAdapter;
        use crate::ui::view::{column_fit, embed, label, row, space};
        use crate::ui::widgets::{Icon, Label};

        let tile = |title: &str, desc: &str| {
            column_fit([
                row([
                    embed(Icon::new("cpu").size(20.0)),
                    label(title).font_size(14.0),
                ])
                .align(AlignItems::Center)
                .gap(10.0),
                space(8.0),
                label(desc).font_size(12.0),
            ])
            .width(200.0)
            .padding(EdgeInsets::uniform(16.0))
        };

        let mut tree = ViewAdapter::build(row([
            tile("应用能力", "State · Timer · Theme · 多窗"),
            tile("通用组件", "Button · Tag · Icon"),
        ]));
        let root_id = tree.root_id().expect("root");
        // 模拟页面行：先给一个偏矮的 frame，依赖 expand 撑开
        tree.get_mut(root_id)
            .expect("root")
            .set_frame(Rect::new(0.0, 0.0, 800.0, 40.0));
        tree.push_layout_invalidation(root_id);
        tree.layout();

        let mut frames = Vec::new();
        for id in tree.traverse() {
            let node = tree.get(id).expect("node");
            if let Some(l) = node.component().as_any().downcast_ref::<Label>() {
                frames.push((l.text().to_string(), node.frame()));
            }
        }
        let title = frames
            .iter()
            .find(|(t, _)| t == "应用能力")
            .expect("title");
        let desc = frames
            .iter()
            .find(|(t, _)| t.contains("State"))
            .expect("desc");
        assert!(
            desc.1.y + 0.5 >= title.1.y + title.1.h,
            "desc must sit below title (no overlap): title={:?} desc={:?} all={frames:?}",
            title.1,
            desc.1
        );
        assert!(
            desc.1.y + 0.5 >= title.1.y + title.1.h + 6.0,
            "expected ~8px spacer between title and desc: title.bottom={} desc.y={} all={frames:?}",
            title.1.y + title.1.h,
            desc.1.y
        );
    }
}
