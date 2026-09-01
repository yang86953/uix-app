//! 组合子函数 — column, row, label, button, space, input 等。
//!
//! 提供函数式声明式 API。异质子节点用元组或 [`crate::views!`]：
//! `column((label("..."), button("...")))` / `column(views![...])`。
//!
//! # 使用示例
//!
//! ```ignore
//! use crate::ui::view::*;
//!
//! let ui = column((
//!     label("Hello").font_size(24.0).color(Color::blue()),
//!     button("Click").primary().on_click_fn(|| println!("clicked")),
//! )).padding(16.0);
//! ```

use crate::platform::windowing::ScrollDirection;
use crate::ui::layout::{FlexDirection, GridTrack};
use crate::ui::theme::style::{DisplayMode, Style};
use crate::ui::view::{View, ViewNode};

use crate::core::{Constraints, Rect, Size};
use crate::ui::WidgetTree;
use crate::ui::reactive::state::{Computed, State};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::traits::{Widget, WidgetCapabilities, WidgetLayout, WidgetRender};
use std::any::Any;
use std::sync::Arc;

// 拆分命令式节点嵌入转换，保持组合器主体在文件规模约束内。
#[path = "combinators/embed.rs"]
// 编译命令式节点嵌入转换的私有实现模块。
mod embed_node;
// 拆分标签内容转换，保持组合器主体在文件规模约束内。
#[path = "combinators/label.rs"]
// 编译标签内容转换的私有实现模块。
mod label_content;
// 保持标签构造器与内容转换契约的既有公开路径。
pub use label_content::{IntoLabelContent, dynamic_label, label};

/// 将异质 / 同质子节点收成 `Vec<ViewNode>`（公开用法见仓库 `docs/使用/布局.md`）。
///
/// - 同质：`column([label("a"), label("b")])`、`Vec<_>`
/// - 异质：`column((label("a"), button("b")))` 或 `column(views![...])`
pub trait IntoViewChildren {
    /// 消费输入并按声明顺序构建统一的视图节点列表。
    fn into_view_children(self) -> Vec<ViewNode>;
}

impl<T: View, const N: usize> IntoViewChildren for [T; N] {
    fn into_view_children(self) -> Vec<ViewNode> {
        self.into_iter().map(View::build).collect()
    }
}

impl<T: View> IntoViewChildren for Vec<T> {
    fn into_view_children(self) -> Vec<ViewNode> {
        self.into_iter().map(View::build).collect()
    }
}

/// 条件子节点：`true` 时包含 `child`，否则为空列表。
///
/// ```ignore
/// column(show(visible, label("详情")))
/// ```
impl View for () {
    fn build(self) -> ViewNode {
        ViewNode::leaf(crate::ui::widgets::Space::new())
    }
}

/// 在条件成立时构建单个子节点，否则返回空子节点列表。
pub fn show(when: bool, child: impl View) -> Vec<ViewNode> {
    if when {
        vec![child.build()]
    } else {
        Vec::new()
    }
}

/// 条件子节点（`Option`）：`Some` 构建为子项，`None` 跳过。
///
/// ```ignore
/// column((label("标题"), optional_banner))
/// ```
impl<T: View> View for Option<T> {
    fn build(self) -> ViewNode {
        match self {
            Some(child) => child.build(),
            None => ViewNode::leaf(crate::ui::widgets::Space::new()),
        }
    }
}

macro_rules! impl_into_view_children_tuple {
    () => {
        impl IntoViewChildren for () {
            fn into_view_children(self) -> Vec<ViewNode> {
                Vec::new()
            }
        }
    };
    ($($T:ident),+) => {
        impl<$($T: View),+> IntoViewChildren for ($($T,)+) {
            fn into_view_children(self) -> Vec<ViewNode> {
                #[allow(non_snake_case)]
                let ($($T,)+) = self;
                vec![$($T.build(),)+]
            }
        }
    };
}

