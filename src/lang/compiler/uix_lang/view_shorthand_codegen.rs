// 引入过程宏令牌流。
use proc_macro2::TokenStream;
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入 Container 复合阴影的共享解析入口。
use super::style_shadow_codegen::concrete_box_shadow_value;
// 引入边框所需的共享长度与颜色解析入口。
use super::style_value_codegen::{parse_color, parse_length};
// 引入 Container 属性、样式值和诊断契约。
use super::{
    Attribute, AttributeValue, Diagnostic, Element, StyleProperty, StyleValue, align_value,
    boolean_value, numeric_value, string_value,
};

// 登记 Container 专属生成阶段已经消费的全部属性。
pub(super) const CONTAINER_CONSUMED_ATTRIBUTES: &[&str] = &[
    // 方向由布局生成器消费。
    "direction",
    // 交叉轴自身对齐由当前模块消费。
    "alignSelf",
    // 背景色简写由当前模块消费。
    "bg",
    // 圆角简写由当前模块消费。
    "radius",
    // 固定宽度简写由当前模块消费。
    "w",
    // 固定高度简写由当前模块消费。
    "h",
    // 透明度简写由当前模块消费。
    "opacity",
    // 可见性简写由当前模块消费。
    "visible",
    // 溢出策略由当前模块消费。
    "overflow",
    // 固定边框简写由当前模块消费。
    "border",
    // 固定盒阴影简写由当前模块消费。
    "shadow",
];

