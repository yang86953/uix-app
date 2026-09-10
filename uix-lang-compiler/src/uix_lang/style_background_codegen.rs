// 引入卫生标识符、浮点字面量与生成令牌流。
use proc_macro2::{Ident, Literal, TokenStream};
// 引入确定性 Rust 令牌拼接宏。
use quote::quote;

// 引入结构化诊断与样式属性。
use super::{Diagnostic, StyleProperty};
// 复用统一颜色值解析与主题引用能力。
use super::style_value_codegen::color_value;

// 区分二维定位中的关键词轴。
#[derive(Clone, Copy, PartialEq, Eq)]
enum KeywordAxis {
    // 水平轴关键词。
    Horizontal,
    // 垂直轴关键词。
    Vertical,
    // center 可以补齐任意一个轴。
    Center,
}

// 保存已解析关键词及其运行时令牌。
#[derive(Clone)]
struct KeywordPosition {
    // 保存关键词所属轴。
    axis: KeywordAxis,
    // 保存公开运行时枚举表达式。
    value: TokenStream,
}

// 把 backgroundImage 映射为显式单层背景来源。
pub(super) fn background_image_field(
    // 接收 map_style 闭包中的卫生 Style 标识符。
    style: &Ident,
    // 接收保留源码跨度的 backgroundImage 属性。
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 去除值两端空白并保留内部路径空格。
    let source = property.value.source.trim();
    // 顶层逗号代表首批契约不支持的多背景层。
    if has_top_level_comma(source) {
        // 返回明确的单层能力边界诊断。
        return Err(background_image_diagnostic(
            // 传入原始属性跨度。
            property,
            // 说明不支持多层。
            "backgroundImage 首批只支持单个本地 url 或单个双色渐变",
        ));
    }
    // 按闭合背景来源语法生成公开运行时值。
    let value = if source == "none" {
        // 显式 none 保持可覆盖继承图片的语义。
        quote! { ::uix_app::prelude::BackgroundImage::None }
    } else if let Some(inner) = function_inner(source, "url") {
        // URL 必须使用成对单引号或双引号包裹。
        let path = quoted_text(inner.trim()).ok_or_else(|| {
            // 返回路径引用形状诊断。
            background_image_diagnostic(property, "backgroundImage 的 url 路径必须使用引号包裹")
        })?;
        // 空路径不能形成稳定资源身份。
        if path.is_empty() {
            // 返回空路径诊断。
            return Err(background_image_diagnostic(
                // 传入原始属性。
                property,
                // 说明路径不能为空。
                "backgroundImage 的本地图片路径不能为空",
            ));
        }
        // 网络、数据与协议相对 URL 不属于本地资源契约。
        if is_remote_or_embedded_path(path) {
            // 返回本地资源能力边界诊断。
            return Err(background_image_diagnostic(
                // 传入原始属性。
                property,
                // 说明首批只接受本地路径。
                "backgroundImage 首批只支持本地文件路径，不支持网络或内嵌 URL",
            ));
        }
        // 为生成代码持有独立路径字符串。
        let path = path.to_owned();
        // 生成公开本地图片来源。
        quote! { ::uix_app::prelude::BackgroundImage::Url(::std::string::String::from(#path)) }
    } else if let Some(inner) = function_inner(source, "linear-gradient") {
        // 解析恰好两个颜色端点。
        let (start, end) = gradient_colors(inner, property)?;
        // 生成固定上下方向的双色线性渐变。
        quote! { ::uix_app::prelude::BackgroundImage::LinearGradient { start: #start, end: #end } }
    } else if let Some(inner) = function_inner(source, "radial-gradient") {
        // 解析恰好两个颜色端点。
        let (inner, outer) = gradient_colors(inner, property)?;
        // 生成固定中心向外的双色径向渐变。
        quote! { ::uix_app::prelude::BackgroundImage::RadialGradient { inner: #inner, outer: #outer } }
    } else {
        // 未登记来源不得静默回退为 none。
        return Err(background_image_diagnostic(
            // 传入原始属性。
            property,
            // 列出完整首批来源语法。
            "backgroundImage 只支持 none、带引号的本地 url()、linear-gradient() 或 radial-gradient()",
        ));
    };
    // Some 区分显式 none 与未声明背景图。
    Ok(quote! { #style.background_image = ::std::option::Option::Some(#value); })
}

// 把 backgroundPosition 映射为显式二维定位。
pub(super) fn background_position_field(
    // 接收 map_style 闭包中的卫生 Style 标识符。
    style: &Ident,
    // 接收保留源码跨度的 backgroundPosition 属性。
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 按空白切分一项或两项声明。
    let parts = property
        // 读取原始值。
        .value
        // 读取值源码。
        .source
        // 按 Unicode 空白切分。
        .split_whitespace()
        // 保存有序分量。
        .collect::<Vec<_>>();
    // 按分量数量选择确定解析规则。
    let (x, y) = match parts.as_slice() {
        // 单值按关键词轴或水平数值补齐另一个中心值。
        [only] => one_position_value(only, property)?,
        // 双值支持两个轴关键词任意顺序或数值的 x y 顺序。
        [first, second] => two_position_values(first, second, property)?,
        // 空值或三项以上不属于首批闭合语法。
        _ => {
            // 返回分量数量诊断。
            return Err(background_position_diagnostic(property));
        }
    };
    // 生成显式二维定位字段更新。
    Ok(quote! {
        #style.background_position = ::std::option::Option::Some(
            ::uix_app::prelude::BackgroundPosition::new(#x, #y)
        );
    })
}

// 把 backgroundRepeat 映射为显式闭合重复枚举。
pub(super) fn background_repeat_field(
    // 接收 map_style 闭包中的卫生 Style 标识符。
    style: &Ident,
    // 接收保留源码跨度的 backgroundRepeat 属性。
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 只接受文档登记的四种规范关键字。
    let value = match property.value.source.trim() {
        // 两个轴都重复。
        "repeat" => quote! { ::uix_app::prelude::BackgroundRepeat::Repeat },
        // 只沿水平轴重复。
        "repeat-x" => quote! { ::uix_app::prelude::BackgroundRepeat::RepeatX },
        // 只沿垂直轴重复。
        "repeat-y" => quote! { ::uix_app::prelude::BackgroundRepeat::RepeatY },
        // 两个轴都不重复。
        "no-repeat" => quote! { ::uix_app::prelude::BackgroundRepeat::NoRepeat },
        // 未登记关键字必须在编译期明确拒绝。
        _ => {
            // 返回完整支持集合诊断。
            return Err(Diagnostic::new(
                // 精确指向失败值。
                property.value.span,
                // 列出闭合重复集合。
                "backgroundRepeat 只支持 repeat、repeat-x、repeat-y 或 no-repeat",
                // 给出最常用的不重复写法。
                "使用 backgroundRepeat: no-repeat",
            ));
        }
    };
    // Some 保留显式 repeat 与未声明默认值之间的差异。
    Ok(quote! { #style.background_repeat = ::std::option::Option::Some(#value); })
}

// 解析单值背景定位并补齐另一个轴为 center。
fn one_position_value(
    // 接收唯一分量。
    source: &str,
    // 接收诊断所属属性。
    property: &StyleProperty,
) -> Result<(TokenStream, TokenStream), Diagnostic> {
    // 关键词按自身轴放置。
    if let Some(keyword) = keyword_position(source) {
        // 中心值同时应用到两个轴。
        if keyword.axis == KeywordAxis::Center {
            // 返回两个中心枚举。
            return Ok((keyword.value.clone(), keyword.value));
        }
        // 垂直关键词让水平轴居中。
        if keyword.axis == KeywordAxis::Vertical {
            // 返回中心水平与指定垂直值。
            return Ok((center_position(), keyword.value));
        }
        // 水平关键词让垂直轴居中。
        return Ok((keyword.value, center_position()));
    }
    // 单个长度或百分比解释为水平值。
    let horizontal = numeric_position(source, property)?;
    // 垂直轴使用中心默认补齐。
    Ok((horizontal, center_position()))
}

// 解析双值背景定位。
fn two_position_values(
    // 接收第一个分量。
    first: &str,
    // 接收第二个分量。
    second: &str,
    // 接收诊断所属属性。
    property: &StyleProperty,
) -> Result<(TokenStream, TokenStream), Diagnostic> {
    // 尝试把两个分量都解释为关键词。
    let first_keyword = keyword_position(first);
    // 解析第二个关键词。
    let second_keyword = keyword_position(second);
    // 两项都是关键词时允许轴顺序交换。
    if let (Some(first), Some(second)) = (first_keyword.clone(), second_keyword.clone()) {
        // 委托关键词轴配对规则。
        return keyword_pair(first, second, property);
    }
    // 关键词与数值混合会产生轴歧义，首批明确拒绝。
    if first_keyword.is_some() || second_keyword.is_some() {
        // 返回统一二维定位诊断。
        return Err(background_position_diagnostic(property));
    }
    // 两个数值严格按 x y 顺序解释。
    let x = numeric_position(first, property)?;
    // 第二个数值解释为垂直轴。
    let y = numeric_position(second, property)?;
    // 返回确定的两个轴值。
    Ok((x, y))
}

// 将两个关键词分配到水平与垂直轴。
fn keyword_pair(
    // 接收第一个关键词。
    first: KeywordPosition,
    // 接收第二个关键词。
    second: KeywordPosition,
    // 接收诊断所属属性。
    property: &StyleProperty,
) -> Result<(TokenStream, TokenStream), Diagnostic> {
    // 两个 center 明确映射为二维居中。
    if first.axis == KeywordAxis::Center && second.axis == KeywordAxis::Center {
        // 保持源码顺序但两个值等价。
        return Ok((first.value, second.value));
    }
    // 第一个 center 补齐第二个关键词的另一轴。
    if first.axis == KeywordAxis::Center {
        // 第二个垂直关键词放到 y 轴。
        if second.axis == KeywordAxis::Vertical {
            // 返回中心 x 与指定 y。
            return Ok((first.value, second.value));
        }
        // 第二个水平关键词放到 x 轴。
        return Ok((second.value, first.value));
    }
    // 第二个 center 补齐第一个关键词的另一轴。
    if second.axis == KeywordAxis::Center {
        // 第一个垂直关键词放到 y 轴。
        if first.axis == KeywordAxis::Vertical {
            // 返回中心 x 与指定 y。
            return Ok((second.value, first.value));
        }
        // 第一个水平关键词放到 x 轴。
        return Ok((first.value, second.value));
    }
    // 水平后接垂直按轴返回。
    if first.axis == KeywordAxis::Horizontal && second.axis == KeywordAxis::Vertical {
        // 返回 x y 顺序。
        return Ok((first.value, second.value));
    }
    // 垂直后接水平需要交换到 x y 顺序。
    if first.axis == KeywordAxis::Vertical && second.axis == KeywordAxis::Horizontal {
        // 返回交换后的轴顺序。
        return Ok((second.value, first.value));
    }
    // 两个同轴非中心关键词无法形成二维定位。
    Err(background_position_diagnostic(property))
}

// 将定位关键词解析为公开单轴枚举。
fn keyword_position(source: &str) -> Option<KeywordPosition> {
    // 按闭合关键词建立轴和值。
    let (axis, value) = match source {
        // left 对齐水平轴起点。
        "left" => (
            KeywordAxis::Horizontal,
            quote! { ::uix_app::prelude::BackgroundAxisPosition::Start },
        ),
        // right 对齐水平轴终点。
        "right" => (
            KeywordAxis::Horizontal,
            quote! { ::uix_app::prelude::BackgroundAxisPosition::End },
        ),
        // top 对齐垂直轴起点。
        "top" => (
            KeywordAxis::Vertical,
            quote! { ::uix_app::prelude::BackgroundAxisPosition::Start },
        ),
        // bottom 对齐垂直轴终点。
        "bottom" => (
            KeywordAxis::Vertical,
            quote! { ::uix_app::prelude::BackgroundAxisPosition::End },
        ),
        // center 可以补齐任意一个轴。
        "center" => (KeywordAxis::Center, center_position()),
        // 非关键词交给数值解析。
        _ => return None,
    };
    // 返回类型化关键词。
    Some(KeywordPosition { axis, value })
}

// 解析有限 px 或零到一百百分比定位。
fn numeric_position(
    // 接收单个定位分量。
    source: &str,
    // 接收诊断所属属性。
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 百分比映射为零到一的剩余空间比例。
    if let Some(number) = source.strip_suffix('%') {
        // 解析百分比数值。
        let value = finite_f32(number, property)?;
        // 首批契约只接受零到一百百分比。
        if !(0.0..=100.0).contains(&value) {
            // 返回范围诊断。
            return Err(background_position_diagnostic(property));
        }
        // 转换为运行时零到一比例。
        let value = Literal::f32_unsuffixed(value / 100.0);
        // 生成百分比单轴值。
        return Ok(quote! { ::uix_app::prelude::BackgroundAxisPosition::Percent(#value) });
    }
    // 固定长度必须显式使用 px 单位。
    let Some(number) = source.strip_suffix("px") else {
        // 无单位数值及其他单位都明确拒绝。
        return Err(background_position_diagnostic(property));
    };
    // 解析允许正负的有限像素值。
    let value = Literal::f32_unsuffixed(finite_f32(number, property)?);
    // 生成像素单轴值。
    Ok(quote! { ::uix_app::prelude::BackgroundAxisPosition::Pixels(#value) })
}

// 解析有限 f32 定位分量。
fn finite_f32(source: &str, property: &StyleProperty) -> Result<f32, Diagnostic> {
    // 使用 Rust 浮点解析器读取十进制数值。
    let value = source
        // 去除数值与单位间可选空白。
        .trim()
        // 解析为运行时同精度数值。
        .parse::<f32>()
        // 非数值返回统一定位诊断。
        .map_err(|_| background_position_diagnostic(property))?;
    // NaN 与无穷值不能形成稳定绘制几何。
    if !value.is_finite() {
        // 返回与其他非法定位一致的诊断。
        return Err(background_position_diagnostic(property));
    }
    // 返回有限值。
    Ok(value)
}

// 返回单轴中心枚举表达式。
fn center_position() -> TokenStream {
    // 使用公开 UI 背景定位契约。
    quote! { ::uix_app::prelude::BackgroundAxisPosition::Center }
}

// 解析双色渐变的两个颜色值。
fn gradient_colors(
    // 接收不含外围函数括号的文本。
    source: &str,
    // 接收诊断所属属性。
    property: &StyleProperty,
) -> Result<(TokenStream, TokenStream), Diagnostic> {
    // 在颜色函数括号外切分端点。
    let colors = split_top_level_commas(source, property)?;
    // 首批只接受恰好两个颜色且不接受角度与色标。
    if colors.len() != 2 || colors.iter().any(|color| color.is_empty()) {
        // 返回双色闭合能力诊断。
        return Err(background_image_diagnostic(
            // 传入原始属性。
            property,
            // 说明渐变端点限制。
            "背景渐变首批只支持恰好两个颜色，不支持角度、色标或多层",
        ));
    }
    // 使用统一颜色值解析第一个端点。
    let first = color_from_source(colors[0], property)?;
    // 使用统一颜色值解析第二个端点。
    let second = color_from_source(colors[1], property)?;
    // 返回两个运行时 ColorValue 表达式。
    Ok((first, second))
}

// 用既有颜色映射解析渐变中的独立颜色文本。
fn color_from_source(
    // 接收单个颜色源码。
    source: &str,
    // 接收原属性用于跨度与诊断。
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 克隆结构化属性以复用主题引用与颜色诊断。
    let mut color_property = property.clone();
    // 把值源码缩小到当前颜色端点。
    color_property.value.source = source.trim().to_owned();
    // 委托统一 ColorValue 生成器。
    color_value(&color_property)
}

// 在嵌套颜色函数外切分逗号。
fn split_top_level_commas<'a>(
    // 接收渐变内部文本。
    source: &'a str,
    // 接收诊断所属属性。
    property: &StyleProperty,
) -> Result<Vec<&'a str>, Diagnostic> {
    // 保存当前圆括号深度。
    let mut depth = 0usize;
    // 保存当前颜色起点。
    let mut start = 0usize;
    // 保存有序颜色片段。
    let mut values = Vec::new();
    // 遍历 UTF-8 字符边界。
    for (index, value) in source.char_indices() {
        // 左括号进入嵌套颜色函数。
        if value == '(' {
            // 增加括号深度。
            depth += 1;
        // 右括号退出嵌套颜色函数。
        } else if value == ')' {
            // 多余右括号属于非法渐变形状。
            if depth == 0 {
                // 返回背景图语法诊断。
                return Err(background_image_diagnostic(property, "背景渐变括号不匹配"));
            }
            // 安全减少括号深度。
            depth -= 1;
        // 顶层逗号结束当前颜色。
        } else if value == ',' && depth == 0 {
            // 保存去除空白的颜色片段。
            values.push(source[start..index].trim());
            // 下一个片段从逗号后开始。
            start = index + value.len_utf8();
        }
    }
    // 未闭合左括号必须明确拒绝。
    if depth != 0 {
        // 返回背景图语法诊断。
        return Err(background_image_diagnostic(property, "背景渐变括号不匹配"));
    }
    // 保存最后一个颜色片段。
    values.push(source[start..].trim());
    // 返回有序片段。
    Ok(values)
}

// 识别完整函数调用并返回内部文本。
fn function_inner<'a>(source: &'a str, name: &str) -> Option<&'a str> {
    // 构造规范函数前缀。
    let prefix = format!("{name}(");
    // 必须同时匹配函数名、左括号与最终右括号。
    source
        // 去除规范前缀。
        .strip_prefix(&prefix)
        // 去除最后一个右括号。
        .and_then(|inner| inner.strip_suffix(')'))
}

// 从成对单引号或双引号中提取静态文本。
fn quoted_text(source: &str) -> Option<&str> {
    // 至少需要两个引号字符。
    if source.len() < 2 {
        // 报告不是引号文本。
        return None;
    }
    // 读取首字符。
    let first = source.chars().next()?;
    // 读取末字符。
    let last = source.chars().next_back()?;
    // 只接受相同的单引号或双引号。
    if first != last || !matches!(first, '\'' | '"') {
        // 报告不是规范引号文本。
        return None;
    }
    // 返回不含外围 ASCII 引号的文本。
    Some(&source[1..source.len() - 1])
}

// 判断路径是否需要网络或内嵌资源能力。
fn is_remote_or_embedded_path(path: &str) -> bool {
    // 使用小写副本进行协议前缀判断。
    let lowercase = path.to_ascii_lowercase();
    // 拒绝完整协议 URL、协议相对 URL 与 data URI。
    path.contains("://") || path.starts_with("//") || lowercase.starts_with("data:")
}

// 判断源码是否包含圆括号外的逗号。
fn has_top_level_comma(source: &str) -> bool {
    // 保存当前函数括号深度。
    let mut depth = 0usize;
    // 遍历全部字符。
    for value in source.chars() {
        // 左括号进入函数。
        if value == '(' {
            // 增加深度。
            depth += 1;
        // 右括号退出函数。
        } else if value == ')' {
            // 使用饱和减法避免恶意输入下溢。
            depth = depth.saturating_sub(1);
        // 顶层逗号表示多层。
        } else if value == ',' && depth == 0 {
            // 立即报告命中。
            return true;
        }
    }
    // 没有顶层逗号。
    false
}

// 构造 backgroundImage 的统一诊断。
fn background_image_diagnostic(property: &StyleProperty, message: &str) -> Diagnostic {
    // 返回指向完整属性值的修复性诊断。
    Diagnostic::new(
        // 精确标记失败值。
        property.value.span,
        // 传入具体失败原因。
        message,
        // 给出覆盖全部首批来源的规范示例。
        "使用 none、url('image.png')、linear-gradient(#fff, #000) 或 radial-gradient(#fff, #000)",
    )
}

// 构造 backgroundPosition 的统一诊断。
fn background_position_diagnostic(property: &StyleProperty) -> Diagnostic {
    // 返回指向完整属性值的修复性诊断。
    Diagnostic::new(
        // 精确标记失败值。
        property.value.span,
        // 说明一项与两项闭合语法。
        "backgroundPosition 只支持一到两个轴关键词、有限 px 或 0% 到 100%",
        // 给出关键词与数值两种规范写法。
        "使用 center、top left、12px 8px 或 50% 25%",
    )
}
