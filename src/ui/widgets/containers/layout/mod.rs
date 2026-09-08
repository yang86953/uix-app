//! Layout 页面布局组件 — Header / Sider / Content / Footer 骨架。
//!
//! 组合使用构建标准页面布局。

// 每个公开布局区域拥有独立 Rust/UIX 同目录组件。
mod content;
mod footer;
mod header;
mod sider;

pub use content::Content;
pub use footer::Footer;
pub use header::Header;
pub use sider::Sider;

use crate::core::{Constraints, Rect, Size};
use crate::draw::Color;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::widget;
// 引入共享子节点测量入口。
use crate::ui::widget_runtime::tree_measure::child_from_tree_with_flex_constraints;
use crate::ui::widget_runtime::widget::WidgetTree;
// 引入布局尺寸归一化入口。
use crate::ui::layout::engine::normalize_layout_size;
// 引入唯一 Flex 算法与布局方向契约。
use crate::ui::layout::{FlexDirection, FlexLayout, LayoutChild};
// 引入组件标识与快照字段。
use crate::ui::{SnapshotFields, View, ViewNode, WidgetId};

// 保存 Layout 的默认方向、固有尺寸与弹性增长策略。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LayoutVisual {
    default_direction: FlexDirection,
    intrinsic_width: f32,
    intrinsic_height: f32,
    flex_grow: f32,
}

crate::uix_items!("src/ui/widgets/containers/layout/layout.uix");

pub(crate) const fn layout_default_direction() -> FlexDirection {
    FlexDirection::Column
}

