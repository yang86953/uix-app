// 引入过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与有序子树生成入口。
use super::codegen::{apply_common_attributes, generate_children};
// 引入 Modal 属性、表达式、值映射与诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, boolean_value, generate_expression,
    generate_handler_expression, string_value,
};

// 生成由 State<bool> 唯一控制并保留完整内容子树的 Modal。
pub(crate) fn generate_modal(element: &Element) -> Result<TokenStream, Diagnostic> {
    // UIX 文档约定页脚缺省可见，因此显式覆盖受控构建器的兼容默认值。
    let mut builder = quote! { ::uix_app::prelude::Modal::builder().footer_visible(true) };

    // 可选 open 必须保留声明端 State<bool> 句柄作为唯一可见性事实源。
    if let Some(attribute) = find_attribute(element, "open") {
        // 解析受控状态表达式并保留 Rust 类型检查。
        let open = state_expression(attribute)?;
        // 运行时 Modal 克隆状态句柄并负责用户关闭后的写回。
        builder = quote! { (#builder).open(&(#open)) };
    }
    // 可选标题接受字符串字面量或受限字符串表达式。
    if let Some(attribute) = find_attribute(element, "title") {
        // 生成统一字符串值令牌。
        let title = string_value(attribute)?;
        // Modal 复制借用标题，调用侧表达式无需满足静态生命周期。
        builder = quote! { (#builder).title(&*(#title)) };
    }
    // 可选遮罩关闭策略接受布尔简写、字面量或受限表达式。
    if let Some(attribute) = find_attribute(element, "maskClosable") {
        // 生成统一布尔值令牌。
        let mask_closable = boolean_value(attribute)?;
        // 把遮罩策略交给运行时输入所有者。
        builder = quote! { (#builder).mask_closable(#mask_closable) };
    }
    // 可选页脚显隐接受布尔简写、字面量或受限表达式。
    if let Some(attribute) = find_attribute(element, "footerVisible") {
        // 生成统一布尔值令牌。
        let footer_visible = boolean_value(attribute)?;
        // 覆盖文档缺省页脚策略并同步布局与命中几何。
        builder = quote! { (#builder).footer_visible(#footer_visible) };
    }
    // 可选确认事件映射为 Modal 实例拥有的同步窄回调。
    if let Some(attribute) = find_attribute(element, "@ok") {
        // 解析不携带额外事件载荷的受限处理器表达式。
        let handler = handler_expression(attribute, "@ok")?;
        // 丢弃处理器返回值并保留确认副作用。
        builder = quote! { (#builder).on_ok(move || { let _ = { #handler }; }) };
    }
    // 可选取消事件覆盖取消按钮、关闭槽、遮罩与 Escape 的共同事实。
    if let Some(attribute) = find_attribute(element, "@cancel") {
        // 解析不携带额外事件载荷的受限处理器表达式。
        let handler = handler_expression(attribute, "@cancel")?;
        // 丢弃处理器返回值并保留取消副作用。
        builder = quote! { (#builder).on_cancel(move || { let _ = { #handler }; }) };
    }

    // 按源码顺序生成普通节点与 If/For 控制流。
    let children = generate_children(&element.children)?;
    // 把完整 View 集合交给 ModalBuilder，避免压缩为单个占位内容。
    builder = quote! { (#builder).content_nodes(#children) };
    // 通过公开 View 契约物化 Modal 与其内容子树。
    let view = quote! { ::uix_app::prelude::View::build(#builder) };
    // 消费 Modal 专有属性后应用统一尺寸、样式、身份与其他公共事件。
    apply_common_attributes(
        // 传入已经配置受控状态、回调与内容的公开 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止 Modal 专有属性和事件被二次映射。
        &[
            "open",
            "title",
            "maskClosable",
            "footerVisible",
            "@ok",
            "@cancel",
        ],
    )
}

// 解析 Modal 的受控打开状态表达式。
fn state_expression(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 字面量不能提供可订阅和可写回的 State<bool> 句柄。
    let AttributeValue::Expression(expression) = &attribute.value else {
        // 返回带来源位置的受控状态诊断。
        return Err(Diagnostic::new(
            // 指向非法 open 属性。
            attribute.span,
            // 明确状态句柄类型要求。
            "Modal open 必须是 State<bool> 表达式",
            // 给出受控绑定写法。
            "使用 open={modal_open}",
        ));
    };
    // 生成受限 Rust 表达式并把最终类型检查交给调用 crate。
    generate_expression(&expression.expression, None)
}

// 解析 Modal 无载荷操作事件。
fn handler_expression(attribute: &Attribute, name: &str) -> Result<TokenStream, Diagnostic> {
    // 事件解析器应始终提供受限表达式。
    let AttributeValue::Expression(expression) = &attribute.value else {
        // 返回内部形状保护诊断。
        return Err(Diagnostic::new(
            // 指向完整事件属性。
            attribute.span,
            // 明确处理器表达式要求。
            format!("Modal {name} 必须是受限处理器表达式"),
            // 给出无载荷处理器写法。
            format!("使用 {name}=\"on_modal_action\""),
        ));
    };
    // 复用统一裸处理器或显式调用生成逻辑。
    generate_handler_expression(&expression.expression, None)
}

// 查找元素上的具名属性。
