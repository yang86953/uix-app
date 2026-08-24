// 引入卫生事件变量所需的标识符、跨度与令牌流。
use proc_macro2::{Ident, Span, TokenStream};
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 ImageGroup 属性、表达式、事件与诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, generate_event_handler_expression,
    generate_expression,
};

// 生成只声明图片集合、初始索引与变化观察器的 ImageGroup 叶节点。
pub(crate) fn generate_image_group(element: &Element) -> Result<TokenStream, Diagnostic> {
    // ImageGroup 自身绘制画廊并管理预览，不能静默忽略 View 子树。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 ImageGroup 元素。
            element.span,
            // 说明画廊不接受子节点。
            "<ImageGroup> 不接受子节点",
            // 给出最小合法数据绑定写法。
            "使用 <ImageGroup images={gallery_images} />",
        ));
    }

    // images 是画廊路径集合，必须显式提供。
    let images_attribute = required_attribute(element, "images")?;
    // 字符串字面量不能表达拥有型可迭代路径集合。
    let AttributeValue::Expression(images_expression) = &images_attribute.value else {
        // 返回集合表达式形状诊断。
        return Err(Diagnostic::new(
            // 指向非法 images 属性。
            images_attribute.span,
            // 说明公开运行时要求可迭代字符串集合。
            "ImageGroup images 必须是可迭代字符串表达式",
            // 给出规范数据引用写法。
            "使用 images={gallery_images}",
        ));
    };
    // 生成受限 Rust 路径集合表达式。
    let images_expression = generate_expression(&images_expression.expression, None)?;
    // 把数组或 Vec 的字符串项统一收集为运行时拥有的 Vec<String>。
    let images = quote! {
        ::std::iter::IntoIterator::into_iter((#images_expression).clone())
            .map(::std::convert::Into::into)
            .collect::<::std::vec::Vec<::std::string::String>>()
    };

    // 由 ImageGroup 运行时取得图片路径集合所有权。
    let mut widget = quote! { ::uix::prelude::ImageGroup::new().images(#images) };
    // 可选初始索引只配置首次物化位置。
    if let Some(attribute) = find_attribute(element, "startIndex") {
        // 生成 usize 字面量或受限表达式。
        let start_index = usize_value(attribute)?;
        // 运行时会按实际图片数量归一化初始索引。
        widget = quote! { (#widget).start_index(#start_index) };
    }

    // 先经公开 View 契约进入同目录 UIX 根，Change 处理器与样式由声明节点拥有。
    let mut view = quote! { ::uix::prelude::View::build(#widget) };
    // 可选变化事件只观察运行时已经提交的当前索引事实。
    if let Some(attribute) = find_attribute(element, "@change") {
        // 事件解析器应始终提供受限表达式。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回内部形状保护诊断。
            return Err(Diagnostic::new(
                // 指向完整事件属性。
                attribute.span,
                // 说明事件处理器形状。
                "ImageGroup @change 必须是受限处理器表达式",
                // 给出带索引文本载荷的规范写法。
                "使用 @change=\"on_gallery_change($event)\"",
            ));
        };
        // 创建卫生的索引文本变量。
        let value = Ident::new("__uix_image_group_change", Span::mixed_site());
        // 生成裸处理器或显式载荷调用。
        let handler = generate_event_handler_expression(&expression.expression, &value, "@change")?;
        // 使用公开 View Change 注册入口保存观察器。
        view = quote! {
            // 注册只接收现有索引十进制文本借用的闭包。
            (#view).on_change_fn(move |#value| {
                // 丢弃处理器返回值并保留调用方业务副作用。
                let _ = { #handler };
            })
        };
    }

    // 消费 ImageGroup 专有属性后应用统一尺寸、样式与其他公共事件。
    apply_common_attributes(
        // 传入已经配置集合、初始索引与变化观察器的公开 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性与 Change 事件被二次映射。
        &["images", "startIndex", "@change"],
    )
}

// 生成 usize 字面量或受限表达式。
fn usize_value(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 按属性值形状生成初始索引。
    match &attribute.value {
        // 静态值必须是 usize 整数。
        AttributeValue::Literal(source) => {
            // 拒绝负数、小数与溢出值。
            let value = source.parse::<usize>().map_err(|_| {
                // 返回整数类型诊断。
                Diagnostic::new(
                    // 指向非法 startIndex 属性。
                    attribute.span,
                    // 说明公开运行时类型。
                    "ImageGroup startIndex 必须是 usize 整数",
                    // 给出合法字面量或表达式。
                    "使用 startIndex=\"1\" 或 usize 表达式",
                )
            })?;
            // 生成类型明确的 usize 字面量。
            Ok(quote! { #value })
        }
        // 动态值保持 Rust usize 类型检查。
        AttributeValue::Expression(expression) => {
            // 生成受限整数表达式。
            generate_expression(&expression.expression, None)
        }
        // 结构化内联样式不能表达初始索引。
        AttributeValue::InlineStyle(_) => Err(Diagnostic::new(
            // 指向异常 startIndex 属性。
            attribute.span,
            // 说明整数类型要求。
            "ImageGroup startIndex 必须是 usize 整数",
            // 给出有效整数写法。
            "使用 usize 整数字面量或受限表达式",
        )),
    }
}

// 查找元素上的具名属性。
fn find_attribute<'a>(element: &'a Element, name: &str) -> Option<&'a Attribute> {
    // 解析器已保证同名属性唯一。
    element
        // 借用有序属性列表。
        .attributes
        // 遍历每个属性。
        .iter()
        // 返回首个名称匹配项。
        .find(|attribute| attribute.name == name)
}

// 查找 ImageGroup 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 缺失属性时构造确定诊断。
    find_attribute(element, name).ok_or_else(|| {
        // 返回完整必需属性错误。
        Diagnostic::new(
            // 指向完整 ImageGroup 元素。
            element.span,
            // 点名缺失属性。
            format!("<ImageGroup> 缺少必需的 {name} 属性"),
            // 给出最小合法写法和 capability 边界。
            "启用 image-codecs，并使用 <ImageGroup images={gallery_images} />",
        )
    })
}
