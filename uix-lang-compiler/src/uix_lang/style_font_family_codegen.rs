// 引入卫生标识符、字符串字面量与生成令牌流。
use proc_macro2::{Ident, Literal, TokenStream};
// 引入确定性 Rust 令牌拼接宏。
use quote::quote;

// 引入结构化诊断与样式属性。
use super::{Diagnostic, StyleProperty};

// 把 fontFamily 有序列表映射为显式 UI 字体族字段更新。
pub(super) fn font_family_field(
    // 接收 map_style 闭包中的卫生 Style 标识符。
    style: &Ident,
    // 接收保留源码跨度的 fontFamily 属性。
    property: &StyleProperty,
) -> Result<TokenStream, Diagnostic> {
    // 在宏期解析并验证完整有序字体族列表。
    let families = parse_font_families(property)?;
    // 把每个规范化名称转换为安全 Rust 字符串字面量。
    let families = families
        // 借用解析结果以生成调用参数。
        .iter()
        // 保留字体族声明顺序。
        .map(|family| Literal::string(family))
        // 收集后供 quote 重复展开。
        .collect::<Vec<_>>();
    // Some 区分显式列表与未声明继承值。
    Ok(quote! {
        // 通过公开受控构造器建立非空字体族契约。
        #style.font_family = ::std::option::Option::Some(
            // 运行时公开构造器再次守卫名称不变量。
            ::uix_app::prelude::FontFamily::from_names([#(#families),*])
                // 该分支只由已验证字面量生成。
                .expect("UIX 已验证 fontFamily 为非空合法列表")
        );
    })
}

// 解析逗号分隔的裸名称或单引号名称列表。
fn parse_font_families(property: &StyleProperty) -> Result<Vec<String>, Diagnostic> {
    // 去除属性值外围空白。
    let source = property.value.source.trim();
    // 空列表不能建立字体回退顺序。
    if source.is_empty() {
        // 返回统一值级诊断。
        return Err(font_family_diagnostic(property));
    }
    // 保存当前分量的字节起点。
    let mut start = 0usize;
    // 跟踪单引号名称状态。
    let mut quoted = false;
    // 跟踪单引号名称内的转义状态。
    let mut escaped = false;
    // 保存按源码顺序切分的名称片段。
    let mut items = Vec::new();
    // 扫描 UTF-8 字符并只在引号外识别逗号。
    for (offset, value) in source.char_indices() {
        // 已转义字符没有结构语义。
        if escaped {
            // 清除单字符转义状态。
            escaped = false;
            // 继续扫描后续字符。
            continue;
        }
        // 引号内反斜杠转义下一字符。
        if quoted && value == '\\' {
            // 标记下一字符为字面字符。
            escaped = true;
            // 继续扫描被转义字符。
            continue;
        }
        // 单引号切换名称字符串状态。
        if value == '\'' {
            // 切换是否位于字符串内部。
            quoted = !quoted;
            // 继续扫描当前分量。
            continue;
        }
        // 引号外逗号结束一个字体族分量。
        if value == ',' && !quoted {
            // 保存逗号之前的原始片段。
            items.push(&source[start..offset]);
            // 下一分量从逗号后开始。
            start = offset + value.len_utf8();
        }
    }
    // 未闭合引号或悬空转义不能生成确定名称。
    if quoted || escaped {
        // 返回统一值级诊断。
        return Err(font_family_diagnostic(property));
    }
    // 保存最后一个逗号之后的分量。
    items.push(&source[start..]);
    // 按声明顺序解析和规范化各分量。
    items
        // 消费分量集合避免额外生命周期传播。
        .into_iter()
        // 每个分量独立验证裸名称或单引号名称形态。
        .map(|item| parse_font_family_name(item, property))
        // 任一非法分量都会拒绝完整列表。
        .collect()
}

// 解析单个字体族名称并去除可选单引号与转义。
fn parse_font_family_name(
    // 接收尚未规范化的单个列表分量。
    item: &str,
    // 接收原始属性以生成一致诊断。
    property: &StyleProperty,
) -> Result<String, Diagnostic> {
    // 去除逗号两侧允许的空白。
    let item = item.trim();
    // 空分量会破坏回退顺序身份。
    if item.is_empty() {
        // 返回统一值级诊断。
        return Err(font_family_diagnostic(property));
    }
    // 单引号名称需要去除外围引号并解释反斜杠转义。
    let name = if item.starts_with('\'') {
        // 开头引号必须由末尾引号闭合。
        if item.len() < 2 || !item.ends_with('\'') {
            // 返回统一值级诊断。
            return Err(font_family_diagnostic(property));
        }
        // 取出不含外围引号的 UTF-8 内容。
        let inner = &item[1..item.len() - 1];
        // 保存解码后的字体族名称。
        let mut decoded = String::new();
        // 跟踪当前字符是否由反斜杠转义。
        let mut escaped = false;
        // 逐字符解释单引号名称内容。
        for value in inner.chars() {
            // 转义字符按字面值写入名称。
            if escaped {
                // 保留被转义字符本身。
                decoded.push(value);
                // 清除单字符转义状态。
                escaped = false;
                // 继续扫描后续字符。
                continue;
            }
            // 反斜杠开始转义下一字符。
            if value == '\\' {
                // 标记下一字符为字面字符。
                escaped = true;
                // 暂不写入转义标记。
                continue;
            }
            // 内部未转义单引号表示外围形状不合法。
            if value == '\'' {
                // 返回统一值级诊断。
                return Err(font_family_diagnostic(property));
            }
            // 普通字符直接保留。
            decoded.push(value);
        }
        // 末尾反斜杠不能形成完整转义。
        if escaped {
            // 返回统一值级诊断。
            return Err(font_family_diagnostic(property));
        }
        // 返回解码后的名称候选。
        decoded
    } else {
        // 裸名称中不允许残留结构引号。
        if item.contains('\'') {
            // 返回统一值级诊断。
            return Err(font_family_diagnostic(property));
        }
        // 裸名称按源码文本保留。
        item.to_string()
    };
    // 与运行时构造器保持相同的外围空白规范化。
    let name = name.trim();
    // 空名称或控制字符不能进入字体注册表查询。
    if name.is_empty() || name.chars().any(char::is_control) {
        // 返回统一值级诊断。
        return Err(font_family_diagnostic(property));
    }
    // 返回拥有所有权的规范化字体族名称。
    Ok(name.to_string())
}

// 构造 fontFamily 值级修复性诊断。
fn font_family_diagnostic(
    // 接收原始属性以定位值跨度。
    property: &StyleProperty,
) -> Diagnostic {
    // 返回统一列表形态和可执行示例。
    Diagnostic::new(
        // 精确标记失败值。
        property.value.span,
        // 说明支持的列表边界。
        "fontFamily 必须是逗号分隔的非空字体族名称列表",
        // 给出带空格名称、裸名称与通用族的组合示例。
        "使用 fontFamily: 'Segoe UI', Arial, sans-serif",
    )
}
