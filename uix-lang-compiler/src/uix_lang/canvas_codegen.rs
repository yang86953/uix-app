// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入共享公共属性生成器与可见节点判定。
use super::codegen::{apply_common_attributes, is_renderable_node};
// 引入 Canvas 所需的语法树、属性值生成器与诊断。
use super::{AttributeValue, Diagnostic, Element, generate_expression, numeric_value};

// 生成文档化 Canvas 标签对应的公开 Rust 绘制 View。
pub(crate) fn generate_canvas(element: &Element) -> Result<TokenStream, Diagnostic> {
    // Canvas 是叶子绘制组件，不能接收声明子节点。
    if element.children.iter().any(is_renderable_node) {
        // 返回带元素跨度的结构化形状诊断。
        return Err(Diagnostic::new(
            // 指向完整 Canvas 元素。
            element.span,
            // 说明 Canvas 不接收声明子内容。
            "<Canvas> 不接受子节点",
            // 给出文档化的自闭合写法。
            "使用 <Canvas width=\"590\" height=\"34\" paint={draw_fn} />",
        ));
    }
    // 按源码顺序收集三个专有属性。
    let (mut width, mut height, mut paint) = (None, None, None);
    for attribute in &element.attributes {
        // 按属性名分派专有语义。
        match attribute.name.as_str() {
            // 宽度接受有限数值、px 字面量或受限表达式。
            "width" => {
                // 生成确定的 f32 表达式。
                width = Some(numeric_value(attribute)?);
            }
            // 高度接受有限数值、px 字面量或受限表达式。
            "height" => {
                // 生成确定的 f32 表达式。
                height = Some(numeric_value(attribute)?);
            }
            // 绘制回调只接收 Rust 表达式；函数体在 Rust 侧实现。
            "paint" => {
                // 字面量不能拥有绘制行为。
                let AttributeValue::Expression(expression) = &attribute.value else {
                    // 返回带属性跨度的形状诊断。
                    return Err(Diagnostic::new(
                        // 指向非法 paint 属性。
                        attribute.span,
                        // 说明绘制回调必须是表达式。
                        "Canvas paint 必须是 Rust 绘制函数表达式",
                        // 给出绘制函数由 Rust 拥有的规范写法。
                        "使用 paint={draw_waveform}；绘制逻辑、状态读取与失效由 Rust 侧实现",
                    ));
                };
                // 复用受限表达式生成，签名由 rustc 在调用点验证。
                paint = Some(generate_expression(&expression.expression, None)?);
            }
            // 公共属性由统一生成器处理。
            _ => {}
        }
    }
    // 三个专有属性全部必填，缺一不可。
    let (Some(width), Some(height), Some(paint)) = (width, height, paint) else {
        // 返回缺失属性诊断。
        return Err(Diagnostic::new(
            // 指向完整 Canvas 元素。
            element.span,
            // 说明必填属性集合。
            "Canvas 需要 width、height 与 paint 三个属性",
            // 给出完整形状示例。
            "使用 <Canvas width=\"590\" height=\"34\" paint={draw_fn} />",
        ));
    };
    // 直接映射公开 canvas 组合器；绘制闭包内的 State 读取
    // 由运行时绑定为该节点的窄 Paint 失效，不形成每帧回调。
    let base = quote! { ::uix::prelude::canvas(#width as f32, #height as f32, #paint) };
    // 专有属性消费后继续复用统一样式与身份路径。
    apply_common_attributes(
        // 传入 Canvas 基础 View。
        base,
        // 传入原始属性列表。
        &element.attributes,
        // 防止专有属性被公共映射重复处理。
        &["width", "height", "paint"],
    )
}