impl_into_view_children_tuple!();
impl_into_view_children_tuple!(A);
impl_into_view_children_tuple!(A, B);
impl_into_view_children_tuple!(A, B, C);
impl_into_view_children_tuple!(A, B, C, D);
impl_into_view_children_tuple!(A, B, C, D, E);
impl_into_view_children_tuple!(A, B, C, D, E, F);
impl_into_view_children_tuple!(A, B, C, D, E, F, G);
impl_into_view_children_tuple!(A, B, C, D, E, F, G, H);
impl_into_view_children_tuple!(A, B, C, D, E, F, G, H, I);
impl_into_view_children_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_into_view_children_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_into_view_children_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);

/// 将 `tree!` / widget-tree 节点嵌入 View DSL（组件库演示等高级 interop）。
pub fn embed(node: impl crate::ui::IntoWidgetNode) -> ViewNode {
    // 将命令式节点转换为可由声明适配器继续处理的节点。
    embed_node::adopt_widget_node(node.into_node())
}

// ── 基础组合子 ──────────────────────────────────────────────

/// 列容器（Flex 方向为 Column），默认 flex_grow(1.0) 填满父容器高度。
///
/// 嵌套内容组若只需 intrinsic 高度，用 [`column_fit`]（公开用法见仓库 `docs/使用/布局.md`）。
///
/// 同质数组 / `Vec`，或异质元组 / [`crate::views!`]（公开用法见仓库 `docs/使用/布局.md`）。
pub fn column(children: impl IntoViewChildren) -> ViewNode {
    ViewNode::new(
        crate::ui::widgets::Container::new()
            .dir(FlexDirection::Column)
            .flex_grow(1.0),
        children.into_view_children(),
    )
}

/// 列容器，保持 intrinsic 高度（flex_grow = 0）。
///
/// 用于顶栏、侧栏品牌区、卡片内文案组等不应吞掉父列剩余空间的局部内容。
pub fn column_fit(children: impl IntoViewChildren) -> ViewNode {
    ViewNode::new(
        crate::ui::widgets::Container::new().dir(FlexDirection::Column),
        children.into_view_children(),
    )
}

/// 行容器（Flex 方向为 Row）。
///
/// 同质数组 / `Vec`，或异质元组 / [`crate::views!`]（公开用法见仓库 `docs/使用/布局.md`）。
pub fn row(children: impl IntoViewChildren) -> ViewNode {
    ViewNode::new(
        crate::ui::widgets::Container::new().dir(FlexDirection::Row),
        children.into_view_children(),
    )
}

/// Grid 容器。
///
/// 默认不预设轨道；调用 `.columns(...)` / `.rows(...)` 明确声明轨道。
pub fn grid(children: impl IntoViewChildren) -> GridBuilder {
    GridBuilder {
        children: children.into_view_children(),
        widget: crate::ui::widgets::Grid::new(),
        style: Style::default().with_display(DisplayMode::Grid),
    }
}

/// 用于配置静态轨道或响应式列规则的 Grid 视图构建器。
pub struct GridBuilder {
    children: Vec<ViewNode>,
    widget: crate::ui::widgets::Grid,
    style: Style,
}

impl GridBuilder {
    /// 设置显式列轨定义并同步到 Grid 组件与节点样式。
    pub fn columns(mut self, columns: Vec<GridTrack>) -> Self {
        self.style = self.style.with_grid_columns(columns.clone());
        self.widget = self.widget.columns(columns);
        self
    }

    /// 设置显式行轨定义并同步到 Grid 组件与节点样式。
    pub fn rows(mut self, rows: Vec<GridTrack>) -> Self {
        self.style = self.style.with_grid_rows(rows.clone());
        self.widget = self.widget.rows(rows);
        self
    }

    /// 切换为 24 单元响应式 Grid。
    pub fn responsive(mut self) -> Self {
        self.style.grid_template_columns.clear();
        self.widget = crate::ui::widgets::Grid::responsive();
        self
    }

