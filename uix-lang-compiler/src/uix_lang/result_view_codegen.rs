// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与叶节点形状判定。
// 引入共享元素属性查找。
use super::codegen::{apply_common_attributes, is_renderable_node};
use super::find_attribute;
// 引入 ResultView 属性、诊断与共享字符串生成契约。
use super::{Diagnostic, Element, literal_string, string_value};

// 生成只投影结果类型与文本配置的 ResultView 叶节点。
pub(crate) fn generate_result_view(element: &Element) -> Result<TokenStream, Diagnostic> {
    // ResultView 是叶组件，子树不能被静默忽略。
    if element.children.iter().any(is_renderable_node) {
        // 返回叶组件形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 ResultView 元素。
            element.span,
            // 说明结果页不接受子节点。
            "<ResultView> 不接受子节点",
            // 给出规范自闭合写法。
            "使用 <ResultView status=\"success\" title=\"操作成功\" />",
        ));
    }

    // 缺省结果类型遵循文档的 success 契约。
    let result_type = if let Some(attribute) = find_attribute(element, "status") {
        // 状态必须在编译期映射为公开枚举。
        let status = literal_string(attribute, "ResultView status")?;
        // 把文档关键字映射到公开运行时枚举。
        match status.as_str() {
            // 映射成功结果。
            "success" => quote! { ::uix_app::prelude::ResultType::Success },
            // 映射错误结果。
            "error" => quote! { ::uix_app::prelude::ResultType::Error },
            // 映射信息结果。
            "info" => quote! { ::uix_app::prelude::ResultType::Info },
            // 映射警告结果。
            "warning" => quote! { ::uix_app::prelude::ResultType::Warning },
            // 映射未找到结果。
            "404" => quote! { ::uix_app::prelude::ResultType::NotFound },
            // 拒绝文档外的状态关键字。
            _ => {
                // 返回包含合法集合的确定性诊断。
                return Err(Diagnostic::new(
                    // 指向非法 status 属性。
                    attribute.span,
                    // 说明未登记值。
                    format!("ResultView status={status:?} 不受支持"),
                    // 给出完整合法集合。
                    "使用 success、error、info、warning 或 404",
                ));
            }
        }
    } else {
        // 未声明状态时使用成功结果。
        quote! { ::uix_app::prelude::ResultType::Success }
    };
    // 从公开构造器开始配置。
    let mut widget = quote! { ::uix_app::prelude::ResultView::new(#result_type) };
    // 可选标题接受字符串字面量或受限字符串表达式。
    if let Some(attribute) = find_attribute(element, "title") {
        // 生成标题字符串令牌。
        let title = string_value(attribute)?;
        // 借用字符串交给会复制内容的公开标题构建器。
        widget = quote! { (#widget).title(&*(#title)) };
    }
    // 可选辅助文字接受字符串字面量或受限字符串表达式。
    if let Some(attribute) = find_attribute(element, "extraText") {
        // 生成辅助文字字符串令牌。
        let extra_text = string_value(attribute)?;
        // 调用公开辅助文字构建器。
        widget = quote! { (#widget).extra_text(#extra_text) };
    }

    // 经公开 View 契约进入组件自己的同目录 UIX 声明壳。
    let view = quote! { ::uix_app::prelude::View::build(#widget) };
    // 消费 ResultView 专有属性并返回公共 View 表达式。
    apply_common_attributes(
        // 传入已经配置的结果页 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["status", "title", "extraText"],
    )
}

// 查找元素上的具名属性。
