//! Layout 页面布局组件 — Header / Sider / Content / Footer 骨架。
//!
//! 组合使用构建标准页面布局。

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Color;
use crate::ui::component::paint_context::PaintContext;
// 引入共享子节点测量入口。
use crate::ui::component::tree_measure::child_from_tree_with_constraints;
use crate::ui::component::widget::WidgetTree;
// 引入布局尺寸归一化入口。
use crate::ui::layout::engine::normalize_layout_size;
// 引入唯一 Flex 算法与布局方向契约。
use crate::ui::layout::{FlexDirection, FlexLayout, LayoutChild, LayoutEngine};
// 引入组件标识与快照字段。
use crate::ui::{ComponentId, SnapshotFields};

component! {
    /// Layout — 页面布局容器（flex 列）。
    pub struct Layout {
        bg_color: Option<Color>,
        direction: FlexDirection,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    flex_grow => (&self) -> f32 { 1.0 }

    measure_children => (&self, frame: Rect, children: &[ComponentId], tree: &WidgetTree)
        -> Vec<LayoutChild>
    {
        // 复用布局壳统一的有限子节点测量入口。
        measure_shell_children(frame, children, tree)
    }

    layout_children => (&self, frame: Rect, children: &[LayoutChild], _tree: &WidgetTree)
        -> Vec<(ComponentId, Rect)>
    {
        // 把方向与子项事实交给共享 FlexLayout。
        layout_shell_children(self.direction, frame, children)
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        if let Some(bg) = self.bg_color {
            ctx.fill_rect(frame, bg, None);
        }
    }
}

component! {
    /// Header — 页面顶部栏。
    pub struct Header {
        height: f32,
        bg_color: Option<Color>,
        title: Option<String>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        if let Some(bg) = self.bg_color {
            ctx.fill_rect(frame, bg, None);
        }
        if let Some(title) = &self.title {
            let color = ctx.tokens().color_text();
            let font_size = 16.0;
            let y = frame.y + (frame.h - font_size) * 0.5;
            ctx.draw_text(title, Point::new(frame.x + 12.0, y), color, font_size);
        }
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(crate::ui::ComponentId, Rect)>
    {
        // Header 内容按纵向来源顺序排列。
        layout_shell_children(FlexDirection::Column, frame, children)
    }

    measure_children => (&self, frame: Rect, children: &[ComponentId], tree: &WidgetTree)
        -> Vec<LayoutChild>
    {
        // Header 与其他布局壳区域共享测量契约。
        measure_shell_children(frame, children, tree)
    }
}

component! {
    /// Sider — 侧边栏。
    pub struct Sider {
        width: f32,
        bg_color: Option<Color>,
        collapsible: bool,
        collapsed: bool,
        collapsed_width: f32,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        if let Some(bg) = self.bg_color {
            ctx.fill_rect(frame, bg, None);
        }
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(crate::ui::ComponentId, Rect)>
    {
        // Sider 内容按纵向来源顺序排列。
        layout_shell_children(FlexDirection::Column, frame, children)
    }

    measure_children => (&self, frame: Rect, children: &[ComponentId], tree: &WidgetTree)
        -> Vec<LayoutChild>
    {
        // Sider 与其他布局壳区域共享测量契约。
        measure_shell_children(frame, children, tree)
    }
}

component! {
    /// Content — 内容区。
    pub struct Content {
        bg_color: Option<Color>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    flex_grow => (&self) -> f32 { 1.0 }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        if let Some(bg) = self.bg_color {
            ctx.fill_rect(frame, bg, None);
        }
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(crate::ui::ComponentId, Rect)>
    {
        // Content 内容按纵向来源顺序排列。
        layout_shell_children(FlexDirection::Column, frame, children)
    }

    measure_children => (&self, frame: Rect, children: &[ComponentId], tree: &WidgetTree)
        -> Vec<LayoutChild>
    {
        // Content 与其他布局壳区域共享测量契约。
        measure_shell_children(frame, children, tree)
    }
}

component! {
    /// Footer — 页面底部栏。
    pub struct Footer {
        height: f32,
        bg_color: Option<Color>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        if let Some(bg) = self.bg_color {
            ctx.fill_rect(frame, bg, None);
        }
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(crate::ui::ComponentId, Rect)>
    {
        // Footer 内容按纵向来源顺序排列。
        layout_shell_children(FlexDirection::Column, frame, children)
    }

    measure_children => (&self, frame: Rect, children: &[ComponentId], tree: &WidgetTree)
        -> Vec<LayoutChild>
    {
        // Footer 与其他布局壳区域共享测量契约。
        measure_shell_children(frame, children, tree)
    }
}

// 以当前区域尺寸收窄布局壳子节点测量约束。
fn measure_shell_children(
    // 接收父区域边框盒。
    frame: Rect,
    // 接收来源顺序组件标识。
    children: &[ComponentId],
    // 借用当前组件树完成受约束测量。
    tree: &WidgetTree,
) -> Vec<LayoutChild> {
    // 只允许有限非负父尺寸进入子测量。
    let maximum = normalize_layout_size(Size::new(frame.w, frame.h));
    // 使用松约束保留子组件固有尺寸与 flex 事实。
    let constraints = Constraints::loose(maximum);
    // 按声明顺序生成共享布局描述符。
    children
        // 遍历轻量组件标识。
        .iter()
        // 复制标识供测量入口使用。
        .copied()
        // 从组件树读取尺寸、弹性与边距事实。
        .map(|id| child_from_tree_with_constraints(id, tree, constraints))
        // 物化当前布局轮次快照。
        .collect()
}

// 把布局壳区域排列委托给唯一共享 Flex 算法。
fn layout_shell_children(
    // 接收文档化主轴方向。
    direction: FlexDirection,
    // 接收当前父区域。
    frame: Rect,
    // 接收已经测量的有序子项。
    children: &[LayoutChild],
) -> Vec<(ComponentId, Rect)> {
    // 只覆盖方向，其余对齐和伸缩规则沿用共享默认值。
    let engine = FlexLayout {
        // 应用 Layout 或固定区域方向。
        direction,
        // 保持 FlexLayout 的统一默认策略。
        ..FlexLayout::new()
    };
    // 执行共享 Flex 求解。
    let output = engine.layout(frame, children);
    // 把来源组件标识与求解位置重新配对。
    children
        // 遍历来源顺序描述符。
        .iter()
        // 与相同顺序的 Flex 输出配对。
        .zip(output.positions)
        // 返回组件树布局入口要求的映射。
        .map(|(child, rect)| (child.id, rect))
        // 物化全部子节点位置。
        .collect()
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
            direction: FlexDirection::Column,
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
        Size::new(300.0, 200.0)
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.bg_color = next.bg_color;
        // 声明更新必须同步会改变子树几何的主轴方向。
        self.direction = next.direction;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Layout {
            bg_color: self.bg_color,
            // 快照保留方向以触发布局失效。
            direction: self.direction,
        }
    }
}

// 为 UIX Lang Header 提供文档锚点默认高度。
impl Default for Header {
    // 构造四十八逻辑像素高的顶部区域。
    fn default() -> Self {
        // 与既有公开使用文档保持一致。
        Self::new(48.0)
    }
}

impl Header {
    /// 创建指定逻辑高度且不覆盖主题背景的顶部栏。
    pub fn new(height: f32) -> Self {
        Self {
            height,
            bg_color: None,
            title: None,
        }
    }
    /// 设置顶部栏背景颜色。
    pub fn bg(mut self, c: Color) -> Self {
        self.bg_color = Some(c);
        self
    }

    /// 声明式标题文字（左侧渲染）。
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(0.0, self.height)
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.height = next.height;
        self.bg_color = next.bg_color;
        self.title = next.title;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Header {
            height: self.height,
            bg_color: self.bg_color,
        }
    }
}

impl Sider {
    /// 创建指定展开宽度且默认不可折叠的侧边栏。
    pub fn new(width: f32) -> Self {
        Self {
            width,
            bg_color: None,
            collapsible: false,
            collapsed: false,
            collapsed_width: 80.0,
        }
    }
    /// 设置侧边栏背景颜色。
    pub fn bg(mut self, c: Color) -> Self {
        self.bg_color = Some(c);
        self
    }
    /// 设置侧边栏是否声明为可折叠。
    pub fn collapsible(mut self, v: bool) -> Self {
        self.collapsible = v;
        self
    }
    /// 设置侧边栏是否使用折叠宽度。
    pub fn collapsed(mut self, v: bool) -> Self {
        self.collapsed = v;
        self
    }
    /// 设置侧边栏折叠状态下的逻辑宽度。
    pub fn collapsed_width(mut self, w: f32) -> Self {
        self.collapsed_width = w;
        self
    }

    fn intrinsic_size(&self) -> Size {
        if self.collapsed {
            Size::new(self.collapsed_width, 0.0)
        } else {
            Size::new(self.width, 0.0)
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.width = next.width;
        self.bg_color = next.bg_color;
        self.collapsible = next.collapsible;
        self.collapsed = next.collapsed;
        self.collapsed_width = next.collapsed_width;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Sider {
            width: self.width,
            bg_color: self.bg_color,
            collapsible: self.collapsible,
            collapsed: self.collapsed,
            collapsed_width: self.collapsed_width,
        }
    }
}

// 为 UIX Lang Sider 提供文档锚点默认宽度。
impl Default for Sider {
    // 构造二百逻辑像素宽的侧栏区域。
    fn default() -> Self {
        // 与既有公开使用文档保持一致。
        Self::new(200.0)
    }
}

impl Default for Content {
    fn default() -> Self {
        Self::new()
    }
}

impl Content {
    /// 创建不覆盖主题背景的内容区。
    pub fn new() -> Self {
        Self { bg_color: None }
    }
    /// 设置内容区背景颜色。
    pub fn bg(mut self, c: Color) -> Self {
        self.bg_color = Some(c);
        self
    }

    /// 便捷：包裹单个子节点（等价 `tree! { Content::new() => [node] }`）。
    pub fn child(
        self,
        node: impl crate::ui::IntoWidgetNode,
    ) -> crate::ui::component::widget::WidgetNode {
        crate::ui::component::widget::WidgetNode::new(Box::new(self), vec![node.into_node()])
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(0.0, 0.0)
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.bg_color = next.bg_color;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Content {
            bg_color: self.bg_color,
        }
    }
}

impl Footer {
    /// 创建指定逻辑高度且不覆盖主题背景的底部栏。
    pub fn new(height: f32) -> Self {
        Self {
            height,
            bg_color: None,
        }
    }
    /// 设置底部栏背景颜色。
    pub fn bg(mut self, c: Color) -> Self {
        self.bg_color = Some(c);
        self
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(0.0, self.height)
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.height = next.height;
        self.bg_color = next.bg_color;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Footer {
            height: self.height,
            bg_color: self.bg_color,
        }
    }
}

// 为 UIX Lang Footer 提供与 Header 对称的默认高度。
impl Default for Footer {
    // 构造四十八逻辑像素高的底部区域。
    fn default() -> Self {
        // 保持应用布局壳默认尺寸一致。
        Self::new(48.0)
    }
}

// 验证布局壳方向与共享 Flex 分配契约。
#[cfg(test)]
mod tests {
    // 引入当前模块组件与共享布局类型。
    use super::*;
    // 引入运行时布局 trait 以调用组件入口。
    use crate::ui::WidgetLayout;

    // 验证默认纵向布局分配固定区域与可增长内容。
    #[test]
    fn layout_default_column_uses_shared_flex_distribution() {
        // 构造默认纵向布局壳。
        let layout = Layout::new();
        // 构造固定四十八像素 Header。
        let header = LayoutChild::new(ComponentId::new(1), Size::new(0.0, 48.0));
        // 构造可增长 Content。
        let mut content = LayoutChild::new(ComponentId::new(2), Size::zero());
        // 声明内容占用剩余空间。
        content.flex_grow = 1.0;
        // 构造固定四十八像素 Footer。
        let footer = LayoutChild::new(ComponentId::new(3), Size::new(0.0, 48.0));
        // 空树满足不读取树的排列签名。
        let tree = WidgetTree::new();
        // 在三百像素高区域中执行共享 Flex 排列。
        let positions = layout.layout_children(
            // 提供确定父区域。
            Rect::new(0.0, 0.0, 400.0, 300.0),
            // 保留 Header、Content、Footer 来源顺序。
            &[header, content, footer],
            // 传入空树。
            &tree,
        );
        // Header 保持固定高度并横向拉伸。
        assert_eq!(positions[0].1, Rect::new(0.0, 0.0, 400.0, 48.0));
        // Content 获得扣除上下固定区域后的剩余高度。
        assert_eq!(positions[1].1, Rect::new(0.0, 48.0, 400.0, 204.0));
        // Footer 位于底部并保持固定高度。
        assert_eq!(positions[2].1, Rect::new(0.0, 252.0, 400.0, 48.0));
    }

    // 验证横向 Layout 为 Sider 与嵌套 Layout 分配剩余宽度。
    #[test]
    fn layout_row_distributes_sider_and_nested_shell() {
        // 显式选择横向应用壳。
        let layout = Layout::new().direction(FlexDirection::Row);
        // 构造固定二百像素 Sider。
        let sider = LayoutChild::new(ComponentId::new(1), Size::new(200.0, 0.0));
        // 构造可增长的嵌套 Layout。
        let mut nested = LayoutChild::new(ComponentId::new(2), Size::zero());
        // 声明嵌套壳占用剩余空间。
        nested.flex_grow = 1.0;
        // 空树满足不读取树的排列签名。
        let tree = WidgetTree::new();
        // 在六百像素宽区域中执行共享 Flex 排列。
        let positions = layout.layout_children(
            // 提供确定父区域。
            Rect::new(0.0, 0.0, 600.0, 400.0),
            // 保持 Sider 在嵌套壳之前。
            &[sider, nested],
            // 传入空树。
            &tree,
        );
        // Sider 保持固定宽度并纵向拉伸。
        assert_eq!(positions[0].1, Rect::new(0.0, 0.0, 200.0, 400.0));
        // 嵌套壳获得其余四百像素宽度。
        assert_eq!(positions[1].1, Rect::new(200.0, 0.0, 400.0, 400.0));
    }
}