    /// 设置响应式 Grid 使用的断点集合。
    pub fn breakpoints(mut self, breakpoints: crate::ui::widgets::Breakpoints) -> Self {
        self.style.grid_template_columns.clear();
        self.widget = self.widget.breakpoints(breakpoints);
        self
    }

    /// 设置响应式 Grid 的子列占位配置。
    pub fn cols(mut self, cols: Vec<crate::ui::widgets::Col>) -> Self {
        self.style.grid_template_columns.clear();
        self.widget = self.widget.cols(cols);
        self
    }

    /// 同时设置 Grid 的列间距与行间距。
    pub fn gap(mut self, gap: f32) -> Self {
        self.style = self.style.with_gap(gap).with_grid_gap(gap, gap);
        self.widget = self.widget.gap(gap);
        self
    }

    /// 设置相邻列轨之间的间距。
    pub fn col_gap(mut self, gap: f32) -> Self {
        self.style.grid_column_gap = gap;
        self.widget = self.widget.col_gap(gap);
        self
    }

    /// 设置相邻行轨之间的间距。
    pub fn row_gap(mut self, gap: f32) -> Self {
        self.style.grid_row_gap = gap;
        self.widget = self.widget.row_gap(gap);
        self
    }

    /// 使用两个等宽弹性列配置 Grid。
    pub fn two_columns(mut self) -> Self {
        let columns = vec![GridTrack::Fr(1.0), GridTrack::Fr(1.0)];
        self.style = self.style.with_grid_columns(columns.clone());
        self.widget = self.widget.columns(columns);
        self
    }

    /// 使用三个等宽弹性列配置 Grid。
    pub fn three_columns(mut self) -> Self {
        let columns = vec![GridTrack::Fr(1.0), GridTrack::Fr(1.0), GridTrack::Fr(1.0)];
        self.style = self.style.with_grid_columns(columns.clone());
        self.widget = self.widget.columns(columns);
        self
    }
}

impl View for GridBuilder {
    fn build(self) -> ViewNode {
        let mut node = ViewNode::new(self.widget, self.children);
        node.style = self.style;
        node
    }
}

impl From<GridBuilder> for ViewNode {
    fn from(builder: GridBuilder) -> Self {
        builder.build()
    }
}

/// Scroll 容器。
///
/// 默认垂直滚动；可链式切换为 `.horizontal()` 或 `.both()`。
pub fn scroll(child: impl View) -> ScrollBuilder {
    ScrollBuilder {
        child: child.build(),
        direction: ScrollDirection::Vertical,
        fixed_size: None,
        flex_grow: 1.0,
        show_scrollbar: true,
        scroll_offset: None,
    }
}

/// 用于配置滚动方向、视口尺寸与滚动条的视图构建器。
pub struct ScrollBuilder {
    child: ViewNode,
    direction: ScrollDirection,
    fixed_size: Option<(f32, f32)>,
    flex_grow: f32,
    show_scrollbar: bool,
    scroll_offset: Option<State<crate::core::Point>>,
}

impl ScrollBuilder {
    /// 将视口限制为仅垂直滚动。
    pub fn vertical(mut self) -> Self {
        self.direction = ScrollDirection::Vertical;
        self
    }

    /// 将视口限制为仅水平滚动。
    pub fn horizontal(mut self) -> Self {
        self.direction = ScrollDirection::Horizontal;
        self
    }

    /// 允许视口在水平与垂直两个方向滚动。
    pub fn both(mut self) -> Self {
        self.direction = ScrollDirection::Both;
        self
    }

