// 引入过程宏令牌流。
// 复用共享属性查找实现。
use super::find_attribute;

use proc_macro2::TokenStream;
// 引入结构化 Rust 令牌生成器。
use quote::quote;

// 引入公共属性与单节点生成入口。
use super::codegen::{apply_common_attributes, generate_node_view};
// 引入 List 属性、表达式、布尔值、字面量字符串、节点与诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, Node, boolean_value, generate_expression,
    literal_string, string_value,
};

// 保存已经验证并生成的 List 命名插槽。
struct ListSlots {
    // 保存可选页首 View。
    header: Option<TokenStream>,
    // 保存可选页尾 View。
    footer: Option<TokenStream>,
    // 保存可选加载入口 View。
    load_more: Option<TokenStream>,
}

// 生成文本数据、兼容字符串槽位与真实节点插槽共同组成的 List。
pub(crate) fn generate_list(element: &Element) -> Result<TokenStream, Diagnostic> {
    // 在配置运行时组件前验证并生成三个静态命名插槽。
    let slots = generate_list_slots(element)?;
    // 同一角色不能同时使用兼容字符串属性与真实节点插槽。
    reject_slot_attribute_conflict(element, "header", slots.header.is_some())?;
    // 页尾同样只能拥有一个声明来源。
    reject_slot_attribute_conflict(element, "footer", slots.footer.is_some())?;
    // 加载入口沿用 UIX 属性名 loadMore 作为角色名称。
    reject_slot_attribute_conflict(element, "loadMore", slots.load_more.is_some())?;

    // data 是列表文本来源，必须显式提供。
    let data_attribute = required_attribute(element, "data")?;
    // 字符串字面量不能表达拥有型可迭代列表。
    let AttributeValue::Expression(data_expression) = &data_attribute.value else {
        // 返回集合表达式形状诊断。
        return Err(Diagnostic::new(
            // 指向非法 data 属性。
            data_attribute.span,
            // 说明公开运行时要求可迭代字符串集合。
            "List data 必须是可迭代字符串表达式",
            // 给出规范数据引用写法。
            "使用 data={list_items}",
        ));
    };
    // 生成受限 Rust 列表表达式。
    let data = generate_expression(&data_expression.expression, None)?;
    // 把数组或 Vec 的文本项统一收集为运行时拥有的 Vec<String>。
    let items = quote! {
        ::std::iter::IntoIterator::into_iter((#data).clone())
            .map(::std::convert::Into::into)
            .collect::<::std::vec::Vec<::std::string::String>>()
    };

    // 从公开构造器开始并把文本数据交给运行时持有。
    let mut widget = quote! { ::uix::prelude::List::new().items(#items) };
    // 可选页首只声明现有字符串槽位。
    if let Some(attribute) = find_attribute(element, "header") {
        // 生成字符串字面量或受限字符串表达式。
        let header = string_value(attribute)?;
        // 构造期间临时借用页首文本，运行时负责复制。
        widget = quote! { (#widget).header(&*(#header)) };
    }
    // 可选页尾只声明现有字符串槽位。
    if let Some(attribute) = find_attribute(element, "footer") {
        // 生成字符串字面量或受限字符串表达式。
        let footer = string_value(attribute)?;
        // 构造期间临时借用页尾文本，运行时负责复制。
        widget = quote! { (#widget).footer(&*(#footer)) };
    }
    // 可选加载更多文字只投影现有运行时文本入口。
    if let Some(attribute) = find_attribute(element, "loadMore") {
        // 生成字符串字面量或受限字符串表达式。
        let load_more = string_value(attribute)?;
        // 构造期间临时借用加载更多文字，运行时负责复制。
        widget = quote! { (#widget).load_more(&*(#load_more)) };
    }
    // 可选页首节点完整交给运行时 List 子树生命周期。
    if let Some(header) = slots.header {
        // 节点入口保留目标 View 自己的事件、焦点与状态。
        widget = quote! { (#widget).header_view(#header) };
    }
    // 可选页尾节点进入独立稳定角色。
    if let Some(footer) = slots.footer {
        // 页尾不会退化成父组件自绘字符串。
        widget = quote! { (#widget).footer_view(#footer) };
    }
    // 可选加载入口可直接声明 Button 等交互 View。
    if let Some(load_more) = slots.load_more {
        // 运行时 List 只负责正常流布局，不复制点击处理器。
        widget = quote! { (#widget).load_more_view(#load_more) };
    }
    // 可选边框开关直接投影公开 List 构建器。
    if let Some(attribute) = find_attribute(element, "bordered") {
        // 接受布尔简写、字面量或受限表达式。
        let bordered = boolean_value(attribute)?;
        // 运行时继续独占边框绘制策略。
        widget = quote! { (#widget).bordered(#bordered) };
    }
    // 可选尺寸关键字映射到公开控件尺寸枚举。
    if let Some(attribute) = find_attribute(element, "size") {
        // 生成确定的公开尺寸枚举。
        let size = list_size(attribute)?;
        // 运行时继续独占行高、测量与绘制行为。
        widget = quote! { (#widget).size(#size) };
    }

    // 使用公开 View 契约保留空数据时的 Empty 替代生命周期。
    let view = quote! { ::uix::prelude::View::build(#widget) };
    // 消费 List 专有属性后应用统一尺寸、样式与自动化属性。
    apply_common_attributes(
        // 传入已经按运行时规则物化的公开 View。
        view,
        // 保留属性源码顺序供公共映射处理。
        &element.attributes,
        // 防止专有属性进入公共映射。
        &["data", "header", "footer", "loadMore", "bordered", "size"],
    )
}

// 验证并生成 List 的 header、footer 与 loadMore 直接命名插槽。
fn generate_list_slots(element: &Element) -> Result<ListSlots, Diagnostic> {
    // 初始化三个唯一插槽为空。
    let mut slots = ListSlots {
        // 尚未发现页首节点。
        header: None,
        // 尚未发现页尾节点。
        footer: None,
        // 尚未发现加载入口节点。
        load_more: None,
    };
    // 按源码顺序扫描 List 的直接子节点。
    for node in &element.children {
        // 排版空白不构成默认插槽内容。
        if matches!(node, Node::Text(text) if text.value.trim().is_empty()) {
            // 忽略格式化空白。
            continue;
        }
        // 只有普通直接元素可以声明稳定角色身份。
        let Node::Element(child) = node else {
            // 拒绝裸文本和插值形成未声明默认插槽。
            return Err(Diagnostic::new(
                // 指向完整 List 以覆盖直接内容。
                element.span,
                // 说明 List 没有默认插槽。
                "<List> 不接受裸文本、插值或默认插槽内容",
                // 给出三个显式角色写法。
                "使用静态直接元素，并声明 slot=\"header\"、slot=\"footer\" 或 slot=\"loadMore\"",
            ));
        };
        // 直接控制流不能保证一个角色只对应一个稳定 View 身份。
        if child.control.is_some()
            // 同时防御尚未附加控制绑定的控制标签。
            || matches!(child.name.as_str(), "If" | "ElseIf" | "Else" | "For")
        {
            // 返回动态直接插槽诊断。
            return Err(Diagnostic::new(
                // 指向非法控制元素。
                child.span,
                // 点明编译期静态直接元素约束。
                "<List> 命名插槽必须是静态直接 View，不能是 If 或 For",
                // 允许稳定容器内部继续使用控制流。
                "使用带 slot 属性的 Container 包裹 If 或 For",
            ));
        }
        // 克隆子元素以移除只负责插槽归位的属性。
        let mut child = child.clone();
        // 读取并移除必需的静态插槽名称。
        let slot = take_list_slot(&mut child)?;
        // 保留移除 slot 后仍对应同一源码元素的诊断跨度。
        let child_span = child.span;
        // 通过目标元素的正常生成器验证完整子树。
        let view = generate_node_view(&Node::Element(child))?;
        // 按登记名称写入唯一目标。
        let target = match slot.as_str() {
            // 页首进入 header_view 构建器。
            "header" => &mut slots.header,
            // 页尾进入 footer_view 构建器。
            "footer" => &mut slots.footer,
            // 加载入口进入 load_more_view 构建器。
            "loadMore" => &mut slots.load_more,
            // 其他名称没有运行时归属，必须显式拒绝。
            _ => {
                // 返回未知命名插槽诊断。
                return Err(Diagnostic::new(
                    // 指向完整直接子元素。
                    child_span,
                    // 点名未知名称。
                    format!("<List> 不支持名为 {slot} 的插槽"),
                    // 给出受支持的三个名称。
                    "使用 slot=\"header\"、slot=\"footer\" 或 slot=\"loadMore\"",
                ));
            }
        };
        // 每个角色只能有一个直接 View。
        if target.is_some() {
            // 返回重复插槽诊断。
            return Err(Diagnostic::new(
                // 指向后出现的重复子元素。
                child_span,
                // 点名重复目标。
                format!("<List> 的 {slot} 插槽重复声明"),
                // 给出唯一性修复动作。
                "每个命名插槽只保留一个静态直接 View",
            ));
        }
        // 保存已经生成的角色 View。
        *target = Some(view);
    }
    // 返回三个可选命名插槽。
    Ok(slots)
}

// 读取并移除 List 直接子元素的必需 slot 属性。
fn take_list_slot(element: &mut Element) -> Result<String, Diagnostic> {
    // 保存唯一 slot 属性的索引与字面量。
    let mut found = None;
    // 扫描直接子元素全部属性。
    for (index, attribute) in element.attributes.iter().enumerate() {
        // 其他属性交给子元素自己的生成器。
        if attribute.name != "slot" {
            // 继续扫描可能位于后方的 slot。
            continue;
        }
        // 同一子元素不能重复声明归位目标。
        if found.is_some() {
            // 返回重复属性诊断。
            return Err(Diagnostic::new(
                // 指向后出现的重复属性。
                attribute.span,
                // 说明归位目标必须唯一。
                "List 子节点 slot 属性重复声明",
                // 给出唯一属性写法。
                "只保留一个 slot=\"header\"、slot=\"footer\" 或 slot=\"loadMore\" 属性",
            ));
        }
        // 插槽名称必须在编译期确定。
        let AttributeValue::Literal(value) = &attribute.value else {
            // 返回动态名称诊断。
            return Err(Diagnostic::new(
                // 指向非法 slot 属性。
                attribute.span,
                // 说明不接受表达式或简写。
                "List 子节点 slot 属性必须是字符串字面量",
                // 给出合法静态名称。
                "使用 slot=\"header\"、slot=\"footer\" 或 slot=\"loadMore\"",
            ));
        };
        // 空名称不能伪装成默认插槽。
        if value.trim().is_empty() {
            // 返回空名称诊断。
            return Err(Diagnostic::new(
                // 指向空 slot 属性。
                attribute.span,
                // 说明 List 不提供默认插槽。
                "List 子节点 slot 名称不能为空",
                // 给出合法静态名称。
                "使用 slot=\"header\"、slot=\"footer\" 或 slot=\"loadMore\"",
            ));
        }
        // 保存待移除属性与名称。
        found = Some((index, value.clone()));
    }
    // List 的每个非空直接子元素都必须显式归位。
    let Some((index, slot)) = found else {
        // 返回缺少命名插槽诊断。
        return Err(Diagnostic::new(
            // 指向未归位直接元素。
            element.span,
            // 说明不存在默认插槽。
            "<List> 直接子 View 必须声明命名 slot",
            // 给出三个受支持目标。
            "添加 slot=\"header\"、slot=\"footer\" 或 slot=\"loadMore\"",
        ));
    };
    // 删除编译期归位属性，避免泄漏到子元素公共属性映射。
    element.attributes.remove(index);
    // 返回静态插槽名称。
    Ok(slot)
}

// 拒绝兼容字符串属性与同名节点插槽同时取得同一角色。
fn reject_slot_attribute_conflict(
    // 接收完整 List 元素。
    element: &Element,
    // 接收对应的父级属性名。
    name: &str,
    // 标记节点插槽是否存在。
    slot_present: bool,
) -> Result<(), Diagnostic> {
    // 没有节点插槽时字符串兼容属性可以正常工作。
    if !slot_present {
        // 明确返回无冲突。
        return Ok(());
    }
    // 查找是否同时声明了兼容字符串属性。
    let Some(attribute) = find_attribute(element, name) else {
        // 只有节点插槽时由运行时独占该角色。
        return Ok(());
    };
    // 同时声明会产生不透明的调用顺序，必须在生成期拒绝。
    Err(Diagnostic::new(
        // 指向冲突的父级属性。
        attribute.span,
        // 点名角色的双重声明。
        format!("List {name} 同时声明了字符串属性与节点插槽"),
        // 要求调用方明确选择兼容文本或真实节点。
        format!("删除 {name} 属性或 slot=\"{name}\" 子节点之一"),
    ))
}

// 映射 List 的编译期尺寸关键字。
fn list_size(attribute: &Attribute) -> Result<TokenStream, Diagnostic> {
    // 尺寸必须在编译期确定以拒绝未登记关键字。
    let value = literal_string(attribute, "List size")?;
    // 按公开三档控件尺寸生成枚举。
    match value.as_str() {
        // 小尺寸映射到 Small。
        "small" => Ok(quote! { ::uix::prelude::ControlSize::Small }),
        // 文档中尺寸映射到 Medium。
        "middle" => Ok(quote! { ::uix::prelude::ControlSize::Medium }),
        // 大尺寸映射到 Large。
        "large" => Ok(quote! { ::uix::prelude::ControlSize::Large }),
        // 其他关键字不能静默回退到运行时默认值。
        _ => Err(Diagnostic::new(
            // 指向完整 size 属性。
            attribute.span,
            // 陈述未知尺寸值。
            format!("List size={value:?} 不受支持"),
            // 给出完整合法集合。
            "使用 small、middle 或 large",
        )),
    }
}

// 查找元素上的具名属性。

// 查找 List 的必需属性。
fn required_attribute<'a>(element: &'a Element, name: &str) -> Result<&'a Attribute, Diagnostic> {
    // 复用共享必需属性查找，仅绑定本组件的缺失诊断与修复建议。
    super::required_attribute(element, name, "List", "使用 <List data={list_items} />")
}
