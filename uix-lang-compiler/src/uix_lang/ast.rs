// 引入受限表达式与顶层声明 AST。
use super::{Declaration, Expression, StyleProperty};

// 表示源码中的半开字节区间及其一基行列位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SourceSpan {
    // 保存起始 UTF-8 字节偏移。
    pub(crate) start: usize,
    // 保存结束 UTF-8 字节偏移。
    pub(crate) end: usize,
    // 保存起始的一基行号。
    pub(crate) line: usize,
    // 保存起始的一基字符列号。
    pub(crate) column: usize,
}

// 表示一个已经通过唯一根约束的 UIX 文档。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Document {
    // 保存根元素之前的顶层声明顺序。
    pub(crate) declarations: Vec<Declaration>,
    // 保存文档的唯一根元素。
    pub(crate) root: Element,
}

// 表示标签元素及其属性和有序子节点。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Element {
    // 保存 PascalCase 标签名。
    pub(crate) name: String,
    // 保存声明顺序中的属性。
    pub(crate) attributes: Vec<Attribute>,
    // 保存 painter 与组合语义所需的子节点顺序。
    pub(crate) children: Vec<Node>,
    // 保存 If 或 For 元素专用的控制绑定。
    pub(crate) control: Option<ControlBinding>,
    // 保存从开始标签到结束标签的完整跨度。
    pub(crate) span: SourceSpan,
    // 保存由组件展开附加到实际 View 根节点的私有状态作用域标记。
    pub(crate) widget_scopes: Vec<WidgetScopeMarker>,
    // 保存 For 每次迭代都必须重新克隆的拥有型事件捕获名称。
    pub(crate) for_iteration_clones: Vec<String>,
    // 保存 For 每次迭代在构建实际子树前执行的组件准备语句。
    pub(crate) for_iteration_setup: Vec<String>,
    // 保存 reactive 组件的作用域准备语句；None 表示普通组件，
    // Some 表示组件根会被包进独立捕获帧的 scoped 闭包重建。
    pub(crate) reactive_setup: Option<Vec<String>>,
}

// 表示一个应在最终 ViewNode 外层应用的组件状态装饰。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WidgetScopeMarker {
    // 保存普通组件根的生命周期作用域标记。
    Scope {
        // 保存生成阶段创建的卫生作用域局部变量名称。
        scope_name: String,
        // 保存组件多根输出中的稳定根序号。
        root_ordinal: u64,
    },
    // 保存需要包裹事件 View 的动态样式状态。
    DynamicStyle(DynamicStyleBinding),
    // 保存自动状态伪类的叠加样式与既有事实读取。
    PseudoStyle(PseudoStyleBinding),
    // 保存静态 animation 声明的持久化播放绑定。
    Animation(AnimationBinding),
    // 保存 transition 声明的持久化目标比较绑定。
    Transition(TransitionBinding),
}

// 保存一个元素 animation 声明的运行时作用域与类型化关键帧。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AnimationBinding {
    // 保存最近组件或文档根状态作用域名称。
    pub(crate) widget_scope_name: String,
    // 保存节点类型与静态位置形成的子作用域声明标识。
    pub(crate) declaration_id: u64,
    // 保存当前节点所属最近 For 实例路径。
    pub(crate) instance_path_name: Option<String>,
    // 保存 animation 简写解析出的完整播放配置。
    pub(crate) playback: AnimationPlayback,
    // 保存按支持矩阵确定顺序排列的属性关键帧。
    pub(crate) properties: Vec<AnimationPropertyBinding>,
}

// 保存 animation 简写的确定性播放配置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AnimationPlayback {
    // 保存单轮时长的微秒整数。
    pub(crate) duration_micros: u64,
    // 保存启动延迟的微秒整数。
    pub(crate) delay_micros: u64,
    // 保存有限总轮数；None 表示 infinite。
    pub(crate) iterations: Option<u64>,
    // 保存统一片段缓动函数。
    pub(crate) easing: AnimationEasing,
    // 保存每轮方向。
    pub(crate) direction: AnimationDirection,
    // 保存延迟期与完成后的填充模式。
    pub(crate) fill_mode: AnimationFillMode,
}

// 保存 animation 支持的闭合缓动函数集合。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AnimationEasing {
    // 保存线性插值。
    Linear,
    // 保存 CSS 默认 ease 曲线。
    Ease,
    // 保存缓入曲线。
    EaseIn,
    // 保存缓出曲线。
    EaseOut,
    // 保存缓入缓出曲线。
    EaseInOut,
}