    /// 设置滚动视口的固定宽度与高度。
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.fixed_size = Some((w, h));
        self
    }

    /// 设置滚动容器在父级 Flex 布局中的伸展权重。
    pub fn flex_grow(mut self, value: f32) -> Self {
        self.flex_grow = value;
        self
    }

    /// 设置是否绘制滚动条。
    pub fn show_scrollbar(mut self, value: bool) -> Self {
        self.show_scrollbar = value;
        self
    }

    /// 将视口运行态偏移双向绑定到应用 State。
    pub fn scroll_offset(mut self, state: &State<crate::core::Point>) -> Self {
        self.scroll_offset = Some(state.clone());
        self
    }
}

impl View for ScrollBuilder {
    fn build(self) -> ViewNode {
        let mut widget = crate::ui::widgets::ScrollView::new(self.direction)
            .flex_grow(self.flex_grow)
            .show_scrollbar(self.show_scrollbar);
        if let Some((w, h)) = self.fixed_size {
            widget = widget.size(w, h);
        }
        if let Some(offset) = self.scroll_offset.as_ref() {
            widget = widget.scroll_offset(offset);
        }
        ViewNode::new(widget, vec![self.child])
    }
}

impl From<ScrollBuilder> for ViewNode {
    fn from(builder: ScrollBuilder) -> Self {
        builder.build()
    }
}

impl<T: Clone + Send + Sync + 'static> State<T> {
    /// 由当前 State 值构建子视图；结构更新由根 View 构建期捕获的依赖触发。
    pub fn map<F, V>(&self, f: F) -> ViewNode
    where
        F: FnOnce(&T) -> V,
        V: View,
    {
        f(&self.get()).build()
    }

    /// 由 State 生成响应式文本节点；内部 clone 句柄，调用方只保留一个名字。
    pub fn map_text<F>(&self, f: F) -> ViewNode
    where
        F: Fn(&T) -> String + 'static,
    {
        let state = self.clone();
        label(move || f(&state.get()))
    }

    /// 按当前 State 值构建可选子视图；`None` 使用空节点参与 reconcile。
    pub fn map_opt<F, V>(&self, f: F) -> Option<ViewNode>
    where
        F: FnOnce(&T) -> Option<V>,
        V: View,
    {
        f(&self.get()).map(View::build)
    }
}

impl<T: Clone + Send + Sync + 'static> Computed<T> {
    /// 由派生值构建当前子视图；结构更新由根 View 构建期捕获的依赖触发。
    pub fn map<F, V>(&self, f: F) -> ViewNode
    where
        F: FnOnce(&T) -> V,
        V: View,
    {
        f(&self.get()).build()
    }

    /// 由派生值生成响应式文本节点，依赖变化仅触发窄 Paint 失效。
    pub fn map_text<F>(&self, f: F) -> ViewNode
    where
        F: Fn(&T) -> String + 'static,
    {
        let computed = self.clone();
        label(move || f(&computed.get()))
    }

    /// 按派生值构建可选子视图；`None` 使用空节点参与 reconcile。
    pub fn map_opt<F, V>(&self, f: F) -> Option<ViewNode>
    where
        F: FnOnce(&T) -> Option<V>,
        V: View,
    {
        f(&self.get()).map(View::build)
    }
}

type CanvasPaint = dyn Fn(Rect, &mut PaintContext<'_, '_>);

struct Canvas {
    size: Size,
    paint: Box<CanvasPaint>,
}

impl Widget for Canvas {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::from_bits(WidgetCapabilities::LAYOUT | WidgetCapabilities::RENDER)
    }

    fn may_produce_overlay(&self) -> bool {
        false
    }

    fn as_layout(&self) -> Option<&dyn WidgetLayout> {
        Some(self)
    }

    fn as_render(&self) -> Option<&dyn WidgetRender> {
        Some(self)
    }

    fn as_render_mut(&mut self) -> Option<&mut dyn WidgetRender> {
        Some(self)
    }
}

impl WidgetLayout for Canvas {
    fn measure(&self, constraints: Constraints) -> Size {
        constraints.clamp(self.size)
    }
}

