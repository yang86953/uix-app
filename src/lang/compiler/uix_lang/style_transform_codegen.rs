// 引入过程宏令牌流与确定性拼接宏。
use proc_macro2::TokenStream;
// 引入 Rust 表达式令牌生成能力。
use quote::quote;

// 引入共享诊断与结构化样式属性。
use super::{Diagnostic, StyleProperty};

// 保存代码生成期可验证的二维 3x2 仿射矩阵。
type Matrix = [f32; 6];

// 保存单轴变换原点的公开运行时值。
#[derive(Clone, Copy)]
enum OriginCoordinate {
    // 保存相对布局帧尺寸的比例。
    Fraction(f32),
    // 保存相对布局帧起点的像素偏移。
    Pixels(f32),
}

// 保存关键字携带的轴限制或无轴长度值。
#[derive(Clone, Copy)]
enum OriginToken {
    // 只允许出现在水平轴位置。
    Horizontal(OriginCoordinate),
    // 只允许出现在垂直轴位置。
    Vertical(OriginCoordinate),
    // 可按上下文解释为任一轴中心。
    Center,
    // 无轴限制的长度或百分比。
    Coordinate(OriginCoordinate),
}

// 把 transformOrigin 映射为公开二维原点值表达式。
pub(super) fn transform_origin_value(
    // 接收结构化样式属性。
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 按 CSS 空白分隔一到三个原点分量。
    let mut parts = property
        // 读取未改写值源码。
        .value
        // 借用源码字符串。
        .source
        // 按连续空白切分。
        .split_whitespace()
        // 收集以便处理可选第三轴。
        .collect::<Vec<_>>();
    // 原点至少需要一个分量，最多允许文档默认中的第三轴零值。
    if parts.is_empty() || parts.len() > 3 {
        // 返回分量数量诊断。
        return Err(transform_diagnostic(
            property,
            "transformOrigin 需要一到三个分量",
            "使用 center、top left、25% 12px 或 50% 50% 0",
        ));
    }
    // 三分量形式的 Z 轴当前只允许二维等价值零。
    if parts.len() == 3 {
        // 读取第三轴文本。
        let depth = parts[2];
        // 仅接受有限的无单位零或零像素。
        if !is_zero_depth(depth) {
            // 返回二维运行时能力边界诊断。
            return Err(transform_diagnostic(
                property,
                "transformOrigin 第三个 Z 轴分量必须为 0",
                "二维运行时只支持 0 或 0px",
            ));
        }
        // 移除已验证的零 Z 轴，只生成二维原点。
        parts.pop();
    }
    // 单分量使用 CSS 默认的另一轴中心语义。
    let (horizontal, vertical) = if parts.len() == 1 {
        // 解析唯一分量。
        single_origin(parts[0], property)?
    } else {
        // 两分量允许常规顺序与 top left 关键字交换顺序。
        pair_origin(parts[0], parts[1], property)?
    };
    // 生成两个轴的公开值表达式。
    let horizontal = origin_coordinate_tokens(horizontal);
    // 生成垂直轴公开值表达式。
    let vertical = origin_coordinate_tokens(vertical);
    // 返回由 UI System 根公开的二维原点构造器。
    Ok(quote! {
        ::uix_app::prelude::TransformOrigin::new(#horizontal, #vertical)
    })
}

// 解析单分量原点并补齐另一轴中心。
fn single_origin(
    // 接收唯一分量文本。
    source: &str,
    // 接收诊断所属属性。
    property: &StyleProperty,
) -> Result<(OriginCoordinate, OriginCoordinate), Diagnostic> {
    // 解析关键字或数值分量。
    let token = origin_token(source, property)?;
    // 中心比例在两个轴上相同。
    let center = OriginCoordinate::Fraction(0.5);
    // 按分量轴限制补齐另一轴。
    match token {
        // 水平关键字保留指定值，垂直轴居中。
        OriginToken::Horizontal(value) => Ok((value, center)),
        // 垂直关键字保持指定值，水平轴居中。
        OriginToken::Vertical(value) => Ok((center, value)),
        // 单独 center 表示两轴中心。
        OriginToken::Center => Ok((center, center)),
        // 单独长度或百分比解释为水平轴，垂直轴居中。
        OriginToken::Coordinate(value) => Ok((value, center)),
    }
}

// 解析两个原点分量并确定水平与垂直轴。
fn pair_origin(
    // 接收第一分量。
    first: &str,
    // 接收第二分量。
    second: &str,
    // 接收诊断所属属性。
    property: &StyleProperty,
) -> Result<(OriginCoordinate, OriginCoordinate), Diagnostic> {
    // 解析第一分量语义。
    let first = origin_token(first, property)?;
    // 解析第二分量语义。
    let second = origin_token(second, property)?;
    // 中心比例可按上下文充当任一轴。
    let center = OriginCoordinate::Fraction(0.5);
    // 覆盖规范顺序、垂直关键字前置和 center 消歧。
    match (first, second) {
        // 标准水平关键字加垂直关键字。
        (OriginToken::Horizontal(x), OriginToken::Vertical(y)) => Ok((x, y)),
        // top left 等垂直关键字前置形式需要交换轴。
        (OriginToken::Vertical(y), OriginToken::Horizontal(x)) => Ok((x, y)),
        // 水平关键字后接 center。
        (OriginToken::Horizontal(x), OriginToken::Center) => Ok((x, center)),
        // center 后接垂直关键字。
        (OriginToken::Center, OriginToken::Vertical(y)) => Ok((center, y)),
        // 垂直关键字后接 center。
        (OriginToken::Vertical(y), OriginToken::Center) => Ok((center, y)),
        // center 后接水平关键字时交换为水平值与垂直中心。
        (OriginToken::Center, OriginToken::Horizontal(x)) => Ok((x, center)),
        // 两个 center 表示两轴中心。
        (OriginToken::Center, OriginToken::Center) => Ok((center, center)),
        // 两个无轴长度值按水平、垂直顺序解释。
        (OriginToken::Coordinate(x), OriginToken::Coordinate(y)) => Ok((x, y)),
        // 水平关键字可与垂直数值组合。
        (OriginToken::Horizontal(x), OriginToken::Coordinate(y)) => Ok((x, y)),
        // 水平数值可与垂直关键字组合。
        (OriginToken::Coordinate(x), OriginToken::Vertical(y)) => Ok((x, y)),
        // 垂直关键字前置时允许第二项为水平数值。
        (OriginToken::Vertical(y), OriginToken::Coordinate(x)) => Ok((x, y)),
        // 水平数值后接 center 时垂直轴居中。
        (OriginToken::Coordinate(x), OriginToken::Center) => Ok((x, center)),
        // center 后接数值时数值解释为垂直轴。
        (OriginToken::Center, OriginToken::Coordinate(y)) => Ok((center, y)),
        // 其余组合重复声明同一轴或顺序含糊。
        _ => Err(transform_diagnostic(
            property,
            "transformOrigin 两个分量无法确定水平与垂直轴",
            "使用 left top、top left、center bottom、25% 12px 等明确组合",
        )),
    }
}

// 解析一个关键字、百分比或像素分量。
fn origin_token(
    // 接收分量文本。
    source: &str,
    // 接收诊断所属属性。
    property: &StyleProperty,
) -> Result<OriginToken, Diagnostic> {
    // 映射五个文档关键字，否则解析长度。
    match source {
        // 左侧是水平零比例。
        "left" => Ok(OriginToken::Horizontal(OriginCoordinate::Fraction(0.0))),
        // 右侧是水平完整比例。
        "right" => Ok(OriginToken::Horizontal(OriginCoordinate::Fraction(1.0))),
        // 顶部是垂直零比例。
        "top" => Ok(OriginToken::Vertical(OriginCoordinate::Fraction(0.0))),
        // 底部是垂直完整比例。
        "bottom" => Ok(OriginToken::Vertical(OriginCoordinate::Fraction(1.0))),
        // center 由分量上下文决定轴。
        "center" => Ok(OriginToken::Center),
        // 其他文本必须是静态百分比或像素值。
        _ => parse_origin_coordinate(source, property).map(OriginToken::Coordinate),
    }
}

// 解析百分比、像素或无单位固定值。
fn parse_origin_coordinate(
    // 接收分量文本。
    source: &str,
    // 接收诊断所属属性。
    property: &StyleProperty,
) -> Result<OriginCoordinate, Diagnostic> {
    // 百分比转换为运行时零到一比例，保留帧外数值能力。
    if let Some(percent) = source.strip_suffix('%') {
        // 解析有限百分比并换算比例。
        let value = parse_finite(percent, "transformOrigin 百分比", property)? / 100.0;
        // 返回比例轴值。
        return Ok(OriginCoordinate::Fraction(value));
    }
    // 去除可选 px 后缀；其他单位会在数值解析时失败。
    let pixels = source.strip_suffix("px").unwrap_or(source);
    // 解析有限固定像素值。
    let value = parse_finite(pixels, "transformOrigin 长度", property)?;
    // 返回像素轴值。
    Ok(OriginCoordinate::Pixels(value))
}

// 判断可选第三轴是否为二维等价零。
fn is_zero_depth(source: &str) -> bool {
    // 去除唯一允许的 px 后缀。
    let number = source.strip_suffix("px").unwrap_or(source);
    // 只接受可解析的有限零值。
    number
        // 解析为 f32 以覆盖 +0 与 -0。
        .parse::<f32>()
        // 非有限值不能作为零深度。
        .is_ok_and(|value| value.is_finite() && value == 0.0)
}

// 生成单轴公开原点值表达式。
fn origin_coordinate_tokens(value: OriginCoordinate) -> TokenStream {
    // 按轴值类型选择公开构造器。
    match value {
        // 比例值延迟到布局帧确定后解析。
        OriginCoordinate::Fraction(value) => {
            // 生成比例构造器。
            quote! { ::uix_app::prelude::TransformOriginValue::fraction(#value) }
        }
        // 像素值相对布局帧起点解析。
        OriginCoordinate::Pixels(value) => {
            // 生成像素构造器。
            quote! { ::uix_app::prelude::TransformOriginValue::pixels(#value) }
        }
    }
}

// 把 transform 函数列表映射为公开 Transform 组合表达式。
pub(super) fn transform_value(property: &StyleProperty) -> Result<TokenStream, Diagnostic> {
    // 读取已经由样式解析器去除外围空白的源码。
    let source = property.value.source.as_str();
    // none 明确恢复单位变换。
    if source == "none" {
        // 返回公开几何层的单位矩阵构造器。
        return Ok(quote! { ::uix_app::draw::Transform::identity() });
    }
    // 空值没有确定变换语义。
    if source.is_empty() {
        // 返回缺失函数诊断。
        return Err(transform_diagnostic(
            property,
            "transform 至少需要一个变换函数",
            "使用 none、translate(...)、rotate(...)、scale(...) 或 skew(...)",
        ));
    }
    // 保存代码生成期的最终矩阵，供有限性与可逆性检查。
    let mut matrix = identity_matrix();
    // 保存与代码生成期组合顺序完全一致的 Rust 表达式。
    let mut expression = quote! { ::uix_app::draw::Transform::identity() };
    // 按源码顺序解析并组合函数。
    for (name, arguments) in parse_functions(source, property)? {
        // 把单个函数转换为矩阵事实和公开构造表达式。
        let (part_matrix, part_expression) = transform_function(name, arguments, property)?;
        // CSS 函数列表按矩阵乘积顺序组合，右侧函数先作用于点。
        matrix = concat_matrix(matrix, part_matrix);
        // 生成与代码生成期矩阵相同顺序的运行时组合。
        expression = quote! { (#expression).concat(#part_expression) };
    }
    // 任一非有限系数都会污染绘制、包围盒与命中计算。
    if matrix.iter().any(|value| !value.is_finite()) {
        // 拒绝无法安全进入运行时的矩阵。
        return Err(transform_diagnostic(
            property,
            "transform 生成了非有限矩阵",
            "减小角度或数值，避免接近 90deg 的无限倾斜",
        ));
    }
    // 计算二维线性部分的行列式。
    let determinant = matrix[0] * matrix[4] - matrix[1] * matrix[3];
    // 命中路由需要最终矩阵可逆。
    if determinant.abs() <= f32::EPSILON {
        // 拒绝会让节点命中坐标无法还原的变换。
        return Err(transform_diagnostic(
            property,
            "transform 生成了不可逆矩阵",
            "避免零缩放或相互抵消为零行列式的倾斜组合",
        ));
    }
    // 返回已完成静态验证的公开运行时表达式。
    Ok(expression)
}

// 从 transform 源码中提取有序函数名与参数文本。
fn parse_functions<'a>(
    // 接收不含外围分号的属性值。
    source: &'a str,
    // 接收诊断所属属性。
    property: &StyleProperty,
) -> Result<Vec<(&'a str, &'a str)>, Diagnostic> {
    // 保存解析出的函数列表。
    let mut functions = Vec::new();
    // 使用字节偏移处理 ASCII 函数语法，同时保持切片边界确定。
    let mut cursor = 0usize;
    // 扫描到源码末尾。
    while cursor < source.len() {
        // 跳过函数之间的空白。
        cursor = skip_ascii_whitespace(source, cursor);
        // 尾部只有空白时结束。
        if cursor >= source.len() {
            // 退出函数扫描。
            break;
        }
        // 记录函数名起点。
        let name_start = cursor;
        // 函数名只允许 ASCII 字母。
        while source
            // 读取当前字节。
            .as_bytes()
            // 安全访问当前偏移。
            .get(cursor)
            // 判断是否为名称字符。
            .is_some_and(u8::is_ascii_alphabetic)
        {
            // 消费一个名称字节。
            cursor += 1;
        }
        // 没有函数名表示存在游离符号。
        if cursor == name_start {
            // 返回结构诊断。
            return Err(transform_diagnostic(
                property,
                "transform 函数列表包含非法字符",
                "使用以字母开头的 translate、rotate、scale 或 skew 函数",
            ));
        }
        // 保存函数名切片。
        let name = &source[name_start..cursor];
        // 函数名与左括号之间允许普通空白。
        cursor = skip_ascii_whitespace(source, cursor);
        // 左括号必须紧随函数名区域。
        if source.as_bytes().get(cursor) != Some(&b'(') {
            // 返回缺少调用括号诊断。
            return Err(transform_diagnostic(
                property,
                format!("transform 函数 {name} 缺少 ("),
                format!("使用 {name}(...)"),
            ));
        }
        // 跳过左括号。
        cursor += 1;
        // 记录参数起点。
        let arguments_start = cursor;
        // 扫描到对应右括号；参数不允许嵌套函数。
        while let Some(byte) = source.as_bytes().get(cursor) {
            // 嵌套左括号没有已登记语义。
            if *byte == b'(' {
                // 返回嵌套函数诊断。
                return Err(transform_diagnostic(
                    property,
                    "transform 参数不支持嵌套函数",
                    "只使用静态数值、px、deg、rad 或 turn 参数",
                ));
            }
            // 右括号结束当前函数。
            if *byte == b')' {
                // 停止参数扫描。
                break;
            }
            // 消费一个参数字节。
            cursor += 1;
        }
        // 输入结束表示缺少右括号。
        if source.as_bytes().get(cursor) != Some(&b')') {
            // 返回闭合诊断。
            return Err(transform_diagnostic(
                property,
                format!("transform 函数 {name} 缺少 )"),
                format!("闭合 {name}(...) 调用"),
            ));
        }
        // 去除参数外围空白。
        let arguments = source[arguments_start..cursor].trim();
        // 空参数没有确定矩阵。
        if arguments.is_empty() {
            // 返回缺失参数诊断。
            return Err(transform_diagnostic(
                property,
                format!("transform 函数 {name} 缺少参数"),
                "按样式参考提供函数参数",
            ));
        }
        // 保存函数及其参数源码。
        functions.push((name, arguments));
        // 跳过右括号。
        cursor += 1;
    }
    // 至少必须解析出一个函数。
    if functions.is_empty() {
        // 返回缺失函数诊断。
        return Err(transform_diagnostic(
            property,
            "transform 没有可映射的函数",
            "使用 none、translate(...)、rotate(...)、scale(...) 或 skew(...)",
        ));
    }
    // 返回保持源码顺序的函数列表。
    Ok(functions)
}

// 把一个已切分函数映射为矩阵与运行时表达式。
fn transform_function(
    // 接收函数名。
    name: &str,
    // 接收未改写参数文本。
    arguments: &str,
    // 接收诊断所属属性。
    property: &StyleProperty,
) -> Result<(Matrix, TokenStream), Diagnostic> {
    // 按逗号或空白切分一到两个静态参数。
    let values = split_arguments(arguments, property)?;
    // 按文档登记的四类函数生成矩阵。
    match name {
        // 映射二维平移。
        "translate" => {
            // 平移接受一个或两个长度值。
            let (x, y) = one_or_two(&values, name, property, parse_length, 0.0)?;
            // 返回代码生成期矩阵与公开构造器。
            Ok((
                translate_matrix(x, y),
                quote! { ::uix_app::draw::Transform::translate(#x, #y) },
            ))
        }
        // 映射二维缩放。
        "scale" => {
            // 单参数等比缩放，双参数分别控制两轴。
            let (x, y) = one_or_two(&values, name, property, parse_scalar, f32::NAN)?;
            // 单参数时 Y 轴沿用 X 轴值。
            let y = if y.is_nan() { x } else { y };
            // 返回代码生成期矩阵与公开构造器。
            Ok((
                scale_matrix(x, y),
                quote! { ::uix_app::draw::Transform::scale(#x, #y) },
            ))
        }
        // 映射二维旋转。
        "rotate" => {
            // 旋转只接受一个角度值。
            let angle = exactly_one(&values, name, property, parse_angle)?;
            // 返回代码生成期矩阵与公开构造器。
            Ok((
                rotate_matrix(angle),
                quote! { ::uix_app::draw::Transform::rotate(#angle) },
            ))
        }
        // 映射双轴倾斜。
        "skew" => {
            // 单参数只倾斜 X 轴，双参数分别控制两轴。
            let (x, y) = one_or_two(&values, name, property, parse_angle, 0.0)?;
            // 返回代码生成期矩阵与公开构造器。
            Ok((
                skew_matrix(x, y),
                quote! { ::uix_app::draw::Transform::skew(#x, #y) },
            ))
        }
        // 其他函数不在当前文档映射范围内。
        _ => Err(transform_diagnostic(
            property,
            format!("未知 transform 函数 {name}"),
            "使用 translate、rotate、scale 或 skew",
        )),
    }
}

// 按逗号或空白切分静态参数。
fn split_arguments<'a>(
    // 接收函数括号内文本。
    source: &'a str,
    // 接收诊断所属属性。
    property: &StyleProperty,
) -> Result<Vec<&'a str>, Diagnostic> {
    // 含逗号时使用传统 CSS 逗号分隔，否则使用空白分隔。
    let values = if source.contains(',') {
        // 逐段去除空白并保留空项供诊断。
        source.split(',').map(str::trim).collect::<Vec<_>>()
    } else {
        // 现代空白分隔形式天然忽略连续空白。
        source.split_whitespace().collect::<Vec<_>>()
    };
    // 逗号产生空项时参数结构非法。
    if values.iter().any(|value| value.is_empty()) {
        // 返回参数缺失诊断。
        return Err(transform_diagnostic(
            property,
            "transform 函数包含空参数",
            "删除多余逗号或补全缺失参数",
        ));
    }
    // 返回有序参数切片。
    Ok(values)
}

// 解析恰好一个参数的函数。
fn exactly_one(
    // 接收已经切分的参数。
    values: &[&str],
    // 接收函数名。
    name: &str,
    // 接收诊断所属属性。
    property: &StyleProperty,
    // 接收单值解析器。
    parse: fn(&str, &StyleProperty) -> Result<f32, Diagnostic>,
) -> Result<f32, Diagnostic> {
    // 参数数量必须为一。
    if values.len() != 1 {
        // 返回数量诊断。
        return Err(argument_count_diagnostic(name, "1", property));
    }
    // 解析唯一参数。
    parse(values[0], property)
}

// 解析接受一到两个参数的函数。
fn one_or_two(
    // 接收已经切分的参数。
    values: &[&str],
    // 接收函数名。
    name: &str,
    // 接收诊断所属属性。
    property: &StyleProperty,
    // 接收单值解析器。
    parse: fn(&str, &StyleProperty) -> Result<f32, Diagnostic>,
    // 接收单参数时第二轴的默认值。
    second_default: f32,
) -> Result<(f32, f32), Diagnostic> {
    // 只接受一项或两项。
    if !(1..=2).contains(&values.len()) {
        // 返回数量诊断。
        return Err(argument_count_diagnostic(name, "1 或 2", property));
    }
    // 解析第一轴值。
    let first = parse(values[0], property)?;
    // 第二轴存在时解析，否则使用函数指定默认值。
    let second = if values.len() == 2 {
        // 解析显式第二轴值。
        parse(values[1], property)?
    } else {
        // 使用单参数语义默认值。
        second_default
    };
    // 返回两轴值。
    Ok((first, second))
}

// 解析像素或无单位平移长度。
fn parse_length(source: &str, property: &StyleProperty) -> Result<f32, Diagnostic> {
    // 百分比依赖尚未公开的帧尺寸上下文。
    if source.ends_with('%') {
        // 返回明确等价能力差距。
        return Err(transform_diagnostic(
            property,
            "translate 百分比尚无公开运行时等价表示",
            "使用 px 或无单位固定数值",
        ));
    }
    // 去除可选 px 单位。
    let number = source.strip_suffix("px").unwrap_or(source);
    // 解析有限数值。
    parse_finite(number, "translate 长度", property)
}

// 解析无单位缩放因子。
fn parse_scalar(source: &str, property: &StyleProperty) -> Result<f32, Diagnostic> {
    // 缩放因子不接受长度或百分比单位。
    if source.ends_with("px") || source.ends_with('%') {
        // 返回单位诊断。
        return Err(transform_diagnostic(
            property,
            "scale 参数必须是无单位数值",
            "使用如 0.5、1 或 2 的数值",
        ));
    }
    // 解析有限缩放因子。
    parse_finite(source, "scale 因子", property)
}

// 解析 deg、rad、turn 或无单位零角度。
fn parse_angle(source: &str, property: &StyleProperty) -> Result<f32, Diagnostic> {
    // 角度值按单位转换为弧度。
    let radians = if let Some(value) = source.strip_suffix("deg") {
        // 角度转换为弧度。
        parse_finite(value, "角度", property)?.to_radians()
    } else if let Some(value) = source.strip_suffix("rad") {
        // 弧度无需比例换算。
        parse_finite(value, "角度", property)?
    } else if let Some(value) = source.strip_suffix("turn") {
        // 圈数乘以完整圆周弧度。
        parse_finite(value, "角度", property)? * std::f32::consts::TAU
    } else if source == "0" || source == "+0" || source == "-0" {
        // CSS 允许零角度省略单位。
        0.0
    } else {
        // 非零角度必须携带确定单位。
        return Err(transform_diagnostic(
            property,
            "rotate/skew 角度必须使用 deg、rad 或 turn 单位",
            "使用如 45deg、0.5rad 或 0.25turn",
        ));
    };
    // 单位换算后仍必须有限。
    if !radians.is_finite() {
        // 返回非有限角度诊断。
        return Err(transform_diagnostic(
            property,
            "transform 角度必须有限",
            "使用有限的 deg、rad 或 turn 数值",
        ));
    }
    // 返回弧度值。
    Ok(radians)
}

// 解析有限 f32 数值。
fn parse_finite(
    // 接收待解析文本。
    source: &str,
    // 接收诊断中的值类别。
    kind: &str,
    // 接收诊断所属属性。
    property: &StyleProperty,
) -> Result<f32, Diagnostic> {
    // 尝试解析为 f32。
    let value = source.parse::<f32>().map_err(|_| {
        // 返回数值格式诊断。
        transform_diagnostic(
            property,
            format!("{kind} {source:?} 不是有效数值"),
            "使用有限十进制数值",
        )
    })?;
    // 拒绝无穷与 NaN。
    if !value.is_finite() {
        // 返回有限性诊断。
        return Err(transform_diagnostic(
            property,
            format!("{kind} 必须有限"),
            "使用有限十进制数值",
        ));
    }
    // 返回有限 f32。
    Ok(value)
}

// 构造参数数量诊断。
fn argument_count_diagnostic(
    // 接收函数名。
    name: &str,
    // 接收期望数量文本。
    expected: &str,
    // 接收诊断所属属性。
    property: &StyleProperty,
) -> Diagnostic {
    // 返回统一数量错误。
    transform_diagnostic(
        property,
        format!("transform 函数 {name} 需要 {expected} 个参数"),
        "按样式参考调整函数参数数量",
    )
}

// 跳过 ASCII 空白并返回新偏移。
fn skip_ascii_whitespace(source: &str, mut cursor: usize) -> usize {
    // 连续消费空白字节。
    while source
        // 读取源码字节。
        .as_bytes()
        // 安全访问当前偏移。
        .get(cursor)
        // 判断是否为空白。
        .is_some_and(u8::is_ascii_whitespace)
    {
        // 推进一个 ASCII 字节。
        cursor += 1;
    }
    // 返回第一个非空白位置。
    cursor
}

// 返回单位矩阵。
fn identity_matrix() -> Matrix {
    // 使用与运行时 Transform 相同的存储顺序。
    [1.0, 0.0, 0.0, 0.0, 1.0, 0.0]
}

// 返回平移矩阵。
fn translate_matrix(x: f32, y: f32) -> Matrix {
    // 写入两轴平移分量。
    [1.0, 0.0, x, 0.0, 1.0, y]
}

// 返回缩放矩阵。
fn scale_matrix(x: f32, y: f32) -> Matrix {
    // 写入两轴缩放分量。
    [x, 0.0, 0.0, 0.0, y, 0.0]
}

// 返回旋转矩阵。
fn rotate_matrix(angle: f32) -> Matrix {
    // 同时计算正弦与余弦。
    let (sin, cos) = angle.sin_cos();
    // 保持与运行时 Transform::rotate 相同的屏幕坐标语义。
    [cos, -sin, 0.0, sin, cos, 0.0]
}

// 返回双轴倾斜矩阵。
fn skew_matrix(x: f32, y: f32) -> Matrix {
    // 写入两轴切线系数。
    [1.0, x.tan(), 0.0, y.tan(), 1.0, 0.0]
}

// 按 Transform::concat 契约组合两个矩阵。
fn concat_matrix(left: Matrix, right: Matrix) -> Matrix {
    // 解构左侧矩阵。
    let [a, b, tx, c, d, ty] = left;
    // 解构右侧矩阵。
    let [right_a, right_b, right_tx, right_c, right_d, right_ty] = right;
    // 返回 left * right，右侧先作用于点。
    [
        // 第一行第一列。
        a * right_a + b * right_c,
        // 第一行第二列。
        a * right_b + b * right_d,
        // 第一行平移分量。
        a * right_tx + b * right_ty + tx,
        // 第二行第一列。
        c * right_a + d * right_c,
        // 第二行第二列。
        c * right_b + d * right_d,
        // 第二行平移分量。
        c * right_tx + d * right_ty + ty,
    ]
}

// 构造指向完整 transform 值的统一诊断。
fn transform_diagnostic(
    // 接收所属属性。
    property: &StyleProperty,
    // 接收失败原因。
    message: impl Into<String>,
    // 接收修复建议。
    suggestion: impl Into<String>,
) -> Diagnostic {
    // 返回覆盖精确属性值跨度的诊断。
    Diagnostic::new(property.value.span, message, suggestion)
}