// 按源码顺序应用 Container 专属视觉与布局简写。
pub(super) fn apply_container_shorthands(
    // 接收已经确定方向与子树的基础 View。
    mut base: TokenStream,
    // 接收原始 Container 元素。
    attributes: &[Attribute],
) -> Result<TokenStream, Diagnostic> {
    // 按源码顺序遍历专属属性。
    for attribute in attributes {
        // 只处理本组件登记的专属简写。
        base = match attribute.name.as_str() {
            // 当前容器作为父布局子项时覆盖交叉轴对齐。
            "alignSelf" => {
                // 复用统一 AlignItems 关键字验证。
                let value = align_value(attribute)?;
                // 应用公开 View 对齐入口。
                quote! { (#base).align_self(#value) }
            }
            // 背景色简写复用公开颜色值入口。
            "bg" => {
                // 接受字符串字面量或受限表达式。
                let value = string_value(attribute)?;
                // 应用现有背景色契约。
                quote! { (#base).bg(#value) }
            }
            // 圆角简写只接受有限非负数值。
            "radius" => {
                // 验证静态边界并生成动态表达式。
                let value =
                    container_number(attribute, "Container radius", NumberBoundary::NonNegative)?;
                // 应用现有统一圆角入口。
                quote! { (#base).radius(#value) }
            }
            // 固定宽度简写只接受有限非负逻辑像素。
            "w" => {
                // 验证静态边界并生成动态表达式。
                let value =
                    container_number(attribute, "Container w", NumberBoundary::NonNegative)?;
                // 应用现有固定宽度入口。
                quote! { (#base).width(#value) }
            }
            // 固定高度简写只接受有限非负逻辑像素。
            "h" => {
                // 验证静态边界并生成动态表达式。
                let value =
                    container_number(attribute, "Container h", NumberBoundary::NonNegative)?;
                // 应用现有固定高度入口。
                quote! { (#base).height(#value) }
            }
            // 透明度简写静态值限制在零到一。
            "opacity" => {
                // 验证静态边界并生成动态表达式。
                let value =
                    container_number(attribute, "Container opacity", NumberBoundary::UnitInterval)?;
                // 应用现有整体透明度入口。
                quote! { (#base).opacity(#value) }
            }
            // 可见性保持节点身份并切换完整子树参与资格。
            "visible" => {
                // 接受布尔简写、字面量或受限表达式。
                let value = boolean_value(attribute)?;
                // 应用现有可见性入口。
                quote! { (#base).visible(#value) }
            }
            // overflow 只登记可见与裁剪两种确定语义。
            "overflow" => apply_overflow(base, attribute)?,
            // border 使用固定宽度与具体颜色语法。
            "border" => apply_border(base, attribute)?,
            // shadow 使用完整双轴偏移盒阴影语法。
            "shadow" => apply_shadow(base, attribute)?,
            // 其余属性交给统一公共映射。
            _ => base,
        };
    }
    // 返回按源码顺序组装的 Container View。
    Ok(base)
}

// 应用 Container overflow 关键字。
fn apply_overflow(
    // 接收当前基础 View。
    base: TokenStream,
    // 接收 overflow 属性。
    attribute: &Attribute,
) -> Result<TokenStream, Diagnostic> {
    // 关键字必须在编译期确定。
    let value = super::literal_string(attribute, "Container overflow")?;
    // 把关键字映射为显式子树裁剪声明。
    let clip = match value.as_str() {
        // visible 不裁剪子树。
        "visible" => false,
        // hidden 裁剪到当前节点边界。
        "hidden" => true,
        // scroll 与 auto 由滚动组件持有状态。
        "scroll" | "auto" => {
            // 返回滚动所有权诊断。
            return Err(Diagnostic::new(
                attribute.span,
                "Container overflow 的 scroll/auto 由滚动容器组件提供",
                "改用 ScrollView 或 VirtualScroll",
            ));
        }
        // 其他关键字不能静默回退。
        _ => {
            // 返回合法集合诊断。
            return Err(Diagnostic::new(
                attribute.span,
                format!("Container overflow={value:?} 不受支持"),
                "使用 visible、hidden，或改用滚动容器",
            ));
        }
    };
    // 应用显式裁剪真假值。
    Ok(quote! { (#base).clip_content(#clip) })
}

// 应用固定宽度与具体颜色组成的边框简写。
fn apply_border(
    // 接收当前基础 View。
    base: TokenStream,
    // 接收 border 属性。
    attribute: &Attribute,
) -> Result<TokenStream, Diagnostic> {
    // 复合值只接受编译期字面量。
    let source = compound_literal(
        attribute,
        "Container border",
        "使用 border=\"1px #d9d9d9\"，动态值改用 style 或 Rust API",
    )?;
    // 首个空白把宽度与保留内部空格的颜色分开。
    let mut parts = source.splitn(2, char::is_whitespace);
    // 读取边框宽度。
    let width = parts.next().unwrap_or_default();
    // 读取并清理颜色外围空白。
    let color = parts.next().map(str::trim).unwrap_or_default();
    // 缺失任一分量时返回固定语法诊断。
    if width.is_empty() || color.is_empty() {
        // 指向完整 border 属性。
        return Err(Diagnostic::new(
            attribute.span,
            "Container border 需要宽度和颜色两个分量",
            "使用 border=\"1px #d9d9d9\"",
        ));
    }
    // 禁止主题引用与颜色名称在复合值中隐式解析。
    ensure_concrete_color(color, attribute, "border")?;
    // 构造共享样式解析器所需的来源信息。
    let property = style_property("border", source, attribute);
    // 解析有限非负宽度。
    let width = parse_length(width, &property)?;
    // 解析具体颜色通道。
    let (red, green, blue, alpha) = parse_color(color, &property)?;
    // 应用现有公开边框入口。
    Ok(quote! {
        (#base).border(
            #width,
            ::uix_app::prelude::ColorValue::custom(
                ::uix_app::prelude::Color::from_rgba(#red, #green, #blue, #alpha)
            )
        )
    })
}

// 应用完整盒阴影简写。
fn apply_shadow(
    // 接收当前基础 View。
    base: TokenStream,
    // 接收 shadow 属性。
    attribute: &Attribute,
) -> Result<TokenStream, Diagnostic> {
    // 复合值只接受编译期字面量。
    let source = compound_literal(
        attribute,
        "Container shadow",
        "使用 shadow=\"0 2px 8px rgba(0,0,0,0.15)\"，动态值改用 style 或 Rust API",
    )?;
    // 构造与 boxShadow 共用的结构化样式属性。
    let property = style_property("boxShadow", source, attribute);
    // 解析 none 或完整双轴偏移阴影定义。
    let value = concrete_box_shadow_value(&property)?;
    // 应用公开完整阴影入口。
    Ok(quote! { (#base).box_shadow(#value) })
}

// 读取只允许编译期确定的复合字面量。
fn compound_literal<'a>(
    // 接收待检查属性。
    attribute: &'a Attribute,
    // 接收诊断契约名。
    contract: &str,
    // 接收确定替代建议。
    suggestion: &str,
) -> Result<&'a str, Diagnostic> {
    // 只接受字符串字面量形状。
    let AttributeValue::Literal(source) = &attribute.value else {
        // 动态复合值不得猜测运行时分词语义。
        return Err(Diagnostic::new(
            attribute.span,
            format!("{contract} 必须是编译期确定的复合字面量"),
            suggestion,
        ));
    };
    // 返回借用的原始字面量。
    Ok(source)
}

// 构造复用样式值解析器所需的结构化属性。
fn style_property(
    // 接收规范样式属性名。
    name: &str,
    // 接收未改写值源码。
    source: &str,
    // 接收原始组件属性跨度。
    attribute: &Attribute,
) -> StyleProperty {
    // 复用原始跨度形成精确诊断。
    StyleProperty {
        // 保存规范属性名。
        name: name.to_owned(),
        // 保存没有主题哈希推断的具体值。
        value: StyleValue {
            // 保存原始复合值。
            source: source.to_owned(),
            // Container 复合值不登记主题引用。
            hashes: Vec::new(),
            // 使用完整属性跨度指向值来源。
            span: attribute.span,
        },
        // 使用完整属性跨度指向声明来源。
        span: attribute.span,
        // Container 复合值来自组件属性，不携带窗口条件。
        media: None,
    }
}

// 验证 Container 复合属性中的具体颜色语法。
fn ensure_concrete_color(
    // 接收颜色源码。
    source: &str,
    // 接收原始组件属性。
    attribute: &Attribute,
    // 接收诊断中的属性名。
    name: &str,
) -> Result<(), Diagnostic> {
    // 十六进制、rgb 与 rgba 属于登记的具体颜色。
    if source.starts_with('#') || source.starts_with("rgb(") || source.starts_with("rgba(") {
        // 合法语法继续交给共享颜色解析器验证通道。
        return Ok(());
    }
    // 拒绝名称、主题引用与其他未登记形态。
    Err(Diagnostic::new(
        attribute.span,
        format!("Container {name} 颜色 {source:?} 不是具体颜色字面量"),
        "使用十六进制、rgb(...) 或 rgba(...) 颜色",
    ))
}

// 声明 Container 简写数值的静态边界。
#[derive(Clone, Copy)]
enum NumberBoundary {
    // 只允许大于等于零。
    NonNegative,
    // 只允许零到一闭区间。
    UnitInterval,
}

// 验证静态 Container 数值并保留动态受限表达式。
fn container_number(
    // 接收待验证属性。
    attribute: &Attribute,
    // 接收诊断中的契约名称。
    contract: &str,
    // 接收允许的数值边界。
    boundary: NumberBoundary,
) -> Result<TokenStream, Diagnostic> {
    // 先复用统一有限数值与 px 解析。
    let tokens = numeric_value(attribute)?;
    // 动态表达式交给公开 Rust API 类型检查与运行时归一。
    let AttributeValue::Literal(source) = &attribute.value else {
        // 返回原始动态表达式令牌。
        return Ok(tokens);
    };
    // 去除可选 px 单位以取得已由统一解析验证的数值。
    let source = source.strip_suffix("px").unwrap_or(source);
    // 统一解析已经保证成功，这里只读取边界值。
    let value = source.parse::<f32>().expect("numeric_value 已验证有限 f32");
    // 根据字段契约验证静态范围。
    let valid = match boundary {
        // 非负字段允许零。
        NumberBoundary::NonNegative => value >= 0.0,
        // 透明度只允许零到一。
        NumberBoundary::UnitInterval => (0.0..=1.0).contains(&value),
    };
    // 合法静态数值直接返回既有令牌。
    if valid {
        // 保留统一 f32 字面量生成结果。
        return Ok(tokens);
    }
    // 为不同边界生成精确修复建议。
    let suggestion = match boundary {
        // 非负字段提示零或正数。
        NumberBoundary::NonNegative => "使用有限非负数值",
        // 透明度提示闭区间。
        NumberBoundary::UnitInterval => "使用 0 到 1 之间的数值",
    };
    // 返回带属性跨度的范围诊断。
    Err(Diagnostic::new(
        attribute.span,
        format!("{contract}={value} 超出允许范围"),
        suggestion,
    ))
}
