// 引入数值字面量与令牌流。
use proc_macro2::{Ident, Literal, TokenStream};
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入属性语法树、结构化诊断与源码跨度。
use super::{Attribute, AttributeValue, Diagnostic, Element, SourceSpan};
// 引入受限表达式生成入口与共享元素属性查找。
use super::find_attribute;
use super::generate_expression;

// 生成字符串或表达式属性值。
pub(crate) fn string_value(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 按属性值形状转换。
    match &attribute.value {
        // 字面量直接生成 Rust 字符串。
        AttributeValue::Literal(value) => Ok(quote! { #value }),
        // 表达式递归生成 Rust 代码。
        AttributeValue::Expression(expression) => {
            // 返回表达式令牌。
            generate_expression(&expression.expression, None)
        }
        // 内联样式不能作为普通字符串传递。
        AttributeValue::InlineStyle(_) => Err(deferred_style_diagnostic(attribute, "style")),
    }
}

// 要求属性为编译期字符串字面量。
pub(crate) fn literal_string(
    // 接收待验证属性。
    attribute: &Attribute,
    // 接收诊断主题。
    subject: &str,
) -> Result<String, Diagnostic> {
    // 只接受双引号字面量。
    if let AttributeValue::Literal(value) = &attribute.value {
        // 返回拥有所有权的字面量。
        return Ok(value.clone());
    }
    // 返回确定性要求诊断。
    Err(Diagnostic::new(
        // 指向完整属性。
        attribute.span,
        // 说明该映射需要编译期选择。
        format!("{subject} 必须使用字符串字面量"),
        // 给出双引号修复建议。
        "使用双引号包裹已登记值",
    ))
}

// 生成布尔字面量或表达式属性。
pub(crate) fn boolean_value(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 按属性值形状转换。
    match &attribute.value {
        // 字面量只接受 true 或 false。
        AttributeValue::Literal(value) => match value.as_str() {
            // 生成 true。
            "true" => Ok(quote! { true }),
            // 生成 false。
            "false" => Ok(quote! { false }),
            // 其他字面量返回诊断。
            _ => Err(Diagnostic::new(
                // 指向完整属性。
                attribute.span,
                // 说明非法布尔值。
                format!("属性 {} 需要布尔值", attribute.name),
                // 给出合法值或表达式。
                "使用 \"true\"、\"false\" 或 {boolean_expression}",
            )),
        },
        // 表达式由 Rust 类型检查保证布尔类型。
        AttributeValue::Expression(expression) => {
            // 返回表达式令牌。
            generate_expression(&expression.expression, None)
        }
        // 内联样式不属于布尔属性。
        AttributeValue::InlineStyle(_) => Err(Diagnostic::new(
            // 指向完整属性。
            attribute.span,
            // 说明值形状错误。
            format!("属性 {} 不能使用样式值", attribute.name),
            // 给出布尔值修复建议。
            "改用布尔字面量或表达式",
        )),
    }
}

// 生成可选布尔属性；未声明时缺省为 false。
pub(crate) fn optional_boolean(
    // 接收完整元素。
    element: &Element,
    // 接收布尔属性名。
    name: &str,
) -> Result<TokenStream, Diagnostic> {
    // 显式属性复用统一布尔诊断与动态表达式生成。
    find_attribute(element, name)
        // 把可选属性转换为可选生成结果。
        .map(boolean_value)
        // 把 Option<Result> 转换为 Result<Option>。
        .transpose()
        // 缺省时生成类型明确的布尔字面量。
        .map(|value| value.unwrap_or_else(|| quote! { false }))
}

// 生成像素字面量或数值表达式。
pub(crate) fn numeric_value(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 按属性值形状转换。
    match &attribute.value {
        // 字面量解析可选 px 后缀。
        AttributeValue::Literal(value) => numeric_literal(value, attribute.span),
        // 表达式由 Rust 类型检查数值类型。
        AttributeValue::Expression(expression) => {
            // 返回表达式令牌。
            generate_expression(&expression.expression, None)
        }
        // 内联样式不属于数值属性。
        AttributeValue::InlineStyle(_) => Err(Diagnostic::new(
            // 指向完整属性。
            attribute.span,
            // 说明值形状错误。
            format!("属性 {} 不能使用样式值", attribute.name),
            // 给出数值修复建议。
            "使用如 \"16px\" 的字面量或数值表达式",
        )),
    }
}

// 把十进制或 px 字面量转换为 f32 令牌。
fn numeric_literal(value: &str, span: SourceSpan) -> Result<TokenStream, Diagnostic> {
    // 去除可选 px 单位。
    let number = value.strip_suffix("px").unwrap_or(value);
    // 解析为有限 f32。
    let parsed = number.parse::<f32>().map_err(|_| {
        // 构造数值字面量诊断。
        Diagnostic::new(
            // 指向完整属性。
            span,
            // 说明数值格式错误。
            format!("数值字面量 {value:?} 无法映射为 f32"),
            // 给出规范格式。
            "使用十进制数或 px 值，例如 \"16px\"",
        )
    })?;
    // 拒绝无穷或非数值结果。
    if !parsed.is_finite() {
        // 返回有限数值要求诊断。
        return Err(Diagnostic::new(
            // 指向完整属性。
            span,
            // 说明非有限值不受支持。
            format!("数值字面量 {value:?} 不是有限值"),
            // 给出有限值修复建议。
            "使用有限十进制数",
        ));
    }
    // 生成无后缀 f32 字面量以匹配公开 API。
    let literal = Literal::f32_unsuffixed(parsed);
    // 返回数值令牌。
    Ok(quote! { #literal })
}

// 生成字号令牌或自定义像素值。
pub(crate) fn typography_value(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 表达式直接交给 Rust 类型系统。
    if let AttributeValue::Expression(expression) = &attribute.value {
        // 返回表达式令牌。
        return generate_expression(&expression.expression, None);
    }
    // 字号令牌必须是普通字面量。
    let value = literal_string(attribute, "fontSize")?;
    // 映射文档中的公开 TypographyToken 名称。
    match value.as_str() {
        // 映射小号正文。
        "small" => Ok(quote! { ::uix::prelude::TypographyToken::Small }),
        // 映射正文。
        "body" => Ok(quote! { ::uix::prelude::TypographyToken::Body }),
        // 映射大号正文。
        "large" => Ok(quote! { ::uix::prelude::TypographyToken::Large }),
        // 映射超大正文。
        "xLarge" => Ok(quote! { ::uix::prelude::TypographyToken::XLarge }),
        // 映射一级标题。
        "heading1" => Ok(quote! { ::uix::prelude::TypographyToken::Heading1 }),
        // 映射二级标题。
        "heading2" => Ok(quote! { ::uix::prelude::TypographyToken::Heading2 }),
        // 映射三级标题。
        "heading3" => Ok(quote! { ::uix::prelude::TypographyToken::Heading3 }),
        // 映射四级标题。
        "heading4" => Ok(quote! { ::uix::prelude::TypographyToken::Heading4 }),
        // 映射五级标题。
        "heading5" => Ok(quote! { ::uix::prelude::TypographyToken::Heading5 }),
        // 其他字面量尝试解析为像素数值。
        _ => numeric_literal(&value, attribute.span),
    }
}

// 生成交叉轴对齐枚举。
pub(crate) fn align_value(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 对齐必须是确定字面量。
    let value = literal_string(attribute, "align")?;
    // 映射公开 AlignItems 变体；kebab 为文档登记值面（布局组件.md），
    // start/end 为旧版兼容驼峰别名（保留以不破坏既有 .uix 源码）。
    match value.as_str() {
        // 映射起始对齐。
        "flex-start" | "start" => Ok(quote! { ::uix::prelude::AlignItems::Start }),
        // 映射居中对齐。
        "center" => Ok(quote! { ::uix::prelude::AlignItems::Center }),
        // 映射末端对齐。
        "flex-end" | "end" => Ok(quote! { ::uix::prelude::AlignItems::End }),
        // 映射拉伸对齐。
        "stretch" => Ok(quote! { ::uix::prelude::AlignItems::Stretch }),
        // 未登记值返回诊断。
        _ => Err(Diagnostic::new(
            // 指向完整属性。
            attribute.span,
            // 说明未知对齐值。
            format!("align={value:?} 不受支持"),
            // 给出合法集合（kebab 为文档登记值面）。
            "使用 flex-start、center、flex-end 或 stretch",
        )),
    }
}

// 生成主轴对齐枚举。
pub(crate) fn justify_value(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 对齐必须是确定字面量。
    let value = literal_string(attribute, "justify")?;
    // 映射公开 JustifyContent 变体；kebab 为文档登记值面（布局组件.md），
    // 驼峰 start/end/spaceBetween 等为旧版兼容别名（保留以不破坏既有 .uix 源码）。
    match value.as_str() {
        // 映射起始对齐。
        "flex-start" | "start" => Ok(quote! { ::uix::prelude::JustifyContent::Start }),
        // 映射居中对齐。
        "center" => Ok(quote! { ::uix::prelude::JustifyContent::Center }),
        // 映射末端对齐。
        "flex-end" | "end" => Ok(quote! { ::uix::prelude::JustifyContent::End }),
        // 映射两端分布。
        "space-between" | "spaceBetween" => {
            // 兼容旧驼峰别名，文档登记值为 kebab。
            Ok(quote! { ::uix::prelude::JustifyContent::SpaceBetween })
        }
        // 映射环绕分布。
        "space-around" | "spaceAround" => {
            // 兼容旧驼峰别名，文档登记值为 kebab。
            Ok(quote! { ::uix::prelude::JustifyContent::SpaceAround })
        }
        // 映射均匀分布。
        "space-evenly" | "spaceEvenly" => {
            // 兼容旧驼峰别名，文档登记值为 kebab。
            Ok(quote! { ::uix::prelude::JustifyContent::SpaceEvenly })
        }
        // 映射拉伸。
        "stretch" => Ok(quote! { ::uix::prelude::JustifyContent::Stretch }),
        // 未登记值返回诊断。
        _ => Err(Diagnostic::new(
            // 指向完整属性。
            attribute.span,
            // 说明未知对齐值。
            format!("justify={value:?} 不受支持"),
            // 给出合法集合（kebab 为文档登记值面）。
            "使用 flex-start、center、flex-end、space-between、space-around、space-evenly 或对应驼峰兼容别名",
        )),
    }
}

// 构造样式映射阶段边界诊断。
pub(crate) fn deferred_style_diagnostic(attribute: &Attribute, name: &str) -> Diagnostic {
    // 返回不静默忽略样式的明确诊断。
    Diagnostic::new(
        // 指向完整样式属性。
        attribute.span,
        // 说明样式尚未进入当前 Gate。
        format!("{name} 样式映射尚未在核心 View Gate 中登记"),
        // 指向完整样式矩阵阶段。
        "由样式与内置组件映射矩阵 Gate 生成该属性",
    )
}

// 把语言绑定名验证并转换为 Rust 标识符。
pub(crate) fn rust_identifier(name: &str, span: SourceSpan) -> Result<Ident, Diagnostic> {
    // 使用 syn 校验 Rust 标识符规则。
    syn::parse_str::<Ident>(name).map_err(|_| {
        // 构造非法绑定诊断。
        Diagnostic::new(
            // 指向绑定声明。
            span,
            // 说明绑定无法进入 Rust 代码。
            format!("绑定名 {name} 不是合法 Rust 标识符"),
            // 给出改名建议。
            "改用非 Rust 关键字的 ASCII 标识符",
        )
    })
}