widget! {
    /// Layout — 页面布局容器（flex 列）。
    pub struct Layout {
        bg_color: Option<Color>,
        direction: FlexDirection,
        /// 同目录 UIX 生成的唯一静态视觉表。
        pub(crate) visual: &'static LayoutVisual,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    flex_grow => (&self) -> f32 { self.visual.flex_grow }

    flex_layout_axes => (&self) -> Option<(FlexDirection, crate::ui::layout::AlignItems)> {
        Some((self.direction, crate::ui::layout::AlignItems::Stretch))
    }

    measure_children => (&self, frame: Rect, children: &[WidgetId], tree: &WidgetTree)
        -> Vec<LayoutChild>
    {
        // 复用布局壳统一的有限子节点测量入口。
        measure_shell_children(self.direction, frame, children, tree)
    }

    measure_children_into => (
        &self,
        frame: Rect,
        children: &[WidgetId],
        tree: &WidgetTree,
        output: &mut Vec<LayoutChild>
    ) {
        // 真实布局帧把测量快照直接写入树级工作区。
        measure_shell_children_into(self.direction, frame, children, tree, output);
    }

    layout_children => (&self, frame: Rect, children: &[LayoutChild], _tree: &WidgetTree)
        -> Vec<(WidgetId, Rect)>
    {
        // 把方向与子项事实交给共享 FlexLayout。
        layout_shell_children(self.direction, frame, children)
    }

    layout_children_into => (
        &self,
        frame: Rect,
        children: &[LayoutChild],
        _tree: &WidgetTree,
        scratch: &mut crate::ui::LayoutEngineScratch,
        output: &mut Vec<(WidgetId, Rect)>
    ) {
        // 真实布局帧复用树级 Flex 求解与位置映射缓冲。
        layout_shell_children_into(self.direction, frame, children, scratch, output);
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        if let Some(bg) = self.bg_color {
            ctx.fill_rect(frame, bg, None);
        }
    }
}

// 以当前区域尺寸收窄布局壳子节点测量约束。
fn measure_shell_children(
    direction: FlexDirection,
    // 接收父区域边框盒。
    frame: Rect,
    // 接收来源顺序组件标识。
    children: &[WidgetId],
    // 借用当前组件树完成受约束测量。
    tree: &WidgetTree,
) -> Vec<LayoutChild> {
    let mut output = Vec::with_capacity(children.len());
    measure_shell_children_into(direction, frame, children, tree, &mut output);
    output
}

// 把布局壳子节点测量快照写入调用方拥有的工作区。
fn measure_shell_children_into(
    direction: FlexDirection,
    frame: Rect,
    children: &[WidgetId],
    tree: &WidgetTree,
    output: &mut Vec<LayoutChild>,
) {
    // 只允许有限非负父尺寸进入子测量。
    let maximum = normalize_layout_size(Size::new(frame.w, frame.h));
    // 使用松约束保留子组件固有尺寸与 flex 事实。
    let constraints = Constraints::loose(maximum);
    // 按声明顺序生成共享布局描述符。
    output.clear();
    output.extend(
        children
            // 遍历轻量组件标识。
            .iter()
            // 复制标识供测量入口使用。
            .copied()
            // 从组件树读取尺寸、弹性与边距事实。
            .map(|id| child_from_tree_with_flex_constraints(id, tree, constraints, direction)),
    );
}

// 把布局壳区域排列委托给唯一共享 Flex 算法。
fn layout_shell_children(
    // 接收文档化主轴方向。
    direction: FlexDirection,
    // 接收当前父区域。
    frame: Rect,
    // 接收已经测量的有序子项。
    children: &[LayoutChild],
) -> Vec<(WidgetId, Rect)> {
    let mut scratch = crate::ui::LayoutEngineScratch::default();
    let mut output = Vec::with_capacity(children.len());
    layout_shell_children_into(direction, frame, children, &mut scratch, &mut output);
    output
}

// 在布局树拥有的 Flex 工作区中求解并写回有序子节点位置。
fn layout_shell_children_into(
    direction: FlexDirection,
    frame: Rect,
    children: &[LayoutChild],
    scratch: &mut crate::ui::LayoutEngineScratch,
    output: &mut Vec<(WidgetId, Rect)>,
) {
    // 只覆盖方向，其余对齐和伸缩规则沿用共享默认值。
    let engine = FlexLayout {
        // 应用 Layout 或固定区域方向。
        direction,
        // 保持 FlexLayout 的统一默认策略。
        ..FlexLayout::new()
    };
    // 执行共享 Flex 求解并复用树级位置数组。
    let _ = engine.layout_into(frame, children, scratch);
    let positions = &scratch.flex.child_rects;
    // 把来源组件标识与求解位置重新配对。
    output.clear();
    output.reserve(children.len());
    output.extend(
        children
            // 遍历来源顺序描述符。
            .iter()
            // 与相同顺序的 Flex 输出配对。
            .zip(positions)
            // 返回组件树布局入口要求的映射。
            .map(|(child, rect)| (child.id, *rect)),
    );
}

// 构造方法
impl Default for Layout {
    fn default() -> Self {
        Self::new()
    }
}

impl Layout {
    /// 创建使用纵向主轴且不覆盖主题背景的页面布局容器。
    pub fn new() -> Self {
        Self {
            // 默认不覆盖主题背景。
            bg_color: None,
            // 文档规定 Layout 默认纵向排列。
            direction: LAYOUT_VISUAL_REF.default_direction,
            visual: LAYOUT_VISUAL_REF,
        }
    }
    /// 设置布局容器背景颜色。
    pub fn bg(mut self, c: Color) -> Self {
        self.bg_color = Some(c);
        self
    }

    /// 设置布局壳直接区域的 Flex 主轴方向。
    pub fn direction(mut self, direction: FlexDirection) -> Self {
        // 保存声明方向供测量后的排列阶段使用。
        self.direction = direction;
        // 返回更新后的构建器。
        self
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(self.visual.intrinsic_width, self.visual.intrinsic_height)
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.bg_color = next.bg_color;
        // 声明更新必须同步会改变子树几何的主轴方向。
        self.direction = next.direction;
        self.visual = next.visual;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Layout {
            bg_color: self.bg_color,
            // 快照保留方向以触发布局失效。
            direction: self.direction,
        }
    }
}

// 把 Layout Rust 内核与 UIX 静态视觉组合为单一叶节点。
fn build_layout_view(mut kernel: Layout, visual: &'static LayoutVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Layout {
    fn build(self) -> ViewNode {
        build_layout_uix_root(self)
    }
}

// 为 UIX 根提供稳定的 Rust 内核绑定名称。
fn build_layout_uix_root(kernel: Layout) -> ViewNode {
    crate::uix!("src/ui/widgets/containers/layout/layout.uix")
}
