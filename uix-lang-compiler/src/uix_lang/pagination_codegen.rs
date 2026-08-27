// 引入卫生事件变量所需的标识符与跨度。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::{Ident, Span, TokenStream};
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 Pagination 属性、表达式、事件与诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, boolean_value,
    generate_event_handler_expression, generate_expression,
};

// 生成 total、双 State<usize> 与 Change 事件绑定的分页器叶节点。
pub(crate) fn generate_pagination(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Pagination 自身绘制完整控件，不接受 UIX 子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Pagination 元素。
            element.span,
            // 说明分页器不接受子节点。
            "<Pagination> 不接受子节点",
            // 给出规范自闭合写法。
            "使用 <Pagination current={page} pageSize={page_size} total={total} />",
        ));
    }

    // total 缺省为文档登记的零。
    let total = optional_usize(element, "total", 0)?;
    // 构造器的非受控 pageSize 缺省为文档登记的十。
    let mut widget = quote! { ::uix::prelude::Pagination::new(#total, 10_usize) };

    // pageSizeOptions 只适配拥有 usize 元素的声明集合。
    if let Some(attribute) = find_attribute(element, "pageSizeOptions") {
        // 解析静态整数列表或受限 Rust 可迭代表达式。
        let options = page_size_options_value(attribute)?;
        // 收集为公开运行时要求的 Vec<usize>。
        widget = quote! {
            (#widget).page_size_options(
                (#options).into_iter().collect::<::std::vec::Vec<usize>>()
            )
        };
    }
    // pageSize 出现时必须保留 State<usize> 所有权句柄。
    if let Some(attribute) = find_attribute(element, "pageSize") {
        // 解析声明端 pageSize 状态表达式。
        let state = state_expression(attribute, "pageSize")?;
        // 先绑定 pageSize，后续 current 才能按正确总页数归一化。
        widget = quote! { (#widget).page_size_state(&(#state)) };
        // 文档承诺每页条数切换，绑定存在时显式启用交互入口。
        widget = quote! { (#widget).show_size_changer(true) };
    }
    // showTotal 显式覆盖运行时默认的总数文案策略。
    if let Some(attribute) = find_attribute(element, "showTotal") {
        // 解析布尔简写、字面量或表达式。
        let show_total = boolean_value(attribute)?;
        // 调用公开总数显示入口。
        widget = quote! { (#widget).show_total(#show_total) };
    }
    // 显式 showSizeChanger 必须覆盖 pageSize 绑定带来的默认启用。
    if let Some(attribute) = find_attribute(element, "showSizeChanger") {
        // 解析布尔简写、字面量或表达式。
        let show_size_changer = boolean_value(attribute)?;
        // 在隐式默认之后应用最终声明策略。
        widget = quote! { (#widget).show_size_changer(#show_size_changer) };
    }
    // simple 声明分页主体的紧凑绘制模式。
    if let Some(attribute) = find_attribute(element, "simple") {
        // 解析布尔简写、字面量或表达式。
        let simple = boolean_value(attribute)?;
        // 调用公开紧凑模式入口。
        widget = quote! { (#widget).simple(#simple) };
    }
    // showJumper 声明页码跳转输入入口。
    if let Some(attribute) = find_attribute(element, "showJumper") {
        // 解析布尔简写、字面量或表达式。
        let show_jumper = boolean_value(attribute)?;
        // 调用公开跳转入口。
        widget = quote! { (#widget).show_jumper(#show_jumper) };
    }
    // current 出现时必须保留 State<usize> 所有权句柄。
    if let Some(attribute) = find_attribute(element, "current") {
        // 解析声明端 current 状态表达式。
        let state = state_expression(attribute, "current")?;
        // 在 pageSize 完成同步后绑定页码。
        widget = quote! { (#widget).current_state(&(#state)) };
    }

    // 先物化公开叶节点，Change 处理器与样式由 View 契约拥有。
    let mut view = quote! { ::uix::prelude::ViewNode::leaf(#widget) };
    // 可选变化事件读取既有页码或 page_size=<值> 文本载荷。
    if let Some(attribute) = find_attribute(element, "@change") {
        // 事件解析器应始终提供受限表达式。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回内部形状保护诊断。
            return Err(Diagnostic::new(
                // 指向完整事件属性。
                attribute.span,
                // 说明事件处理器形状。
                "Pagination @change 必须是受限处理器表达式",
                // 给出带载荷的规范写法。
                "使用 @change=\"on_page_change($event)\"",
            ));
        };
        // 创建卫生的文本载荷变量。
        let value = Ident::new("__uix_pagination_change", Span::mixed_site());
        // 生成裸处理器或显式载荷调用。
        let handler = generate_event_handler_expression(&expression.expression, &value, "@change")?;
        // 使用公开 View Change 注册入口保存处理器。
        view = quote! {
            // 注册只接收现有 Change 文本借用的闭包。
            (#view).on_change_fn(move |#value| {
                // 丢弃处理器返回值并保留副作用。
                let _ = { #handler };
            })
        };
    }

    // 消费 Pagination 专有属性后应用统一尺寸、样式与其他公共事件。
    apply_common_attributes(
        // 传入已经配置双状态和事件的公开 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性与 Change 事件被二次映射。
        &[
            "total",
            "current",
            "pageSize",
            "pageSizeOptions",
            "showTotal",
            "showSizeChanger",
            "simple",
            "showJumper",
            "@change",
        ],
    )
}

// 生成 pageSizeOptions 的静态整数列表或拥有型 usize 可迭表达式。
fn page_size_options_value(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 按声明值形状生成拥有型集合。
    match &attribute.value {
        // 静态列表使用逗号分隔的十进制 usize。
        AttributeValue::Literal(source) => {
            // 为每个经过验证的选项保留 usize 值。
            let mut options = Vec::new();
            // 按逗号逐项解析静态列表。
            for item in source.split(',') {
                // 忽略逗号两侧的可读性空白。
                let item = item.trim();
                // 空项不能表达有效尺寸选项。
                if item.is_empty() {
                    // 返回精确列表形状诊断。
                    return Err(page_size_options_diagnostic(attribute));
                }
                // 拒绝负数、小数、后缀与溢出值。
                let value = item
                    // 解析为运行时公开 API 要求的 usize。
                    .parse::<usize>()
                    // 将解析失败统一映射为属性诊断。
                    .map_err(|_| page_size_options_diagnostic(attribute))?;
                // 保存已验证静态选项。
                options.push(value);
            }
            // 生成拥有型 Vec，后续统一走 IntoIterator 收集路径。
            Ok(quote! { ::std::vec![#(#options),*] })
        }
        // 动态表达式由 Rust 核对 IntoIterator<Item = usize>。
        AttributeValue::Expression(expression) => {
            // 生成受限集合表达式。
            generate_expression(&expression.expression, None)
        }
        // 内联样式不能携带类型化集合。
        AttributeValue::InlineStyle(_) => {
            // 返回精确列表形状诊断。
            Err(page_size_options_diagnostic(attribute))
        }
    }
}

// 构造 pageSizeOptions 的统一值形状诊断。
fn page_size_options_diagnostic(attribute: &Attribute) -> Diagnostic {
    // 返回带静态和动态修复建议的诊断。
    Diagnostic::new(
        // 指向非法 pageSizeOptions 属性。
        attribute.span,
        // 说明元素和容器边界。
        "Pagination pageSizeOptions 必须是逗号分隔的 usize 列表或可迭表达式",
        // 给出静态列表或 Rust 绑定写法。
        "使用 pageSizeOptions=\"10,20\" 或 pageSizeOptions={options}",
    )
}

// 生成可选 usize 属性或显式默认值。
fn optional_usize(
    // 接收完整元素。
    element: &Element,
    // 接收属性名称。
    name: &str,
    // 接收缺省整数。
    default: usize,
) -> Result<TokenStream, Diagnostic> {
    // 缺失属性直接生成带类型的默认字面量。
    let Some(attribute) = find_attribute(element, name) else {
        // 返回稳定 usize 令牌。
        return Ok(quote! { #default });
    };
    // 字面量在生成期验证为 usize。
    match &attribute.value {
        // 解析十进制整数字面量。
        AttributeValue::Literal(source) => {
            // 拒绝负数、小数与溢出值。
            let value = source.parse::<usize>().map_err(|_| {
                // 返回精确属性诊断。
                Diagnostic::new(
                    // 指向非法整数属性。
                    attribute.span,
                    // 说明 total 的运行时类型。
                    format!("Pagination {name} 必须是 usize 整数"),
                    // 给出规范字面量或表达式写法。
                    format!("使用 {name}=\"100\" 或 {name}={{total}}"),
                )
            })?;
            // 返回已验证整数字面量。
            Ok(quote! { #value })
        }
        // 动态表达式交给 Rust 消费端核对 usize 类型。
        AttributeValue::Expression(expression) => {
            // 生成受限 Rust 表达式。
            generate_expression(&expression.expression, None)
        }
        // 结构化内联样式不能表达总条数。
        AttributeValue::InlineStyle(_) => Err(Diagnostic::new(
            // 指向非法内联样式属性。
            attribute.span,
            // 说明整数类型要求。
            format!("Pagination {name} 必须是 usize 整数"),
            // 给出规范值形状。
            format!("使用 {name}=\"100\" 或 {name}={{total}}"),
        )),
    }
}

// 生成 current 或 pageSize 的 State<usize> 表达式。
fn state_expression(attribute: &Attribute, name: &str) -> Result<TokenStream, Diagnostic> {
    // 字面量和内联样式不能提供双向状态所有权。
    let AttributeValue::Expression(expression) = &attribute.value else {
        // 返回绑定形状诊断。
        return Err(Diagnostic::new(
            // 指向非法状态属性。
            attribute.span,
            // 说明公开运行时绑定类型。
            format!("Pagination {name} 必须绑定 State<usize> 表达式"),
            // 给出规范绑定写法。
            format!(
                "使用 {name}={{{}}}",
                if name == "current" {
                    "page"
                } else {
                    "page_size"
                }
            ),
        ));
    };
    // 生成受限状态表达式。
    generate_expression(&expression.expression, None)
}

// 查找元素上的具名属性。
