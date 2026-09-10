// 引入卫生事件变量所需的标识符与跨度。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::{Ident, Span, TokenStream};
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 Upload 属性、表达式、事件、布尔映射与诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, boolean_value,
    generate_event_handler_expression, generate_expression,
};

// 生成绑定调用方上传队列并发布类型化事实的 Upload 叶组件。
pub(crate) fn generate_upload(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Upload 自身绘制拖放入口与完整文件列表，不接受 UIX 子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Upload 元素。
            element.span,
            // 说明上传组件不接受子节点。
            "<Upload> 不接受子节点",
            // 给出规范自闭合写法。
            "使用 <Upload files={files} />",
        ));
    }

    // action 属于应用传输服务，首版组件必须显式拒绝。
    if let Some(attribute) = find_attribute(element, "action") {
        // 返回所有权边界诊断。
        return Err(Diagnostic::new(
            // 精确指向 action 属性。
            attribute.span,
            // 说明首版没有网络传输所有权。
            "Upload action 不属于首版 UIX 契约",
            // 给出应用服务按稳定 id 消费队列的修复方向。
            "由应用服务观察 UploadChange 并按 UploadFileId 管理传输、重试与取消",
        ));
    }

    // files 是调用方拥有的必需唯一队列真值。
    let files_attribute = required_attribute(element, "files")?;
    // 字面量不能提供可写回和可订阅的状态句柄。
    let AttributeValue::Expression(files_expression) = &files_attribute.value else {
        // 返回受控绑定形状诊断。
        return Err(Diagnostic::new(
            // 指向非法 files 属性。
            files_attribute.span,
            // 明确公开运行时状态类型。
            "Upload files 必须绑定 State<Vec<UploadFile>> 表达式",
            // 给出规范状态引用写法。
            "使用 files={upload_files}",
        ));
    };
    // 生成受限状态表达式并由 Rust 核对最终类型。
    let files = generate_expression(&files_expression.expression, None)?;
    // 从公开构造器和唯一队列绑定开始配置。
    let mut widget = quote! { ::uix_app::prelude::Upload::new().files(&(#files)) };

    // accept 在 UIX 首版只接受可于宏展开期验证的字符串字面量。
    if let Some(attribute) = find_attribute(element, "accept") {
        // 表达式不能保证编译期拒绝 MIME 模式。
        let AttributeValue::Literal(pattern) = &attribute.value else {
            // 返回确定性语法诊断。
            return Err(Diagnostic::new(
                // 指向动态或样式属性。
                attribute.span,
                // 说明首版 UIX 的字面量要求。
                "Upload accept 必须使用字符串字面量",
                // 指引 Rust 动态配置使用类型化错误入口。
                "UIX 使用 accept=\".png,.jpg\"；动态配置改用 Rust Upload::accept 并处理 UploadAcceptError",
            ));
        };
        // MIME 与未登记模式必须在生成期失败。
        validate_accept_literal(pattern, attribute)?;
        // 静态验证后调用返回 Result 的公开运行时入口。
        widget = quote! {
            (#widget)
                .accept(#pattern)
                .expect("UIX 已在宏展开期验证 Upload accept")
        };
    }

    // multiple 接受布尔简写、字面量或表达式。
    if let Some(attribute) = find_attribute(element, "multiple") {
        // 生成统一布尔属性令牌。
        let multiple = boolean_value(attribute)?;
        // 单批多选限制交给公开运行时。
        widget = quote! { (#widget).multiple(#multiple) };
    }
    // drag 接受布尔简写、字面量或表达式。
    if let Some(attribute) = find_attribute(element, "drag") {
        // 生成统一布尔属性令牌。
        let drag = boolean_value(attribute)?;
        // 拖放入口配置交给公开运行时。
        widget = quote! { (#widget).drag(#drag) };
    }
    // maxCount 只限制后续用户新增。
    if let Some(attribute) = find_attribute(element, "maxCount") {
        // 生成 usize 字面量或表达式。
        let max_count = integer_value(attribute, "usize")?;
        // 外部状态不会被该配置截断。
        widget = quote! { (#widget).max_count(#max_count) };
    }
    // maxSize 表达单文件字节上限。
    if let Some(attribute) = find_attribute(element, "maxSize") {
        // 生成 u64 字面量或表达式。
        let max_size = integer_value(attribute, "u64")?;
        // 大小门禁交给公开运行时。
        widget = quote! { (#widget).max_size(#max_size) };
    }

    // Change 处理器直接接收不可变 UploadChange 借用。
    if let Some(attribute) = find_attribute(element, "@change") {
        // 事件解析器应始终提供受限表达式。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回内部形状保护诊断。
            return Err(Diagnostic::new(
                // 指向完整事件属性。
                attribute.span,
                // 说明事件处理器形状。
                "Upload @change 必须是受限处理器表达式",
                // 给出类型化事实载荷的规范写法。
                "使用 @change=\"on_upload_change($event)\"",
            ));
        };
        // 创建卫生的类型化变化事实变量。
        let change = Ident::new("__uix_upload_change", Span::mixed_site());
        // 生成裸处理器或显式载荷调用。
        let handler =
            generate_event_handler_expression(&expression.expression, &change, "@change")?;
        // 在物化 View 前登记组件自己的类型化观察器。
        widget = quote! {
            (#widget).on_change(move |#change| {
                // 丢弃处理器返回值并保留副作用。
                let _ = { #handler };
            })
        };
    }

    // 物化已经完成状态和类型化事件配置的公开叶 View。
    let view = quote! { ::uix_app::prelude::ViewNode::leaf(#widget) };
    // 消费专有属性后应用统一尺寸、样式与其他公共事件。
    apply_common_attributes(
        // 传入已经配置完成的公开 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性与 Change 事件被二次映射。
        &[
            "files", "accept", "multiple", "maxCount", "maxSize", "drag", "@change",
        ],
    )
}

// 验证 UIX 字面量符合首版扩展名模式。
fn validate_accept_literal(pattern: &str, attribute: &Attribute) -> Result<(), Diagnostic> {
    // 空模式与两种通配模式均合法。
    if pattern.trim().is_empty() || matches!(pattern.trim(), "*" | "*/*") {
        // 完成通配模式验证。
        return Ok(());
    }
    // 每项必须是单段 ASCII 点扩展名模式。
    let valid = pattern.split(',').map(str::trim).all(|item| {
        // 拒绝空项、非 ASCII 与 MIME 分隔符。
        if item.is_empty() || !item.is_ascii() || item.contains('/') {
            // 当前项非法。
            return false;
        }
        // 只接受 .ext 或 *.ext。
        let extension = item
            // 优先移除星号点前缀。
            .strip_prefix("*.")
            // 再接受单点前缀。
            .or_else(|| item.strip_prefix('.'));
        // 扩展名必须非空且不含模式字符、点或空白。
        extension.is_some_and(|value| {
            // 收敛到首版确定性单扩展名语法。
            !value.is_empty()
                // 内部星号非法。
                && !value.contains('*')
                // 内部点非法。
                && !value.contains('.')
                // 内部空白非法。
                && !value.chars().any(char::is_whitespace)
        })
    });
    // 合法列表完成验证。
    if valid {
        // 不改变原始模式文本。
        return Ok(());
    }
    // 返回指向 accept 属性的编译期诊断。
    Err(Diagnostic::new(
        // 精确指向非法模式。
        attribute.span,
        // 明确首版支持范围。
        "Upload accept 仅支持空、*、*/*、.ext、*.ext 及其 ASCII 逗号列表",
        // 给出确定可修复的示例。
        "使用 accept=\".png,*.jpg\"；MIME 模式暂不支持",
    ))
}

// 生成无符号整数字面量或动态表达式。
fn integer_value(attribute: &Attribute, rust_type: &str) -> Result<TokenStream, Diagnostic> {
    // 按属性值形状生成令牌。
    match &attribute.value {
        // 字面量在生成期验证为无符号整数。
        AttributeValue::Literal(source) => {
            // 统一使用 u64 检查负数、小数与溢出，再由目标构造器推断类型。
            let value = source.parse::<u64>().map_err(|_| {
                // 返回精确属性诊断。
                Diagnostic::new(
                    // 指向非法整数属性。
                    attribute.span,
                    // 说明公开运行时数值类型。
                    format!("Upload {} 必须是 {rust_type} 无符号整数", attribute.name),
                    // 给出规范字面量或表达式写法。
                    format!(
                        "使用 {}=\"10\" 或 {}={{limit}}",
                        attribute.name, attribute.name
                    ),
                )
            })?;
            // maxCount 需要显式收敛为 usize。
            if rust_type == "usize" {
                // 检查目标平台 usize 范围。
                let value = usize::try_from(value).map_err(|_| {
                    // 返回目标类型溢出诊断。
                    Diagnostic::new(
                        // 指向溢出字面量。
                        attribute.span,
                        // 明确 usize 类型范围。
                        "Upload maxCount 超出 usize 范围",
                        // 指引使用较小正整数。
                        "使用当前平台可表示的 maxCount",
                    )
                })?;
                // 返回带类型推断的 usize 字面量。
                return Ok(quote! { #value });
            }
            // maxSize 保留 u64 字面量。
            Ok(quote! { #value })
        }
        // 动态表达式交给 Rust 消费端核对最终类型。
        AttributeValue::Expression(expression) => {
            // 生成受限 Rust 表达式。
            generate_expression(&expression.expression, None)
        }
        // 内联样式不能表达整数配置。
        AttributeValue::InlineStyle(_) => Err(Diagnostic::new(
            // 指向非法内联样式属性。
            attribute.span,
            // 说明无符号整数类型要求。
            format!("Upload {} 必须是 {rust_type} 无符号整数", attribute.name),
            // 给出规范值形状。
            format!(
                "使用 {}=\"10\" 或 {}={{limit}}",
                attribute.name, attribute.name
            ),
        )),
    }
}

// 查找 Upload 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享必需属性查找，仅绑定本组件的缺失诊断与修复建议。
    super::required_attribute(
        element,
        name,
        "Upload",
        "使用 <Upload files={upload_files} />",
    )
}

// 查找元素上的具名属性。
