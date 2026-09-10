// 引入过程宏字面量与令牌流。
use proc_macro2::{Literal, TokenStream};
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入共享公共属性生成器与文本内容生成器。
use super::codegen::{apply_common_attributes, generate_text_content};
// 引入 Typography 所需的语法树、属性值生成器与诊断。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, boolean_value, generate_expression,
    literal_string,
};

// 生成文档化 Typography 标签对应的公开 Rust View。
pub(crate) fn generate_typography(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 生成只包含文本与插值的排版内容。
    let content = generate_text_content(&element.children, element.span)?;
    // 从现有公开 Typography 正文构造器开始构建。
    let mut typography = quote! { ::uix_app::prelude::Typography::text(&(#content)) };
    // 按源码顺序应用 Typography 专有属性。
    for attribute in &element.attributes {
        // 按属性名选择公开构建器。
        match attribute.name.as_str() {
            // 标题层级映射到现有 level 构建器。
            "level" => {
                // 生成已验证的 u8 字面量或受限表达式。
                let level = level_value(attribute)?;
                // 应用标题层级。
                typography = quote! { (#typography).level(#level) };
            }
            // 语义类型映射到主题感知颜色值。
            "type" => {
                // 生成已登记的主题颜色值。
                let color = semantic_color_value(attribute)?;
                // 由 Typography 在绘制阶段解析当前 Provider 主题。
                typography = quote! { (#typography).semantic_color(#color) };
            }
            // 标记高亮支持布尔字面量与受限表达式。
            "mark" => {
                // 生成确定的布尔表达式。
                let enabled = boolean_value(attribute)?;
                // 仅在值为真时调用现有启用式构建器。
                typography = quote! {
                    // 保持 Typography 类型在两个分支中一致。
                    if #enabled { (#typography).mark() } else { #typography }
                };
            }
            // 下划线支持布尔字面量与受限表达式。
            "underline" => {
                // 生成确定的布尔表达式。
                let enabled = boolean_value(attribute)?;
                // 仅在值为真时调用现有启用式构建器。
                typography = quote! {
                    // 保持 Typography 类型在两个分支中一致。
                    if #enabled { (#typography).underline() } else { #typography }
                };
            }
            // 复制按钮状态直接映射完整布尔值。
            "copyable" => {
                // 生成确定的布尔表达式。
                let copyable = boolean_value(attribute)?;
                // 应用现有复制能力构建器。
                typography = quote! { (#typography).copyable(#copyable) };
            }
            // 公共属性由统一生成器处理。
            _ => {}
        }
    }
    // Typography 是不持有元素子树的叶组件。
    let base = quote! { ::uix_app::prelude::ViewNode::leaf(#typography) };
    // 专有属性消费后继续复用统一样式与事件诊断路径。
    apply_common_attributes(
        // 传入 Typography 基础 View。
        base,
        // 传入原始属性列表。
        &element.attributes,
        // 防止专有属性被公共映射重复处理。
        &["level", "type", "mark", "underline", "copyable"],
    )
}

// 生成标题层级的 u8 令牌并在字面量阶段执行范围诊断。
fn level_value(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 按属性值形状转换。
    match &attribute.value {
        // 字面量在宏期解析并验证一到五级。
        AttributeValue::Literal(value) => {
            // 将十进制字符串解析为 u8。
            let level = value.parse::<u8>().map_err(|_| {
                // 返回明确整数格式诊断。
                Diagnostic::new(
                    // 指向完整 level 属性。
                    attribute.span,
                    // 说明非法层级字面量。
                    format!("Typography level={value:?} 不是 1~5 的整数"),
                    // 给出文档允许的范围。
                    "使用 1、2、3、4、5 或 u8 类型的受限表达式",
                )
            })?;
            // 拒绝运行时构建器会夹紧的越界字面量。
            if !(1..=5).contains(&level) {
                // 返回精确范围诊断。
                return Err(Diagnostic::new(
                    // 指向完整 level 属性。
                    attribute.span,
                    // 说明具体越界值。
                    format!("Typography level={level} 超出 1~5 范围"),
                    // 给出合法层级集合。
                    "使用 1、2、3、4 或 5",
                ));
            }
            // 生成可直接传给公开 level(u8) 的无后缀字面量。
            let literal = Literal::u8_unsuffixed(level);
            // 返回标题层级令牌。
            Ok(quote! { #literal })
        }
        // 表达式由 Rust 类型系统保证 u8 类型。
        AttributeValue::Expression(expression) => {
            // 生成受限表达式令牌。
            generate_expression(&expression.expression, None)
        }
        // 内联样式不属于标题层级属性。
        AttributeValue::InlineStyle(_) => Err(Diagnostic::new(
            // 指向完整 level 属性。
            attribute.span,
            // 说明值形状错误。
            "Typography level 不能使用样式值",
            // 给出正确值形状。
            "使用 1~5 字面量或 u8 类型的受限表达式",
        )),
    }
}

// 生成 Typography type 对应的主题颜色值。
fn semantic_color_value(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 颜色角色影响公开枚举选择，因此要求编译期字符串字面量。
    let value = literal_string(attribute, "Typography type")?;
    // 映射文档登记的语义角色。
    match value.as_str() {
        // 次要文字使用中性文字令牌。
        "secondary" => Ok(quote! {
            // 构造主题感知的次要文字颜色。
            ::uix_app::prelude::ColorValue::neutral(::uix_app::prelude::NeutralRole::TextSecondary)
        }),
        // 成功文字使用功能成功色。
        "success" => Ok(quote! {
            // 构造主题感知的成功颜色。
            ::uix_app::prelude::ColorValue::palette(::uix_app::prelude::PaletteColor::Success)
        }),
        // 警告文字使用功能警告色。
        "warning" => Ok(quote! {
            // 构造主题感知的警告颜色。
            ::uix_app::prelude::ColorValue::palette(::uix_app::prelude::PaletteColor::Warning)
        }),
        // 危险文字使用功能错误色。
        "danger" => Ok(quote! {
            // 构造主题感知的错误颜色。
            ::uix_app::prelude::ColorValue::palette(::uix_app::prelude::PaletteColor::Error)
        }),
        // 其他角色不能静默近似。
        _ => Err(Diagnostic::new(
            // 指向非法 type 属性。
            attribute.span,
            // 说明具体非法值。
            format!("Typography type={value:?} 不受支持"),
            // 给出已登记角色集合。
            "使用 secondary、success、warning 或 danger",
        )),
    }
}
