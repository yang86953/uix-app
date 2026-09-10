// 引入卫生局部变量与过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::{Ident, Span, TokenStream};
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与有序子树生成入口。
use super::codegen::{apply_common_attributes, generate_children};
// 引入 Carousel 属性、布尔值与诊断契约。
use super::{Diagnostic, Element, boolean_value};

// 生成保留幻灯片顺序与运行时计时器所有权的 Carousel 容器。
pub(crate) fn generate_carousel(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 从不启用自动播放的公开构造器开始。
    let mut widget = quote! { ::uix_app::prelude::Carousel::new() };
    // 可选 autoplay 接受布尔简写、字面量或受限表达式。
    if let Some(attribute) = find_attribute(element, "autoplay") {
        // 复用统一布尔值诊断并保留 Rust 类型检查。
        let autoplay = boolean_value(attribute)?;
        // 创建不会捕获调用方同名变量的卫生局部名称。
        let carousel = Ident::new("__uix_carousel", Span::mixed_site());
        // true 使用项目 Rust 文档现有的三秒间隔，false 保持计时器关闭。
        widget = quote! {{
            // 只构造一次组件，避免动态表达式重复求值。
            let #carousel = #widget;
            // 按声明值决定是否启用既有运行时计时器。
            if #autoplay {
                // 采用文档登记的三秒自动播放间隔。
                #carousel.autoplay(::std::time::Duration::from_secs(3_u64))
            } else {
                // 关闭时保留默认无计时器配置。
                #carousel
            }
        }};
    }

    // 按源码顺序生成普通节点与 If/For 幻灯片。
    let children = generate_children(&element.children)?;
    // 经公开桥接进入 Carousel 自己的同目录 UIX 根声明，并原样移交幻灯片子树。
    let view = quote! { (#widget).build_view_with_children(#children) };
    // 消费 Carousel 专有属性并应用公共尺寸、样式、身份与事件。
    apply_common_attributes(view, &element.attributes, &["autoplay"])
    // 结束 Carousel 生成函数。
}

// 查找元素上的具名属性。