impl WidgetRender for Canvas {
    fn render(&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        (self.paint)(frame, ctx);
    }
}

// 显式共享 parity 通过真实 Canvas WidgetRender 入口提交一条 UI 绘制命令。
#[cfg(feature = "graphics-parity-test")]
pub(crate) fn render_shared_production_scene(
    draw_context: &mut crate::draw::painting::PaintContext<'_>,
    frame: Rect,
    rect: Rect,
    color: crate::draw::Color,
) {
    let tree = WidgetTree::new();
    let mut context = PaintContext::new(draw_context, tree.theme_tokens());
    let widget = Canvas {
        size: Size::new(frame.w, frame.h),
        paint: Box::new(move |_frame, context| context.fill_rect(rect, color, None)),
    };
    WidgetRender::render(&widget, frame, &mut context, &tree);
}

/// 创建固定 logical 尺寸的轻量绘制节点，无需声明完整组件。
///
/// 闭包只在节点需要绘制时执行；其中读取的 `State` / `Computed` 会自动绑定
/// 到该节点的 Paint 失效，不会形成每帧回调。
pub fn canvas<F>(width: f32, height: f32, paint: F) -> ViewNode
where
    F: Fn(Rect, &mut PaintContext) + 'static,
{
    let finite_extent = |value: f32| {
        if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        }
    };
    ViewNode::leaf(Canvas {
        size: Size::new(finite_extent(width), finite_extent(height)),
        paint: Box::new(paint),
    })
}

/// 空白占位，通过 `height` 控制垂直间距。
pub fn space(height: f32) -> ViewNode {
    ViewNode::leaf(
        crate::ui::widgets::Space::new()
            .size(crate::ui::widgets::SpaceSize::Small)
            .height(height),
    )
}

// 作用域捕获帧守卫：正常路径显式结束，panic 展开时恢复线程捕获栈深度。
struct ScopedCaptureFrame {
    finished: bool,
}

impl ScopedCaptureFrame {
    fn begin() -> Self {
        crate::ui::reactive::state::begin_state_capture();
        Self { finished: false }
    }
    fn finish(mut self) -> crate::ui::reactive::state::StateCaptureOutput {
        let output = crate::ui::reactive::state::end_state_capture();
        self.finished = true;
        output
    }
}

impl Drop for ScopedCaptureFrame {
    fn drop(&mut self) {
        if !self.finished {
            // 丢弃仅属于异常构建帧的捕获输出并恢复栈深度。
            let _ = crate::ui::reactive::state::end_state_capture();
        }
    }
}

/// 声明一个子树作用域（node-scoped reactive view）。
///
/// 闭包在挂载时执行一次；其中读取的 [`State`](crate::ui::reactive::state::State)
/// 登记为本子树的结构依赖——变化时框架只重跑本闭包并原位协调该子树，
/// 不再触发整树根重建。外层其他 State 变化引起的根重建会照常重跑本闭包。
///
/// 闭包契约与 [`State::map`](crate::ui::reactive::state::State::map) 一致：
/// 必须是可重复执行的纯构建（不修改外部可变状态、不依赖执行次数）。
/// 高频更新的值仍应优先使用窄失效通道（`canvas`、`map_text`、输入组件绑定）；
/// `scoped` 面向的是「结构随状态变化」的中低频子树。
pub fn scoped<F, V>(build: F) -> ViewNode
where
    F: Fn() -> V + 'static,
    V: View,
{
    // 重建工厂：节点失效时由所属树在完整捕获边界内重跑。
    let rebuild: std::sync::Arc<dyn Fn() -> ViewNode> =
        std::sync::Arc::new(move || build().build());
    // 独立捕获帧：作用域内读取的 State 登记到本子树，不进入外层根帧。
    let frame = ScopedCaptureFrame::begin();
    let mut node = rebuild();
    let output = frame.finish();
    // 直接返回另一个 scoped 节点会让外层工厂覆盖内层语义；需要嵌套时
    // 必须在两者之间包一层容器节点（如 column）形成真实父子结构。
    assert!(
        node.scoped_rebuild.is_none(),
        "scoped 闭包不得直接返回另一个 scoped 节点：请在两者之间包一层容器"
    );
    node.captured_state_binds.extend(output.state_binds);
    node.captured_effects.extend(output.effects);
    node.scoped_rebuild = Some(rebuild);
    node
}