// 保存 animation 支持的闭合方向集合。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AnimationDirection {
    // 每轮正向播放。
    Normal,
    // 每轮倒向播放。
    Reverse,
    // 从正向开始交替播放。
    Alternate,
    // 从倒向开始交替播放。
    AlternateReverse,
}

// 保存 animation 支持的闭合填充模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AnimationFillMode {
    // 延迟期与完成后都恢复基础值。
    None,
    // 完成后保留终值。
    Forwards,
    // 延迟期显示首帧值。
    Backwards,
    // 同时应用 backwards 与 forwards。
    Both,
}

// 保存一个可动画 Style 字段的基础值与有序关键帧。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AnimationPropertyBinding {
    // 保存闭合可动画字段类型。
    pub(crate) kind: AnimationPropertyKind,
    // 保存 fill-mode none/backwards 需要恢复的基础值。
    pub(crate) baseline: StyleProperty,
    // 保存仅包含当前字段的关键帧序列。
    pub(crate) frames: Vec<AnimationPropertyFrame>,
}

// 保存单个字段在一个偏移处的类型化前置值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AnimationPropertyFrame {
    // 保存零到一百万闭区间内的时间偏移。
    pub(crate) offset_millionths: u32,
    // 保存待由字段专用生成器解析的样式值。
    pub(crate) property: StyleProperty,
}

// 保存首批与现有 Animated View 绑定对应的 UIX 样式字段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum AnimationPropertyKind {
    // 显式宽度。
    Width,
    // 显式高度。
    Height,
    // 统一圆角半径。
    BorderRadius,
    // 节点透明度。
    Opacity,
    // 前景或文本颜色。
    Color,
    // 普通背景颜色。
    BackgroundColor,
}

// 保存一个元素 transition 声明的状态作用域、播放配置与字段集合。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TransitionBinding {
    // 保存最近组件或文档根状态作用域名称。
    pub(crate) widget_scope_name: String,
    // 保存节点类型与静态位置形成的子作用域声明标识。
    pub(crate) declaration_id: u64,
    // 保存当前节点所属最近 For 实例路径。
    pub(crate) instance_path_name: Option<String>,
    // 保存过渡时长的微秒整数。
    pub(crate) duration_micros: u64,
    // 保存启动延迟的微秒整数。
    pub(crate) delay_micros: u64,
    // 保存闭合缓动曲线。
    pub(crate) easing: AnimationEasing,
    // 保存实际参与目标比较的闭合字段集合。
    pub(crate) properties: Vec<AnimationPropertyKind>,
}

// 保存一个元素的状态伪类叠加元数据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PseudoStyleBinding {
    // hover 存在时保存最近组件作用域名称。
    pub(crate) hover_scope_name: Option<String>,
    // 保存 hover 私有状态子作用域的稳定声明标识。
    pub(crate) declaration_id: u64,
    // 保存当前节点所属最近 For 实例路径。
    pub(crate) instance_path_name: Option<String>,
    // 保存 hover 只声明的差异字段。
    pub(crate) hover: Vec<StyleProperty>,
    // 保存 disabled 事实与差异字段。
    pub(crate) disabled: Option<(PseudoStyleCondition, Vec<StyleProperty>)>,
    // 保存 checked 事实与差异字段。
    pub(crate) checked: Option<(PseudoStyleCondition, Vec<StyleProperty>)>,
}

// 表示伪类选择使用的既有布尔事实形状。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PseudoStyleCondition {
    // 保存静态布尔属性。
    Literal(bool),
    // 保存已经降低为布尔值的表达式。
    Value(ExpressionNode),
    // 保存需要读取 get() 的 State<bool> 表达式。
    State(ExpressionNode),
}

// 表示标签上的一个具名属性。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Attribute {
    // 保存属性名。
    pub(crate) name: String,
    // 保存字面值或表达式值。
    pub(crate) value: AttributeValue,
    // 保存完整属性跨度。
    pub(crate) span: SourceSpan,
}

// 区分双引号属性字面量与花括号表达式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AttributeValue {
    // 保存已经去除双引号的字面值。
    Literal(String),
    // 保存已经完成受限语法验证的表达式。
    Expression(ExpressionNode),
    // 保存使用共享样式语法解析的内联属性。
    InlineStyle(Vec<StyleProperty>),
}

