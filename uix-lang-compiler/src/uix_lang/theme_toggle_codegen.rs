// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入共享公共属性生成器与可见节点判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 ThemeToggle 所需的语法树与诊断。
use super::{Diagnostic, Element};

// 生成文档化 ThemeToggle 标签对应的公开 Rust View。
pub(crate) fn generate_theme_toggle(element: &Element) -> Result<TokenStream, Diagnostic> {
    // ThemeToggle 是叶子组件，不能静默丢弃可见子节点。
    if element.children.iter().any(is_renderable_node) {
        // 返回带元素跨度的结构化形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 ThemeToggle 元素。
            element.span,
            // 说明 ThemeToggle 不接收子内容。
            "<ThemeToggle> 不接受子节点",
            // 给出文档化的自闭合写法。
            "使用 <ThemeToggle />",
        ));
    }
    // 复用现有公开 ThemeToggle Widget 默认契约，并把其 Change 语义接入
    // uix-lang 唯一的 App 级主题请求通道。
    let base = quote! {
        // 把组件包装成公开叶 View。
        ::uix::prelude::ViewNode::leaf(::uix::prelude::ThemeToggle::new())
            // ThemeToggle 的文本载荷只有 light / dark，两者都由 App 主题表解析。
            .on_change_fn(move |__uix_theme_name| {
                // 与 setTheme 内置操作复用同一请求入口，禁止组件另存主题状态。
                ::uix::ui::__private::uix_set_theme(__uix_theme_name);
            })
    };
    // 继续复用统一样式、事件与未知属性诊断路径。
    apply_common_attributes(
        // 传入 ThemeToggle 基础 View。
        base,
        // 传入原始属性列表。
        &element.attributes,
        // ThemeToggle 当前没有专有属性。
        &[],
    )
}
