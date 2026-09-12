// 引入过程宏令牌流。
use proc_macro2::TokenStream;

// 引入结构化诊断与样式属性。
use super::{Diagnostic, StyleProperty};

// 为不属于 UIX 布局模型的 float/clear 生成专用诊断。
pub(super) fn reject_float_or_clear(
    // 接收保留源码跨度的样式属性。
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 按两个闭合属性分别说明语法与架构边界。
    match property.name.as_str() {
        // float 不建立 CSS 浮动格式上下文。
        "float" => Err(float_diagnostic(property)),
        // clear 没有浮动兄弟可清除。
        "clear" => Err(clear_diagnostic(property)),
        // 调用方只应交付两个已登记名称。
        _ => Err(Diagnostic::new(
            // 指向完整属性作为内部边界保护。
            property.span,
            // 说明错误的调用形状。
            "浮动布局诊断只处理 float 或 clear",
            // 引导维护者核对样式分派矩阵。
            "核对 UIX 样式属性分派",
        )),
    }
}

// 构造 float 的架构决策诊断。
fn float_diagnostic(property: &StyleProperty) -> Diagnostic {
    // 判断源码是否属于 CSS 闭合关键字。
    let recognized = matches!(property.value.source.trim(), "left" | "right" | "none");
    // 合法 CSS 值与拼写错误分别给出精确原因。
    let message = if recognized {
        // 规范值仍因 UIX 无浮动格式上下文而拒绝。
        "UIX 不提供 CSS 浮动格式上下文，float 明确不属于目标设计"
    } else {
        // 非规范值同时说明 CSS 值边界与 UIX 决策。
        "float 的 CSS 值只包括 left、right 或 none，且 UIX 不提供浮动格式上下文"
    };
    // 返回指向属性值的可修复诊断。
    Diagnostic::new(
        // 精确标记失败值。
        property.value.span,
        // 使用按值形状选择的原因。
        message,
        // 给出 UIX 原生布局替代方案。
        "使用 flexDirection: row，并配合 justifyContent、flexGrow 或 Grid 轨道",
    )
}

// 构造 clear 的架构决策诊断。
fn clear_diagnostic(property: &StyleProperty) -> Diagnostic {
    // 判断源码是否属于 CSS 闭合关键字。
    let recognized = matches!(
        // 读取去除空白的值。
        property.value.source.trim(),
        // 覆盖完整 clear 关键字集合。
        "left" | "right" | "both" | "none"
    );
    // 合法 CSS 值与拼写错误分别给出精确原因。
    let message = if recognized {
        // 没有浮动兄弟时 clear 不建立独立运行时字段。
        "UIX 没有可清除的浮动格式上下文，clear 明确不属于目标设计"
    } else {
        // 非规范值同时说明 CSS 值边界与 UIX 决策。
        "clear 的 CSS 值只包括 left、right、both 或 none，且 UIX 没有浮动格式上下文"
    };
    // 返回指向属性值的可修复诊断。
    Diagnostic::new(
        // 精确标记失败值。
        property.value.span,
        // 使用按值形状选择的原因。
        message,
        // 给出重新开始一行或分区的原生替代方案。
        "使用 Column/Row 重新分组，或用 Grid 显式声明新行",
    )
}