// 保存一个动态样式节点的组件状态与稳定身份元数据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DynamicStyleBinding {
    // 保存最近组件作用域的卫生局部变量名称。
    pub(crate) widget_scope_name: String,
    // 保存节点类型与静态位置共同形成的子作用域声明标识。
    pub(crate) declaration_id: u64,
    // 保存子作用域内动态样式状态的稳定字段标识。
    pub(crate) field_id: u64,
    // 保存生成的闭合样式枚举名称。
    pub(crate) enum_name: String,
    // 保存当前节点所属最近 For 实例路径的卫生名称。
    pub(crate) instance_path_name: Option<String>,
    // 保存参与实际节点身份的可选 key。
    pub(crate) key: Option<DynamicStyleKey>,
    // 保存原始类与所有可达目标类的类型化分支。
    pub(crate) variants: Vec<DynamicStyleVariant>,
}

// 保存动态样式节点用于 reconcile 的显式 key。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DynamicStyleKey {
    // 保存字符串字面量 key。
    Literal(String),
    // 保存已经完成组件字段改写的表达式 key。
    Expression(ExpressionNode),
}

// 保存一个动态样式枚举分支及其零参数状态写入器。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DynamicStyleVariant {
    // 保存生成枚举变体的卫生名称。
    pub(crate) variant_name: String,
    // 保存每个调用点独占的卫生 setter 名称，避免事件闭包重复移动。
    pub(crate) setter_names: Vec<String>,
    // 保存类层与内联层合并后的完整 Style 字段更新。
    pub(crate) properties: Vec<StyleProperty>,
}

// 表示条件渲染或循环元素的专用绑定。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ControlBinding {
    // 保存 If 条件表达式。
    If(ExpressionNode),
    // 保存 For 单标识符绑定与数据源。
    For {
        // 保存循环项绑定名称。
        binding: String,
        // 保存绑定声明跨度。
        binding_span: SourceSpan,
        // 保存可选索引绑定名称。
        index_binding: Option<String>,
        // 保存可选索引绑定声明跨度。
        index_span: Option<SourceSpan>,
        // 保存数据源表达式。
        iterable: ExpressionNode,
        // 保存可选稳定行身份表达式。
        key: Option<ExpressionNode>,
    },
}

// 表示元素内部的有序节点联合。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Node {
    // 保存嵌套元素。
    Element(Element),
    // 保存普通文本。
    Text(TextNode),
    // 保存文本插值表达式。
    Interpolation(ExpressionNode),
    // 保存 Widget 模板内的具名成员声明块。
    WidgetMember(WidgetMemberBlock),
}

// 表示 <Widget> 模板内部的 @props / @state / @computed / @actions 声明块。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WidgetMemberBlock {
    // 保存成员类别。
    pub(crate) member: WidgetMemberKind,
    // 保存去除外围空白后的声明体源码，语法与字符串属性形式一致。
    pub(crate) body: String,
    // 保存包含 @ 名称与花括号的完整跨度。
    pub(crate) span: SourceSpan,
}

// 区分 Widget 模板允许的成员声明块类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WidgetMemberKind {
    // 对应 props 属性形式。
    Props,
    // 对应 state 属性形式。
    State,
    // 对应 computed 属性形式。
    Computed,
    // 对应 actions 属性形式。
    Actions,
}

// 实现成员类别的规范名称映射。
impl WidgetMemberKind {
    // 返回与属性形式一致的成员名称。
    pub(crate) const fn as_str(self) -> &'static str {
        // 按闭合集合返回名称。
        match self {
            // 返回 props 名称。
            Self::Props => "props",
            // 返回 state 名称。
            Self::State => "state",
            // 返回 computed 名称。
            Self::Computed => "computed",
            // 返回 actions 名称。
            Self::Actions => "actions",
        }
    }
}

// 表示已经处理转义花括号的文本节点。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextNode {
    // 保存用户可见文本。
    pub(crate) value: String,
    // 保存原始文本跨度。
    pub(crate) span: SourceSpan,
}

// 表示保留原始内容的表达式节点。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExpressionNode {
    // 保存去除外层花括号后的源码。
    pub(crate) source: String,
    // 保存确定性表达式 AST。
    pub(crate) expression: Expression,
    // 保存包含花括号的完整跨度。
    pub(crate) span: SourceSpan,
}
