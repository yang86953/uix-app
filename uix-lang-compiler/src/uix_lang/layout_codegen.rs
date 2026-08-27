// 引入过程宏令牌类型。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::TokenStream;
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入核心 View 生成器拥有的有序子树与公共属性映射。
use super::codegen::{
    apply_common_attributes, generate_children, generate_node_view, is_renderable_node,
};
// 引入 Container 专属简写生成与消费登记。
use super::container_style_codegen::{CONTAINER_CONSUMED_ATTRIBUTES, apply_container_shorthands};
// 引入布局映射所需的语言 AST、诊断与共享属性值解析。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, Node, SourceSpan, generate_expression,
    literal_string, numeric_value,
};

// 生成通用 Flex Container。
pub(crate) fn generate_container(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Container 默认采用纵向 Flex。
    let mut row = false;
    // 查找可选方向属性。
    if let Some(attribute) = find_attribute(element, "direction") {
        // 方向必须在编译期确定。
        let direction = literal_string(attribute, "Container direction")?;
        // 映射文档登记的两个方向。
        row = match direction.as_str() {
            // row 选择现有横向 Flex 构造器。
            "row" => true,
            // column 保持现有纵向 Flex 构造器。
            "column" => false,
            // 其他方向不得静默猜测。
            _ => {
                // 返回带来源位置的方向诊断。
                return Err(Diagnostic::new(
                    // 指向非法方向属性。
                    attribute.span,
                    // 说明具体非法值。
                    format!("Container direction={direction:?} 不受支持"),
                    // 给出文档登记集合。
                    "使用 direction=\"row\" 或 direction=\"column\"",
                ));
            }
        };
    }
    // 生成保持源码顺序的子节点向量。
    let children = generate_children(&element.children)?;
    // 按已验证方向复用公开 Flex 构造器。
    let mut base = if row {
        // 横向 Container 继续使用公开 row 函数。
        quote! { ::uix::prelude::row(#children) }
    } else {
        // UIX 默认 flexGrow=0，纵向 Container 必须保持内容固有高度。
        quote! { ::uix::prelude::column_fit(#children) }
    };
    // 按源码顺序应用 Container 专属简写属性。
    base = apply_container_shorthands(base, element)?;
    // 消费专属简写后应用统一公共属性。
    apply_common_attributes(
        // 传入已应用专属属性的容器 View。
        base,
        // 保留原始属性供公共映射处理。
        &element.attributes,
        // 标记本阶段已经消费的属性集合。
        CONTAINER_CONSUMED_ATTRIBUTES,
    )
}

// 生成保留兼容性的显式 Flex Column。
pub(crate) fn generate_column(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Column 已固定方向，不允许重复声明 direction。
    reject_fixed_direction(element)?;
    // 生成保持源码顺序的子节点向量。
    let children = generate_children(&element.children)?;
    // UIX 默认 flexGrow=0；显式 flexGrow 属性由公共属性阶段覆盖。
    let base = quote! { ::uix::prelude::column_fit(#children) };
    // 应用统一公共属性。
    apply_common_attributes(base, &element.attributes, &[])
}

// 生成持有滚动生命周期与偏移状态的 ScrollView。
pub(crate) fn generate_scroll_view(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 滚动构建器只接收一个内容 View，避免宏层引入隐式布局容器。
    let child = single_scroll_child(element)?;
    // 递归生成唯一内容 View。
    let child = generate_node_view(child)?;
    // 默认方向直接复用公开 scroll 构造器的垂直契约。
    let mut base = quote! { ::uix::prelude::scroll(#child) };
    // 可选方向必须在编译期确定。
    if let Some(attribute) = find_attribute(element, "direction") {
        // 读取文档登记的方向关键字。
        let direction = literal_string(attribute, "ScrollView direction")?;
        // 把关键字映射为公开 ScrollBuilder 方法。
        base = match direction.as_str() {
            // vertical 显式保持默认方向。
            "vertical" => quote! { (#base).vertical() },
            // horizontal 切换为横向滚动。
            "horizontal" => quote! { (#base).horizontal() },
            // both 同时开放两条滚动轴。
            "both" => quote! { (#base).both() },
            // 未登记关键字不能静默回退。
            _ => {
                // 返回带属性位置的确定性诊断。
                return Err(Diagnostic::new(
                    // 指向非法方向属性。
                    attribute.span,
                    // 保留用户输入值以便修复。
                    format!("ScrollView direction={direction:?} 不受支持"),
                    // 给出完整合法关键字集合。
                    "使用 direction=\"vertical\"、direction=\"horizontal\" 或 direction=\"both\"",
                ));
            }
        };
    }
    // 可选 offset 必须双向绑定现有 State<Point>。
    if let Some(attribute) = find_attribute(element, "offset") {
        // 绑定只接受表达式形状，字符串不能表达状态句柄。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回绑定形状诊断。
            return Err(Diagnostic::new(
                // 指向非法 offset 属性。
                attribute.span,
                // 说明公开运行时要求 State<Point>。
                "ScrollView offset 必须绑定 State<Point> 表达式",
                // 给出最小合法写法。
                "使用 offset={scroll_offset}",
            ));
        };
        // 把受限绑定表达式生成为 Rust 值。
        let offset = generate_expression(&expression.expression, None)?;
        // 公开构建器借用并克隆 State，运行态仍由 ScrollView 与应用状态共同持有。
        base = quote! { (#base).scroll_offset(&(#offset)) };
    }
    // 消费专有属性后，把尺寸、样式和自动化属性交给统一契约。
    apply_common_attributes(base, &element.attributes, &["direction", "offset"])
}

// 提取 ScrollView 唯一的可渲染直接子节点。
fn single_scroll_child(element: &Element) -> Result<&Node, Diagnostic> {
    // 忽略用于排版源码的纯空白文本。
    let mut children = element
        // 遍历原始直接子节点。
        .children
        // 借用节点以保持源码顺序和位置。
        .iter()
        // 只保留会生成 View 的节点。
        .filter(|child| is_renderable_node(child));
    // 空滚动容器没有可建立内容范围的子 View。
    let Some(child) = children.next() else {
        // 返回完整元素位置上的缺失内容诊断。
        return Err(Diagnostic::new(
            // 指向空 ScrollView。
            element.span,
            // 说明单子节点契约。
            "<ScrollView> 必须包含一个可渲染直接子节点",
            // 给出显式内容容器示例。
            "在 ScrollView 内放置一个 Column、Container、Grid 或其他 View",
        ));
    };
    // 第二个可渲染节点会破坏 ScrollBuilder 的单内容所有权。
    if children.next().is_some() {
        // 返回父元素位置上的形状诊断。
        return Err(Diagnostic::new(
            // 指向多子节点 ScrollView。
            element.span,
            // 说明不能隐式包裹多个内容节点。
            "<ScrollView> 只能包含一个可渲染直接子节点",
            // 引导调用方显式选择内容布局语义。
            "用 Column、Container、Row 或 Grid 包裹多个内容节点",
        ));
    }
    // 返回唯一内容 View。
    Ok(child)
}

// 生成文档定义的 Row/Col 24 单元响应式栅格。
pub(crate) fn generate_row(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Row 已固定为栅格语义，不接受旧 direction 属性。
    reject_fixed_direction(element)?;
    // 收集并验证直接 Col 子项。
    let columns = direct_columns(element, "Row")?;
    // 保存与配置一一对应的有序列 View。
    let mut views = Vec::with_capacity(columns.len());
    // 保存源码顺序中的响应式列配置。
    let mut configs = Vec::with_capacity(columns.len());
    // 逐列生成内容与现有运行时 Col 配置。
    for column in columns {
        // 生成单一列内容 View。
        views.push(generate_column_view(column, ROW_COL_ATTRIBUTES)?);
        // 生成 24 栅格结构配置。
        configs.push(generate_responsive_col_config(column)?);
    }
    // 只通过公开 GridBuilder 组合现有运行时布局能力。
    let base = quote! {
        // 创建源码顺序中的列 View。
        (::uix::prelude::grid(::std::vec![#(#views),*]))
            // 启用现有 24 单元响应式轨道。
            .responsive()
            // 绑定与子 View 一一对应的列配置。
            .cols(::std::vec![#(#configs),*])
    };
    // gap、align、justify 与样式继续走统一公开 View 契约。
    apply_common_attributes(base, &element.attributes, &[])
}

// 生成文档定义的显式 Grid/Col 轨道布局。
pub(crate) fn generate_grid(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 显式 Grid 必须声明列轨道，避免生成无轨道占位布局。
    let columns_attribute = find_attribute(element, "columns").ok_or_else(|| {
        // 返回缺失结构属性诊断。
        Diagnostic::new(
            // 指向完整 Grid。
            element.span,
            // 说明缺少列轨道。
            "<Grid> 缺少必需的 columns 属性",
            // 给出文档示例。
            "使用 columns=\"1fr 1fr\"、columns=\"200px 1fr\" 或其他 px/fr/auto 轨道",
        )
    })?;
    // 把列轨道字面量转换为公开 GridTrack 向量。
    let columns = generate_track_vector(columns_attribute, "Grid columns")?;
    // 收集并验证直接 Col 子项。
    let column_elements = direct_columns(element, "Grid")?;
    // 保存显式轨道中的有序列 View。
    let mut views = Vec::with_capacity(column_elements.len());
    // 逐列生成带跨轨道样式的单一 View。
    for column in column_elements {
        // 生成 Grid 专用 Col 包装 View。
        views.push(generate_explicit_grid_col(column)?);
    }
    // 先建立公开 GridBuilder 与列轨道。
    let mut base = quote! {
        // 创建源码顺序中的列 View 并声明列轨道。
        (::uix::prelude::grid(::std::vec![#(#views),*])).columns(#columns)
    };
    // 可选 rows 使用同一轨道解析契约。
    if let Some(attribute) = find_attribute(element, "rows") {
        // 生成公开行轨道向量。
        let rows = generate_track_vector(attribute, "Grid rows")?;
        // 在同一 GridBuilder 上声明行轨道。
        base = quote! { (#base).rows(#rows) };
    }
    // 统一间距先进入现有 GridBuilder，独立轴间距随后覆盖。
    if let Some(attribute) = find_attribute(element, "gap") {
        // 解析长度或受限数值表达式。
        let value = numeric_value(attribute)?;
        // 同时更新运行时行列间距与公开 Style。
        base = quote! { (#base).gap(#value) };
    }
    // 可选独立列间距进入现有 GridBuilder。
    if let Some(attribute) = find_attribute(element, "colGap") {
        // 解析长度或受限数值表达式。
        let value = numeric_value(attribute)?;
        // 更新运行时列间距与公开 Style。
        base = quote! { (#base).col_gap(#value) };
    }
    // 可选独立行间距进入现有 GridBuilder。
    if let Some(attribute) = find_attribute(element, "rowGap") {
        // 解析长度或受限数值表达式。
        let value = numeric_value(attribute)?;
        // 更新运行时行间距与公开 Style。
        base = quote! { (#base).row_gap(#value) };
    }
    // 结构属性消费后，padding/class/style 走统一公开契约。
    apply_common_attributes(
        // 传递已配置的 GridBuilder。
        base,
        // 传递源码属性列表。
        &element.attributes,
        // 排除已经由布局映射消费的属性。
        &["columns", "rows", "gap", "colGap", "rowGap"],
    )
}

// 为脱离 Row/Grid 的 Col 返回父级语义诊断。
pub(crate) fn generate_orphan_col(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Col 的结构配置只有父布局能够解释。
    Err(Diagnostic::new(
        // 指向完整孤立 Col。
        element.span,
        // 说明父级契约缺失。
        "<Col> 只能作为 <Row> 或 <Grid> 的直接子项",
        // 给出两种合法结构。
        "把 Col 放入 <Row>...</Row> 24 栅格，或放入带 columns 的 <Grid>...</Grid>",
    ))
}

// 保存 Row 下 Col 可消费的结构属性名。
const ROW_COL_ATTRIBUTES: &[&str] = &[
    // 基础跨度。
    "span",   // 基础偏移。
    "offset", // 视觉顺序。
    "order",  // 小屏跨度。
    "sm",     // 中屏跨度。
    "md",     // 大屏跨度。
    "lg",     // 超大屏跨度。
    "xl",     // 最大屏跨度。
    "xxl",
];

// 保存 Grid 下 Col 可消费的结构属性名。
const GRID_COL_ATTRIBUTES: &[&str] = &[
    // 文档简写列跨度。
    "span",
    // 显式列跨度。
    "gridColumnSpan",
    // 显式行跨度。
    "gridRowSpan",
];

// 拒绝固定方向标签上的 direction 属性。
fn reject_fixed_direction(element: &Element) -> Result<(), Diagnostic> {
    // 查找冲突方向属性。
    let Some(attribute) = find_attribute(element, "direction") else {
        // 没有冲突即可继续。
        return Ok(());
    };
    // 返回固定语义冲突诊断。
    Err(Diagnostic::new(
        // 指向冲突属性。
        attribute.span,
        // 说明当前标签语义已经固定。
        format!("<{}> 已固定布局语义", element.name),
        // 指向 Flex 横向兼容入口。
        "删除 direction；需要 Flex 横向布局时使用 <Container direction=\"row\">",
    ))
}

// 收集只含格式化空白与直接 Col 的父布局子项。
fn direct_columns<'a>(
    // 接收 Row 或 Grid 元素。
    element: &'a Element,
    // 接收诊断中的父标签名称。
    parent: &str,
) -> Result<Vec<&'a Element>, Diagnostic> {
    // 保存源码顺序中的 Col 引用。
    let mut columns = Vec::new();
    // 检查每个直接子节点。
    for child in &element.children {
        // 格式化空白不生成布局子项。
        if !is_renderable_node(child) {
            // 继续检查下一子节点。
            continue;
        }
        // 只接受直接 Col 元素。
        if let Node::Element(column) = child {
            // 匹配文档规定的 Col 标签。
            if column.name == "Col" {
                // 保持源码顺序保存列。
                columns.push(column);
                // 继续检查下一子节点。
                continue;
            }
        }
        // 其他可见节点不能被静默包成列。
        return Err(Diagnostic::new(
            // 指向非法直接子节点。
            node_span(child),
            // 说明父级形状要求。
            format!("<{parent}> 只接受直接 <Col> 子项与格式化空白"),
            // 给出显式列包装修复。
            format!("用 <Col>...</Col> 包裹该节点后再放入 <{parent}>"),
        ));
    }
    // 返回已验证的有序列集合。
    Ok(columns)
}

// 生成 Row 下响应式 Col 的公开配置链。
fn generate_responsive_col_config(column: &Element) -> Result<TokenStream, Diagnostic> {
    // 从现有运行时默认 24 跨度开始。
    let mut config = quote! { ::uix::prelude::Col::new() };
    // 按源码顺序应用全部结构属性。
    for attribute in &column.attributes {
        // 映射基础与响应式跨度。
        config = match attribute.name.as_str() {
            // 基础跨度限制为 1 到 24。
            "span" => chain_u32(config, "span", constant_u32(attribute, "Col span", 1, 24)?),
            // 偏移遵守文档 0 到 24 范围。
            "offset" => chain_u32(
                config,
                "offset",
                constant_u32(attribute, "Col offset", 0, 24)?,
            ),
            // 视觉顺序接受完整 i32。
            "order" => {
                // 解析编译期整数顺序。
                let value = constant_i32(attribute, "Col order")?;
                // 调用现有顺序构建器。
                quote! { (#config).order(#value) }
            }
            // sm 响应式跨度。
            "sm" => chain_u32(config, "sm", constant_u32(attribute, "Col sm", 1, 24)?),
            // md 响应式跨度。
            "md" => chain_u32(config, "md", constant_u32(attribute, "Col md", 1, 24)?),
            // lg 响应式跨度。
            "lg" => chain_u32(config, "lg", constant_u32(attribute, "Col lg", 1, 24)?),
            // xl 响应式跨度。
            "xl" => chain_u32(config, "xl", constant_u32(attribute, "Col xl", 1, 24)?),
            // xxl 响应式跨度。
            "xxl" => chain_u32(config, "xxl", constant_u32(attribute, "Col xxl", 1, 24)?),
            // 公共属性由列 View 映射消费。
            _ => config,
        };
    }
    // 返回现有运行时 Col 配置表达式。
    Ok(config)
}

// 生成显式 Grid 下带跨度的 Col View。
fn generate_explicit_grid_col(column: &Element) -> Result<TokenStream, Diagnostic> {
    // span 与 gridColumnSpan 不能同时声明两个列跨度来源。
    if find_attribute(column, "span").is_some()
        // 同时检查显式列跨度。
        && find_attribute(column, "gridColumnSpan").is_some()
    {
        // 选择显式属性作为诊断位置。
        let attribute = find_attribute(column, "gridColumnSpan")
            // 前置条件保证属性存在。
            .expect("已确认存在 gridColumnSpan");
        // 返回结构冲突诊断。
        return Err(Diagnostic::new(
            // 指向重复跨度属性。
            attribute.span,
            // 说明两个属性含义相同。
            "Grid 下的 Col 不能同时声明 span 与 gridColumnSpan",
            // 给出确定修复。
            "只保留 span 或 gridColumnSpan 其中一个",
        ));
    }
    // 优先读取显式列跨度，再读取简写 span。
    let column_span = find_attribute(column, "gridColumnSpan")
        // 缺少显式属性时回退到简写。
        .or_else(|| find_attribute(column, "span"))
        // 解析存在的正整数。
        .map(|attribute| constant_u32(attribute, "Grid Col column span", 1, u32::MAX))
        // 转置可选解析结果。
        .transpose()?
        // 默认只占一个显式轨道。
        .unwrap_or(1);
    // 读取可选行跨度。
    let row_span = find_attribute(column, "gridRowSpan")
        // 解析存在的正整数。
        .map(|attribute| constant_u32(attribute, "Grid Col row span", 1, u32::MAX))
        // 转置可选解析结果。
        .transpose()?
        // 默认只占一行。
        .unwrap_or(1);
    // 生成列内容与公共样式。
    let view = generate_column_view(column, GRID_COL_ATTRIBUTES)?;
    // 把跨度交给现有公开 ViewNode Grid 样式契约。
    Ok(quote! { (#view).grid_span(#column_span, #row_span) })
}

// 把一个 Col 的全部内容物化为单一有序列 View。
fn generate_column_view(
    // 接收 Col 元素。
    column: &Element,
    // 接收当前父布局消费的结构属性。
    consumed: &[&str],
) -> Result<TokenStream, Diagnostic> {
    // 生成 Col 内保持源码顺序的子节点。
    let children = generate_children(&column.children)?;
    // Col 内容默认保持固有高度，与 UIX flexGrow=0 契约一致。
    let base = quote! { ::uix::prelude::column_fit(#children) };
    // 结构属性由父布局消费，其余属性进入统一 View 映射。
    apply_common_attributes(base, &column.attributes, consumed)
}

// 生成 px/fr/auto 显式轨道向量。
fn generate_track_vector(
    // 接收 columns 或 rows 属性。
    attribute: &Attribute,
    // 接收诊断中的契约名称。
    contract: &str,
) -> Result<TokenStream, Diagnostic> {
    // 轨道列表必须是确定字符串字面量。
    let source = literal_string(attribute, contract)?;
    // 保存源码顺序中的公开 GridTrack 表达式。
    let mut tracks = Vec::new();
    // 按空白切分文档规定的轨道列表。
    for track in source.split_whitespace() {
        // auto 直接映射自动轨道。
        if track == "auto" {
            // 保存自动轨道表达式。
            tracks.push(quote! { ::uix::prelude::GridTrack::Auto });
            // 继续下一轨道。
            continue;
        }
        // 识别 fr 或 px 后缀并选择公开构造器。
        let (number, constructor) = if let Some(number) = track.strip_suffix("fr") {
            // 弹性比例轨道。
            (number, "fr")
        } else if let Some(number) = track.strip_suffix("px") {
            // 固定像素轨道。
            (number, "px")
        } else {
            // 未登记单位必须在宏展开期失败。
            return Err(Diagnostic::new(
                // 指向完整轨道属性。
                attribute.span,
                // 说明具体非法轨道。
                format!("{contract} 轨道 {track:?} 缺少 px/fr 单位"),
                // 给出登记格式。
                "使用 100px、1fr 或 auto，并以空格分隔轨道",
            ));
        };
        // 解析有限非负轨道数值。
        let value = number.parse::<f32>().map_err(|_| {
            // 构造数值格式诊断。
            Diagnostic::new(
                // 指向完整轨道属性。
                attribute.span,
                // 说明具体非法数值。
                format!("{contract} 轨道 {track:?} 不是有效数字"),
                // 给出登记格式。
                "使用 100px、1fr 或 auto",
            )
        })?;
        // 非有限值与负值不进入运行时布局。
        if !value.is_finite() || value < 0.0 {
            // 返回范围诊断。
            return Err(Diagnostic::new(
                // 指向完整轨道属性。
                attribute.span,
                // 说明有限非负约束。
                format!("{contract} 轨道 {track:?} 必须是有限非负值"),
                // 给出修复格式。
                "使用非负有限的 px/fr 数值或 auto",
            ));
        }
        // 按单位生成公开 GridTrack 变体。
        if constructor == "fr" {
            // 保存弹性轨道。
            tracks.push(quote! { ::uix::prelude::GridTrack::Fr(#value) });
        } else {
            // 保存固定像素轨道。
            tracks.push(quote! { ::uix::prelude::GridTrack::Px(#value) });
        }
    }
    // 空轨道列表没有可执行布局语义。
    if tracks.is_empty() {
        // 返回缺失轨道诊断。
        return Err(Diagnostic::new(
            // 指向空属性。
            attribute.span,
            // 说明轨道列表为空。
            format!("{contract} 轨道列表不能为空"),
            // 给出最小修复。
            "至少声明一个 100px、1fr 或 auto 轨道",
        ));
    }
    // 返回公开 GridTrack 向量。
    Ok(quote! { ::std::vec![#(#tracks),*] })
}

// 生成 u32 构建器链。
fn chain_u32(
    // 接收已有配置表达式。
    config: TokenStream,
    // 接收公开构建器方法名。
    method: &str,
    // 接收已验证值。
    value: u32,
) -> TokenStream {
    // 按封闭方法集合生成确定令牌。
    match method {
        // 基础跨度。
        "span" => quote! { (#config).span(#value) },
        // 基础偏移。
        "offset" => quote! { (#config).offset(#value) },
        // sm 跨度。
        "sm" => quote! { (#config).sm(#value) },
        // md 跨度。
        "md" => quote! { (#config).md(#value) },
        // lg 跨度。
        "lg" => quote! { (#config).lg(#value) },
        // xl 跨度。
        "xl" => quote! { (#config).xl(#value) },
        // xxl 跨度。
        "xxl" => quote! { (#config).xxl(#value) },
        // 调用点只传递封闭集合中的名称。
        _ => unreachable!("未知 Col 构建器方法"),
    }
}

// 解析编译期 u32 结构常量并检查范围。
fn constant_u32(
    // 接收结构属性。
    attribute: &Attribute,
    // 接收诊断名称。
    contract: &str,
    // 接收最小值。
    minimum: u32,
    // 接收最大值。
    maximum: u32,
) -> Result<u32, Diagnostic> {
    // 取得字面量或受限表达式原文。
    let source = constant_source(attribute, contract)?;
    // 只接受完整无符号整数。
    let value = source.parse::<u32>().map_err(|_| {
        // 返回结构常量类型诊断。
        Diagnostic::new(
            // 指向具体属性。
            attribute.span,
            // 说明编译期整数要求。
            format!("{contract} 必须是编译期无符号整数"),
            // 给出文档化写法。
            format!("使用 {contract}={{{minimum}}} 到 {{{maximum}}} 范围内的整数"),
        )
    })?;
    // 检查文档规定范围。
    if value < minimum || value > maximum {
        // 返回范围诊断。
        return Err(Diagnostic::new(
            // 指向越界属性。
            attribute.span,
            // 说明实际值和范围。
            format!("{contract}={value} 超出 {minimum}..={maximum} 范围"),
            // 给出修复动作。
            format!("改用 {minimum} 到 {maximum} 之间的整数"),
        ));
    }
    // 返回已验证结构常量。
    Ok(value)
}

// 解析编译期 i32 顺序常量。
fn constant_i32(attribute: &Attribute, contract: &str) -> Result<i32, Diagnostic> {
    // 取得字面量或受限表达式原文。
    let source = constant_source(attribute, contract)?;
    // 只接受完整有符号整数。
    source.parse::<i32>().map_err(|_| {
        // 返回结构常量类型诊断。
        Diagnostic::new(
            // 指向具体属性。
            attribute.span,
            // 说明编译期整数要求。
            format!("{contract} 必须是编译期 i32 整数"),
            // 给出文档化写法。
            "使用 order={0}、order={1} 或 order={-1} 等整数",
        )
    })
}

// 取得结构属性可静态验证的源码。
fn constant_source<'a>(
    // 接收结构属性。
    attribute: &'a Attribute,
    // 接收诊断名称。
    contract: &str,
) -> Result<&'a str, Diagnostic> {
    // 区分字面量、受限表达式与内联样式。
    match &attribute.value {
        // 双引号整数允许兼容文档属性形式。
        AttributeValue::Literal(value) => Ok(value.trim()),
        // 花括号表达式必须本身是可解析整数常量。
        AttributeValue::Expression(expression) => Ok(expression.source.trim()),
        // 内联样式不是结构属性值。
        AttributeValue::InlineStyle(_) => Err(Diagnostic::new(
            // 指向完整属性。
            attribute.span,
            // 说明结构属性类型错误。
            format!("{contract} 不能使用内联样式对象"),
            // 给出整数表达式修复。
            "使用花括号整数，例如 span={8}",
        )),
    }
}

// 查找元素上的具名属性。

// 返回任意直接子节点的来源跨度。
fn node_span(node: &Node) -> SourceSpan {
    // 按节点种类提取其完整跨度。
    match node {
        // 元素使用开始到结束标签跨度。
        Node::Element(element) => element.span,
        // 文本使用原始文本跨度。
        Node::Text(text) => text.span,
        // 插值使用花括号表达式跨度。
        Node::Interpolation(expression) => expression.span,
        // 成员块保留完整声明跨度。
        Node::WidgetMember(block) => block.span,
    }
}
