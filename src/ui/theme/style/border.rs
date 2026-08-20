// 定义 UI System 拥有的类 CSS 边框线型值。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BorderStyle {
    // 不绘制边框，但盒模型仍保留声明宽度。
    None,
    // 绘制单道连续边框。
    #[default]
    Solid,
    // 绘制连续相位的虚线边框。
    Dashed,
    // 绘制连续相位的圆点边框。
    Dotted,
    // 绘制两道同色边框。
    Double,
}

// 验证显式线型值覆盖与 none 绘制 Gate。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../tests/unit/ui/theme/style/border__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