// ── 按钮（View DSL 唯一入口）──────────────────────────────────

use crate::ui::event::{HandlerRegistration, SemanticEvent, SemanticKind};
use crate::ui::theme::style::StyleSet;
// 引入按钮组件与连体位置契约。
use crate::ui::widgets::{Button, ButtonGroupPosition};

/// 按钮构建器 — `button("text").primary().on_click(&state, |s| …)`。
///
/// 样式（背景、颜色、边距等）在 builder 之后链式调用 `StyleExt` 方法：
/// `button("保存").primary().bg(color).padding(8.0)`。
///
/// 嵌入 `tree!` / `Space::child` 等非 View 容器时，末尾调用 `.widget()`。
pub struct ButtonBuilder {
    text: String,
    // 共享 UIX 选择的主题预设，避免每个构建器重复分配相同 StyleSet。
    style_set: Arc<StyleSet>,
    disabled: bool,
    block: bool,
    size: crate::platform::windowing::ControlSize,
    // 保存待物化到底层 Button 的加载态。
    loading: bool,
    handlers: Vec<HandlerRegistration>,
}

impl ButtonBuilder {
    fn into_parts(self) -> (Button, Vec<HandlerRegistration>) {
        (
            Button::assemble(
                self.text,
                self.style_set,
                self.disabled,
                self.block,
                self.size,
            )
            // 复用 Button 既有旋转器、禁用点击与逐帧生命周期。
            .loading(self.loading),
            self.handlers,
        )
    }

    /// 构建底层 `Button`（用于 `tree!`、`Space::child` 等）。
    pub fn widget(self) -> Button {
        self.into_parts().0
    }
    /// 使用指定样式集合替换按钮当前的主题样式。
    pub fn style_set(mut self, style_set: StyleSet) -> Self {
        self.style_set = Arc::new(style_set);
        self
    }

    /// 应用主题定义的主要操作按钮样式。
    pub fn primary(mut self) -> Self {
        self.style_set = Button::primary_preset_style_set();
        self
    }
    /// 应用主题定义的幽灵按钮样式。
    pub fn ghost(mut self) -> Self {
        self.style_set = Button::ghost_preset_style_set();
        self
    }
    /// 应用主题定义的危险操作按钮样式。
    pub fn danger(mut self) -> Self {
        self.style_set = Button::danger_preset_style_set();
        self
    }
    /// 设置按钮是否拒绝交互并呈现禁用状态。
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    /// 设置按钮是否占满父级提供的水平空间。
    pub fn block(mut self, v: bool) -> Self {
        self.block = v;
        self
    }
    /// 设置按钮使用的标准控件尺寸档位。
    pub fn size(mut self, size: crate::platform::windowing::ControlSize) -> Self {
        self.size = size;
        self
    }

    /// 设置按钮是否显示加载旋转器并拒绝点击。
    pub fn loading(mut self, loading: bool) -> Self {
        // 保存配置直到构建器物化底层 Button。
        self.loading = loading;
        // 返回构建器以继续链式配置。
        self
    }

    /// 将底层按钮标记为 ButtonGroup 中的连体位置并物化为 ViewNode。
    pub fn group_position(self, position: ButtonGroupPosition) -> ViewNode {
        // 拆分底层按钮与已登记事件处理器。
        let (button, handlers) = self.into_parts();
        // 把连体位置交给既有 Button 视觉语义。
        let mut node = View::build(button.group_position(position));
        // 保留物化前已经登记的按钮事件。
        node.handlers = handlers;
        // 返回可继续应用公共样式与事件的节点。
        node
    }

