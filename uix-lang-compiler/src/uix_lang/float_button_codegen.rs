// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入共享公共属性生成器与可见节点判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 FloatButton 所需的语法树、值生成器与诊断。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, Expression, ExpressionKind,
    generate_expression, literal_string, string_value,
};

// 生成文档化 FloatButton 标签对应的公开 Rust View。
pub(crate) fn generate_float_button(element: &Element) -> Result<TokenStream, Diagnostic> {
    // FloatButton 是叶子组件，不能静默丢弃可见子节点。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶子形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 FloatButton 元素。
            element.span,
            // 陈述叶子组件边界。
            "<FloatButton> 不接受子节点",
            // 给出文档化自闭合写法。
            "使用 <FloatButton icon=\"name\" />",
        ));
    }
    // 图标是构造运行时组件所需的唯一必填属性。
    let icon_attribute = element
        // 遍历原始属性。
        .attributes
        // 查找图标属性。
        .iter()
        // 只匹配 icon。
        .find(|attribute| attribute.name == "icon")
        // 缺失时返回结构化诊断。
        .ok_or_else(|| {
            // 构造缺失图标诊断。
            Diagnostic::new(
                // 指向完整元素。
                element.span,
                // 陈述必填属性。
                "<FloatButton> 缺少必填属性 icon",
                // 给出最小合法示例。
                "使用 <FloatButton icon=\"message\" />",
            )
        })?;
    // 图标接受字符串字面量或受限字符串表达式。
    let icon = string_value(icon_attribute)?;
    // UIX 缺省位置显式映射到文档定义的右下角。
    let mut placement = quote! { ::uix_app::prelude::Placement::BottomRight };
    // 保存可选说明文字。
    let mut description = None;
    // 保存可选悬停提示。
    let mut tooltip = None;
    // 保存可选数字徽标。
    let mut badge_count = None;
    // 保存可选圆点徽标。
    let mut badge_dot = None;
    // 按源码顺序解析 FloatButton 专有属性。
    for attribute in &element.attributes {
        // 按属性名保存专有配置。
        match attribute.name.as_str() {
            // icon 已由必填入口消费。
            "icon" => {}
            // description 接受字符串或字符串表达式。
            "description" => description = Some(string_value(attribute)?),
            // tooltip 接受字符串或字符串表达式。
            "tooltip" => tooltip = Some(string_value(attribute)?),
            // position 必须是确定的四角关键字。
            "position" => placement = placement_value(attribute)?,
            // badge 必须是已验证对象字面量。
            "badge" => {
                // 解析 count 与 dot 字段。
                let (count, dot) = badge_values(attribute)?;
                // 保存可选数字字段。
                badge_count = count;
                // 保存可选圆点字段。
                badge_dot = dot;
            }
            // 公共属性由统一生成器处理。
            _ => {}
        }
    }
    // 从现有公开运行时组件开始构造。
    let mut widget = quote! {
        // UIX 始终显式设置文档默认 Placement。
        (::uix_app::prelude::FloatButton::new(#icon)).placement(#placement)
    };
    // 有说明文字时调用公开构建器。
    if let Some(value) = description {
        // 追加 description 配置。
        widget = quote! { (#widget).description(#value) };
    }
    // 有提示文字时调用公开构建器。
    if let Some(value) = tooltip {
        // 追加 tooltip 配置。
        widget = quote! { (#widget).tooltip(#value) };
    }
    // 有数字徽标时调用公开构建器。
    if let Some(value) = badge_count {
        // 追加 count 配置。
        widget = quote! { (#widget).badge(#value) };
    }
    // 有圆点徽标时调用公开构建器。
    if let Some(value) = badge_dot {
        // 追加 dot 配置。
        widget = quote! { (#widget).badge_dot(#value) };
    }
    // 把运行时 Widget 包装成公开叶 View。
    let base = quote! { ::uix_app::prelude::ViewNode::leaf(#widget) };
    // 继续复用统一样式、事件与未知属性诊断路径。
    apply_common_attributes(
        // 传入 FloatButton 基础 View。
        base,
        // 传入原始属性列表。
        &element.attributes,
        // 防止专有属性被公共映射重复处理。
        &["icon", "description", "position", "tooltip", "badge"],
    )
}

// 把 UIX 四角关键字映射为公开 Placement。
fn placement_value(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 位置必须是确定字符串字面量。
    let value = literal_string(attribute, "position")?;
    // 映射文档中的四种位置。
    match value.as_str() {
        // 映射左上角。
        "leftTop" => Ok(quote! { ::uix_app::prelude::Placement::TopLeft }),
        // 映射左下角。
        "leftBottom" => Ok(quote! { ::uix_app::prelude::Placement::BottomLeft }),
        // 映射右上角。
        "rightTop" => Ok(quote! { ::uix_app::prelude::Placement::TopRight }),
        // 映射右下角。
        "rightBottom" => Ok(quote! { ::uix_app::prelude::Placement::BottomRight }),
        // 未登记位置返回结构化诊断。
        _ => Err(Diagnostic::new(
            // 指向完整 position 属性。
            attribute.span,
            // 陈述未知位置。
            format!("FloatButton position={value:?} 不受支持"),
            // 给出完整合法集合。
            "使用 leftTop、leftBottom、rightTop 或 rightBottom",
        )),
    }
}

// 生成 badge 对象中的可选 count 与 dot 字段。
fn badge_values(
    attribute: &Attribute,
) -> Result<(Option<TokenStream>, Option<TokenStream>), Diagnostic> {
    // badge 必须使用表达式属性值。
    let AttributeValue::Expression(node) = &attribute.value else {
        // 返回对象写法诊断。
        return Err(badge_object_diagnostic(attribute));
    };
    // 表达式必须是受限对象字面量。
    let ExpressionKind::Object(fields) = &node.expression.kind else {
        // 返回对象写法诊断。
        return Err(badge_object_diagnostic(attribute));
    };
    // 保存可选数字徽标。
    let mut count = None;
    // 保存可选圆点徽标。
    let mut dot = None;
    // 按 AST 源码顺序消费字段。
    for field in fields {
        // 按字段名执行类型边界。
        match field.name.as_str() {
            // count 接受非负整数表达式。
            "count" => {
                // 拒绝明显不是数字的标量字面量。
                validate_count(&field.value)?;
                // 生成字段值 Rust 表达式。
                count = Some(generate_expression(&field.value, None)?);
            }
            // dot 接受布尔表达式。
            "dot" => {
                // 拒绝明显不是布尔的标量字面量。
                validate_dot(&field.value)?;
                // 生成字段值 Rust 表达式。
                dot = Some(generate_expression(&field.value, None)?);
            }
            // 未知字段不能被静默忽略。
            _ => {
                // 返回未知 badge 字段诊断。
                return Err(Diagnostic::new(
                    // 指向完整字段。
                    field.span,
                    // 陈述未知字段名。
                    format!("FloatButton badge 不支持字段 {}", field.name),
                    // 给出合法字段集合。
                    "只使用 count 和 dot 字段",
                ));
            }
        }
    }
    // 返回两个可选构建器参数。
    Ok((count, dot))
}

// 验证 count 的明显字面量类型与非负边界。
fn validate_count(expression: &Expression) -> Result<(), Diagnostic> {
    // 按可静态判断的字面量形状验证。
    match &expression.kind {
        // 整数字面量属于合法 count。
        ExpressionKind::Number(value) if !value.contains('.') => Ok(()),
        // 小数字面量不能进入整数徽标。
        ExpressionKind::Number(_) => Err(Diagnostic::new(
            // 指向数字值。
            expression.span,
            // 陈述整数要求。
            "FloatButton badge.count 必须是整数",
            // 给出合法示例。
            "使用 count: 7 或整数受限表达式",
        )),
        // 负数字面量违反非负边界。
        ExpressionKind::Unary { operator, operand }
            if matches!(operator, super::UnaryOperator::Negate)
                && matches!(operand.kind, ExpressionKind::Number(_)) =>
        {
            // 返回非负约束诊断。
            Err(Diagnostic::new(
                // 指向完整负数。
                expression.span,
                // 陈述非负要求。
                "FloatButton badge.count 不能为负数",
                // 给出合法边界。
                "使用零或正整数",
            ))
        }
        // 字符串、布尔和对象是确定的错误类型。
        ExpressionKind::String(_) | ExpressionKind::Boolean(_) | ExpressionKind::Object(_) => {
            // 返回整数类型诊断。
            Err(Diagnostic::new(
                // 指向字段值。
                expression.span,
                // 陈述 count 类型。
                "FloatButton badge.count 必须是整数表达式",
                // 给出合法示例。
                "使用 count: 7 或整数受限表达式",
            ))
        }
        // 其他受限表达式交给 Rust 类型系统验证为 i32。
        _ => Ok(()),
    }
}

// 验证 dot 的明显字面量类型。
fn validate_dot(expression: &Expression) -> Result<(), Diagnostic> {
    // 布尔字面量直接合法。
    if matches!(expression.kind, ExpressionKind::Boolean(_)) {
        // 返回验证成功。
        return Ok(());
    }
    // 数字、字符串和对象是确定的错误类型。
    if matches!(
        expression.kind,
        ExpressionKind::Number(_) | ExpressionKind::String(_) | ExpressionKind::Object(_)
    ) {
        // 返回布尔类型诊断。
        return Err(Diagnostic::new(
            // 指向字段值。
            expression.span,
            // 陈述 dot 类型。
            "FloatButton badge.dot 必须是布尔表达式",
            // 给出合法示例。
            "使用 dot: true 或布尔受限表达式",
        ));
    }
    // 其他受限表达式交给 Rust 类型系统验证为 bool。
    Ok(())
}

// 构造 badge 非对象属性的统一诊断。
fn badge_object_diagnostic(attribute: &Attribute) -> Diagnostic {
    // 返回双花括号对象写法。
    Diagnostic::new(
        // 指向完整 badge 属性。
        attribute.span,
        // 陈述对象结构要求。
        "FloatButton badge 必须使用对象字面量",
        // 给出文档化示例。
        "使用 badge={{ count: 7, dot: false }}",
    )
}
