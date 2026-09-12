//! Component-independent view composition and low-level paint primitives.
use crate::core::{Constraints, Rect, Size};
use crate::ui::{WidgetTree, View, ViewNode, State, Computed, PaintContext, Widget, WidgetCapabilities, WidgetLayout, WidgetRender};
use std::any::Any;
#[path = "embed.rs"]
mod embed_node;

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
impl View for () {
    fn build(self) -> ViewNode {
        ViewNode::leaf(EmptyView)
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
impl<T: View> View for Option<T> {
    fn build(self) -> ViewNode {
        match self {
            Some(child) => child.build(),
            None => ViewNode::leaf(EmptyView),
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
        dynamic_label(move || f(&state.get()))
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
        dynamic_label(move || f(&computed.get()))
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


struct EmptyView;
impl Widget for EmptyView {
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
    fn into_any(self: Box<Self>) -> Box<dyn Any> { self }
    fn capabilities(&self) -> WidgetCapabilities { WidgetCapabilities::new() }
    fn snapshot_fields(&self) -> crate::ui::WidgetSnapshotFields { crate::ui::WidgetSnapshotFields::custom("EmptyView", Vec::new()) }
}
/// A reactive text primitive whose dependencies invalidate only its own output.
pub fn dynamic_label<F: Fn() -> String + 'static>(value: F) -> ViewNode {
    ViewNode::leaf(crate::ui::widget_runtime::dynamic_label::DynamicLabel::new(value))
}
/// A reactive single-line text primitive with constrained-width ellipsis.
pub fn elided_label<F: Fn() -> String + 'static>(value: F) -> ViewNode {
    ViewNode::leaf(crate::ui::widget_runtime::dynamic_label::DynamicLabel::new(value).elided())
}

// 真实 GPU 验证仍经过框架 Canvas 的 WidgetRender 入口。
#[cfg(any(uix_gpu_parity_vulkan, uix_gpu_parity_opengl, uix_gpu_parity_d3d11))]
include!("../../../../tests-src/ui/widgets/combinators_parity_fns.rs");