    /// 默认点击路径：绑定 `State` 指纹，reconcile 可稳定复用（公开用法见仓库 `docs/使用/事件.md`）。
    ///
    /// 框架传入 `&State<T>`，调用方无需再 clone 句柄进闭包。
    pub fn on_click<T, F>(mut self, state: &State<T>, mut f: F) -> Self
    where
        T: Clone + Send + Sync + 'static,
        F: FnMut(&State<T>) + 'static,
    {
        let captured = state.clone();
        self.handlers.push(
            HandlerRegistration::new(
                SemanticKind::Click,
                Box::new(move |event| {
                    if event.is_primary_click() {
                        f(&captured);
                    }
                }),
            )
            .with_state_capture(state),
        );
        self
    }

    /// 无 State 的点击闭包；每次 reconcile **保守重绑**（公开用法见仓库 `docs/使用/事件.md`）。
    ///
    /// 命名保留 `_fn`：Rust 无法与 [`Self::on_click`] 重载；有 State 时优先 `on_click(&state, …)`（公开用法见仓库 `docs/使用/事件.md`）。
    pub fn on_click_fn<F: FnMut() + 'static>(mut self, mut f: F) -> Self {
        self.handlers.push(HandlerRegistration::new(
            SemanticKind::Click,
            Box::new(move |event| {
                if event.is_primary_click() {
                    f();
                }
            }),
        ));
        self
    }

    /// 兼容别名：指纹同 [`Self::on_click`]，闭包不接收 `&State`。
    ///
    /// 新代码优先 `on_click(&state, |s| …)`；无 State 用 [`Self::on_click_fn`]。
    #[doc(alias = "on_click")]
    pub fn on_click_capture<T, F>(self, state: &State<T>, mut f: F) -> Self
    where
        T: Clone + Send + Sync + 'static,
        F: FnMut() + 'static,
    {
        self.on_click(state, move |_| f())
    }

    /// 绑定携带窗口捕获指纹的无状态主键点击回调。
    pub fn on_click_window_capture<F>(mut self, window_id: crate::core::WindowId, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.handlers.push(
            HandlerRegistration::new(
                SemanticKind::Click,
                Box::new(move |event| {
                    if event.is_primary_click() {
                        f();
                    }
                }),
            )
            .with_window_capture(window_id),
        );
        self
    }

    /// 绑定可读写完整语义点击事件的回调。
    pub fn on_click_event<F: FnMut(&mut SemanticEvent) + 'static>(mut self, f: F) -> Self {
        self.handlers
            .push(HandlerRegistration::new(SemanticKind::Click, Box::new(f)));
        self
    }

    /// 绑定完整语义点击事件回调，并用指定状态建立稳定捕获指纹。
    pub fn on_click_event_capture<T, F>(mut self, state: &State<T>, f: F) -> Self
    where
        T: Clone + Send + Sync + 'static,
        F: FnMut(&mut SemanticEvent) + 'static,
    {
        self.handlers.push(
            HandlerRegistration::new(SemanticKind::Click, Box::new(f)).with_state_capture(state),
        );
        self
    }

    /// 绑定完整语义点击事件回调，并用窗口标识建立捕获指纹。
    pub fn on_click_event_window_capture<F>(
        mut self,
        window_id: crate::core::WindowId,
        f: F,
    ) -> Self
    where
        F: FnMut(&mut SemanticEvent) + 'static,
    {
        self.handlers.push(
            HandlerRegistration::new(SemanticKind::Click, Box::new(f))
                .with_window_capture(window_id),
        );
        self
    }
}

impl View for ButtonBuilder {
    fn build(self) -> ViewNode {
        let (button, handlers) = self.into_parts();
        let mut node = View::build(button);
        node.handlers = handlers;
        node
    }
}

