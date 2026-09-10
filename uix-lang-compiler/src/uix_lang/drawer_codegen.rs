// 引入过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与有序子树生成入口。
use super::codegen::{apply_common_attributes, generate_children};
// 引入 Drawer 属性、表达式、数值与诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, generate_expression, literal_string,
    numeric_value,
};

// 生成由 State<bool> 唯一控制并保留完整内容子树的 Drawer。
pub(crate) fn generate_drawer(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 从运行时文档默认值构造无标题 Drawer。
    let mut drawer = quote! { ::uix_app::prelude::Drawer::new("") };

    // 可选 open 必须保留声明端 State<bool> 句柄。
    if let Some(attribute) = find_attribute(element, "open") {
        // 解析受控状态表达式并保留 Rust 类型检查。
        let open = state_expression(attribute)?;
        // 运行时 Drawer 克隆状态句柄并负责用户关闭写回。
        drawer = quote! { (#drawer).controlled_open(&(#open)) };
    }
    // 可选 placement 必须在编译期选择确定方向。
    if let Some(attribute) = find_attribute(element, "placement") {
        // 把文档关键字映射到公开运行时枚举。
        let placement = placement_value(attribute)?;
        // 把方向所有权交给运行时 Drawer。
        drawer = quote! { (#drawer).placement(#placement) };
    }
    // 可选 width 作为面板宽度消费，不落入公共 View 布局宽度。
    if let Some(attribute) = find_attribute(element, "width") {
        // 接受十进制、px 字面量或受限数值表达式。
        let width = numeric_value(attribute)?;
        // 调用 Drawer 的窄面板宽度构建契约。
        drawer = quote! { (#drawer).width(#width) };
    }

    // 按源码顺序生成普通节点与 If/For 控制流。
    let children = generate_children(&element.children)?;
    // 通过公开 ViewNode 契约把 Drawer 与完整内容子树物化。
    let view = quote! { ::uix_app::prelude::ViewNode::new(#drawer, #children) };
    // 消费 Drawer 专有属性后应用其他公共样式、身份与事件。
    apply_common_attributes(
        // 传入已配置受控状态、几何与内容的公开 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止 Drawer 专有属性被二次映射。
        &["open", "placement", "width"],
    )
}

// 解析 Drawer 的受控打开状态表达式。
fn state_expression(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 字面量不能提供可订阅和可写回的 State<bool> 句柄。
    let AttributeValue::Expression(expression) = &attribute.value else {
        // 返回带来源位置的受控状态诊断。
        return Err(Diagnostic::new(
            // 指向非法 open 属性。
            attribute.span,
            // 明确状态句柄类型要求。
            "Drawer open 必须是 State<bool> 表达式",
            // 给出受控绑定写法。
            "使用 open={drawer_open}",
        ));
    };
    // 生成受限 Rust 表达式并把最终类型检查交给调用 crate。
    generate_expression(&expression.expression, None)
}

// 解析 Drawer 的确定性方向关键字。
fn placement_value(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 方向必须是编译期可验证的普通字符串字面量。
    let placement = literal_string(attribute, "Drawer placement")?;
    // 把文档值映射到公开枚举。
    match placement.as_str() {
        // 映射左侧抽屉。
        "left" => Ok(quote! { ::uix_app::prelude::DrawerPlacement::Left }),
        // 映射右侧抽屉。
        "right" => Ok(quote! { ::uix_app::prelude::DrawerPlacement::Right }),
        // 映射顶部抽屉。
        "top" => Ok(quote! { ::uix_app::prelude::DrawerPlacement::Top }),
        // 映射底部抽屉。
        "bottom" => Ok(quote! { ::uix_app::prelude::DrawerPlacement::Bottom }),
        // 拒绝未登记方向。
        _ => Err(Diagnostic::new(
            // 指向完整 placement 属性。
            attribute.span,
            // 点明具体非法值。
            format!("Drawer placement={placement:?} 不受支持"),
            // 给出文档允许集合。
            "使用 left、right、top 或 bottom",
        )),
    }
}

// 查找元素上的具名属性。