impl From<ButtonBuilder> for ViewNode {
    fn from(b: ButtonBuilder) -> Self {
        b.build()
    }
}

/// 创建按钮。
///
/// ```ignore
/// button("保存").primary().on_click(&state, |s| save(s));
/// button("关闭").on_click_fn(|| close());
/// ```
pub fn button(text: impl Into<String>) -> ButtonBuilder {
    let config = crate::ui::widget_runtime::config::use_config();
    ButtonBuilder {
        text: text.into(),
        style_set: config
            .overrides
            .button
            .style_set
            .map(Arc::new)
            .unwrap_or_else(Button::default_preset_style_set),
        disabled: config.disabled,
        block: false,
        size: config.size,
        // 默认不进入加载态。
        loading: false,
        handlers: Vec::new(),
    }
}

// ── 输入框构建器 ────────────────────────────────────────────

/// 输入框构建器。通过 `input().placeholder("...").on_change(fn)` 链式调用。
///
/// # 示例
///
/// ```ignore
/// input().placeholder("请输入用户名").on_change(|v| println!("输入: {}", v))
/// ```
pub struct InputBuilder {
    placeholder: String,
    value: Option<State<String>>,
    handlers: Vec<HandlerRegistration>,
}

impl InputBuilder {
    /// 设置占位文本。
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// 绑定输入值；组件编辑与外部 `State` 更新保持双向同步。
    pub fn value(mut self, state: &State<String>) -> Self {
        self.value = Some(state.clone());
        self
    }

    /// 绑定值变更回调（输入内容变化时触发）。
    pub fn on_change<F: FnMut(&str) + 'static>(mut self, mut f: F) -> Self {
        self.handlers.push(HandlerRegistration::new(
            SemanticKind::Change,
            Box::new(move |event| {
                if let Some(value) = event.text_payload() {
                    f(value);
                }
            }),
        ));
        self
    }

    /// 绑定值变更回调，并用指定状态建立稳定捕获指纹。
    pub fn on_change_capture<T, F>(
        mut self,
        state: &crate::ui::reactive::state::State<T>,
        mut f: F,
    ) -> Self
    where
        T: Clone + Send + Sync + 'static,
        F: FnMut(&str) + 'static,
    {
        self.handlers.push(
            HandlerRegistration::new(
                SemanticKind::Change,
                Box::new(move |event| {
                    if let Some(value) = event.text_payload() {
                        f(value);
                    }
                }),
            )
            .with_state_capture(state),
        );
        self
    }

    /// 绑定值变更回调，并用窗口标识建立捕获指纹。
    pub fn on_change_window_capture<F>(mut self, window_id: crate::core::WindowId, mut f: F) -> Self
    where
        F: FnMut(&str) + 'static,
    {
        self.handlers.push(
            HandlerRegistration::new(
                SemanticKind::Change,
                Box::new(move |event| {
                    if let Some(value) = event.text_payload() {
                        f(value);
                    }
                }),
            )
            .with_window_capture(window_id),
        );
        self
    }
}

impl View for InputBuilder {
    fn build(self) -> ViewNode {
        let mut input = crate::ui::widgets::Input::new(self.placeholder);
        if let Some(value) = self.value.as_ref() {
            input = input.value(value);
        }
        let mut node = ViewNode::leaf(input);
        node.handlers = self.handlers;
        node
    }
}

impl From<InputBuilder> for ViewNode {
    fn from(builder: InputBuilder) -> Self {
        builder.build()
    }
}

/// 创建输入框。返回 `InputBuilder` 以链式设置属性。
///
/// # 示例
///
/// ```ignore
/// input().placeholder("搜索...").on_change(|v| search(v))
/// ```
pub fn input() -> InputBuilder {
    InputBuilder {
        placeholder: String::new(),
        value: None,
        handlers: Vec::new(),
    }
}
