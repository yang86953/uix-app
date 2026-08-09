// 引入顶层声明、共享游标、样式解析与诊断。
use super::{
    parse_style_properties, Cursor, Declaration, Diagnostic, ExportDeclaration, ImportDeclaration,
    StyleClassDeclaration, ThemeDeclaration,
};
// 引入顶层名称去重集合。
use std::collections::HashSet;

// 判断当前位置是否为保留的顶层 Component 声明。
pub(crate) fn starts_component_declaration(cursor: &Cursor<'_>) -> bool {
    // 借用当前位置之后的源码。
    let remaining = &cursor.source()[cursor.offset()..];
    // 要求精确 Component 标签前缀。
    let Some(after) = remaining.strip_prefix("<Component") else {
        // 前缀不匹配。
        return false;
    };
    // 后续必须是标签边界而非更长名称。
    after
        // 查看边界字符。
        .chars()
        // 接受空白、斜杠或右尖括号。
        .next()
        // 返回边界检查结果。
        .is_some_and(|value| value.is_whitespace() || matches!(value, '/' | '>'))
}

// 验证样式类和主题名称在各自命名空间唯一。
pub(crate) fn register_declaration_name(
    declaration: &Declaration,
    style_names: &mut HashSet<String>,
    theme_names: &mut HashSet<String>,
    component_names: &mut HashSet<String>,
) -> Result<(), Diagnostic> {
    // 样式类名称不得重复。
    if let Declaration::StyleClass(style) = declaration {
        // 首次插入成功时通过。
        if style_names.insert(style.name.clone()) {
            // 返回成功。
            return Ok(());
        }
        // 返回重复样式类诊断。
        return Err(Diagnostic::new(
            // 指向重复声明。
            style.span,
            // 陈述失败原因。
            format!("样式类 {} 重复声明", style.name),
            // 给出修复建议。
            "合并同名样式类或使用不同名称",
        ));
    }
    // 主题名称不得重复。
    if let Declaration::Theme(theme) = declaration {
        // 首次插入成功时通过。
        if theme_names.insert(theme.name.clone()) {
            // 返回成功。
            return Ok(());
        }
        // 返回重复主题诊断。
        return Err(Diagnostic::new(
            // 指向重复声明。
            theme.span,
            // 陈述失败原因。
            format!("主题 {} 重复声明", theme.name),
            // 给出修复建议。
            "合并同名主题或使用不同名称",
        ));
    }
    // 自定义组件名称不得重复。
    if let Declaration::Component(component) = declaration {
        // 首次插入成功时通过。
        if component_names.insert(component.name.clone()) {
            // 返回成功。
            return Ok(());
        }
        // 返回重复组件诊断。
        return Err(Diagnostic::new(
            // 指向重复声明。
            component.span,
            // 陈述失败原因。
            format!("组件 {} 重复声明", component.name),
            // 给出修复建议。
            "合并同名组件或使用不同名称",
        ));
    }
    // 其他声明不占用样式或主题命名空间。
    Ok(())
}

// 解析以 @ 开头的顶层指令。
pub(crate) fn parse_at_declaration(cursor: &mut Cursor<'_>) -> Result<Declaration, Diagnostic> {
    // 保存完整指令起点。
    let start = cursor.offset();
    // 消费 @ 前缀。
    cursor.consume("@");
    // 读取指令名。
    let (name, name_span) = cursor.identifier().ok_or_else(|| {
        // 返回缺失指令名诊断。
        Diagnostic::new(
            // 指向 @ 后位置。
            cursor.point_span(),
            // 陈述失败原因。
            "顶层指令缺少名称",
            // 给出支持指令。
            "使用 @import、@export 或 @theme",
        )
    })?;
    // 按指令名分派解析。
    match name.as_str() {
        // 解析导入指令。
        "import" => parse_import(cursor, start),
        // 解析导出指令。
        "export" => parse_export(cursor, start),
        // 解析主题声明。
        "theme" => parse_theme(cursor, start),
        // 其他 @ 指令不属于当前规范。
        _ => Err(Diagnostic::new(
            // 指向未知名称。
            name_span,
            // 陈述失败原因。
            format!("不支持顶层指令 @{name}"),
            // 给出支持集合。
            "使用 @import、@export 或 @theme",
        )),
    }
}

// 解析具名样式类声明。
pub(crate) fn parse_style_class(cursor: &mut Cursor<'_>) -> Result<Declaration, Diagnostic> {
    // 保存完整声明起点。
    let start = cursor.offset();
    // 读取样式类名。
    let (name, _) = cursor.identifier().ok_or_else(|| {
        // 返回缺失名称诊断。
        Diagnostic::new(
            // 指向当前位置。
            cursor.point_span(),
            // 陈述失败原因。
            "样式类缺少合法名称",
            // 给出合法示例。
            "使用 baseButton { color: red; }",
        )
    })?;
    // 跳过名称后 trivia。
    cursor.skip_trivia()?;
    // 提取块源码和内容位置。
    let (source, _, content_span) = cursor.braced_style_source()?;
    // 使用共享样式语法并允许 extends。
    let (extends, properties) = parse_style_properties(&source, content_span, true)?;
    // 返回样式类声明。
    Ok(Declaration::StyleClass(StyleClassDeclaration {
        // 保存名称。
        name,
        // 保存继承目标。
        extends,
        // 保存有序属性。
        properties,
        // 保存完整声明跨度。
        span: cursor.span_from(start),
    }))
}

// 解析 @import 指令。
fn parse_import(cursor: &mut Cursor<'_>, start: usize) -> Result<Declaration, Diagnostic> {
    // 读取一个路径和可选组件名。
    let arguments = parse_string_arguments(cursor, "@import")?;
    // 导入只允许一到两个参数。
    if !(1..=2).contains(&arguments.len()) {
        // 返回参数数量诊断。
        return Err(Diagnostic::new(
            // 覆盖完整指令。
            cursor.span_from(start),
            // 陈述失败原因。
            "@import 只接受路径和可选组件名",
            // 给出合法示例。
            "使用 @import('./file.uix') 或 @import('./file.uix', 'Component')",
        ));
    }
    // 取得路径参数。
    let path = arguments[0].clone();
    // 导入路径必须指向 .uix 文件。
    if !path.ends_with(".uix") {
        // 返回路径后缀诊断。
        return Err(Diagnostic::new(
            // 覆盖完整指令。
            cursor.span_from(start),
            // 陈述失败原因。
            "@import 路径必须指向 .uix 文件",
            // 给出合法示例。
            "使用 @import('./component.uix')",
        ));
    }
    // 提取可选组件名。
    let component = arguments.get(1).cloned();
    // 具名导入必须使用 PascalCase 组件名。
    if component
        // 借用可选名称。
        .as_deref()
        // 验证组件名。
        .is_some_and(|value| !is_component_name(value))
    {
        // 返回组件名诊断。
        return Err(Diagnostic::new(
            // 覆盖完整指令。
            cursor.span_from(start),
            // 陈述失败原因。
            "@import 的组件名必须使用 PascalCase",
            // 给出合法示例。
            "使用 @import('./file.uix', 'MyComponent')",
        ));
    }
    // 返回导入声明。
    Ok(Declaration::Import(ImportDeclaration {
        // 保存路径。
        path,
        // 保存可选组件。
        component,
        // 保存完整跨度。
        span: cursor.span_from(start),
    }))
}

// 解析 @export 指令。
fn parse_export(cursor: &mut Cursor<'_>, start: usize) -> Result<Declaration, Diagnostic> {
    // 读取一个或多个组件名。
    let components = parse_string_arguments(cursor, "@export")?;
    // 导出列表不能为空。
    if components.is_empty() {
        // 返回空列表诊断。
        return Err(Diagnostic::new(
            // 覆盖完整指令。
            cursor.span_from(start),
            // 陈述失败原因。
            "@export 至少需要一个组件名",
            // 给出合法示例。
            "使用 @export('ComponentName')",
        ));
    }
    // 验证名称与列表内唯一性。
    for (index, component) in components.iter().enumerate() {
        // 每个导出名必须使用 PascalCase。
        if !is_component_name(component) {
            // 返回名称诊断。
            return Err(Diagnostic::new(
                // 覆盖完整指令。
                cursor.span_from(start),
                // 陈述失败原因。
                format!("导出组件名 {component} 必须使用 PascalCase"),
                // 给出合法示例。
                "使用 @export('MyComponent')",
            ));
        }
        // 后续列表不能重复当前名称。
        if components[index + 1..].contains(component) {
            // 返回重复名称诊断。
            return Err(Diagnostic::new(
                // 覆盖完整指令。
                cursor.span_from(start),
                // 陈述失败原因。
                format!("@export 重复列出组件 {component}"),
                // 给出修复建议。
                "每个组件只导出一次",
            ));
        }
    }
    // 返回导出声明。
    Ok(Declaration::Export(ExportDeclaration {
        // 保存有序组件列表。
        components,
        // 保存完整跨度。
        span: cursor.span_from(start),
    }))
}

// 解析 @theme 声明。
fn parse_theme(cursor: &mut Cursor<'_>, start: usize) -> Result<Declaration, Diagnostic> {
    // 跳过指令名后 trivia。
    cursor.skip_trivia()?;
    // 读取主题名。
    let (name, _) = cursor.identifier().ok_or_else(|| {
        // 返回缺失主题名诊断。
        Diagnostic::new(
            // 指向当前位置。
            cursor.point_span(),
            // 陈述失败原因。
            "@theme 缺少主题名",
            // 给出合法示例。
            "使用 @theme light { primaryColor: #2196F3; }",
        )
    })?;
    // 跳过主题名后 trivia。
    cursor.skip_trivia()?;
    // 提取主题块源码和内容位置。
    let (source, _, content_span) = cursor.braced_style_source()?;
    // 使用共享样式语法但禁止 extends。
    let (_, properties) = parse_style_properties(&source, content_span, false)?;
    // 返回主题声明。
    Ok(Declaration::Theme(ThemeDeclaration {
        // 保存主题名。
        name,
        // 保存有序主题属性。
        properties,
        // 保存完整跨度。
        span: cursor.span_from(start),
    }))
}

// 解析圆括号内逗号分隔的单引号字符串。
fn parse_string_arguments(
    cursor: &mut Cursor<'_>,
    directive: &str,
) -> Result<Vec<String>, Diagnostic> {
    // 跳过指令名后 trivia。
    cursor.skip_trivia()?;
    // 指令必须使用圆括号。
    if !cursor.consume("(") {
        // 返回缺少左括号诊断。
        return Err(Diagnostic::new(
            // 指向当前位置。
            cursor.point_span(),
            // 陈述失败原因。
            format!("{directive} 缺少 ("),
            // 给出修复建议。
            format!("使用 {directive}('value')"),
        ));
    }
    // 保存参数列表。
    let mut arguments = Vec::new();
    // 跳过左括号后 trivia。
    cursor.skip_trivia()?;
    // 空列表直接允许调用方生成专用诊断。
    if cursor.consume(")") {
        // 返回空参数列表。
        return Ok(arguments);
    }
    // 解析逗号分隔参数。
    loop {
        // 读取单引号字符串。
        arguments.push(cursor.single_quoted_literal()?);
        // 跳过参数后 trivia。
        cursor.skip_trivia()?;
        // 右括号结束参数列表。
        if cursor.consume(")") {
            // 返回完整列表。
            return Ok(arguments);
        }
        // 参数之间必须有逗号。
        if !cursor.consume(",") {
            // 返回缺少分隔符诊断。
            return Err(Diagnostic::new(
                // 指向当前位置。
                cursor.point_span(),
                // 陈述失败原因。
                format!("{directive} 参数之间缺少逗号或右括号"),
                // 给出修复建议。
                "使用逗号分隔参数并以 ) 结束",
            ));
        }
        // 跳过逗号后 trivia。
        cursor.skip_trivia()?;
        // 尾随逗号不属于规范语法。
        if cursor.starts_with(")") {
            // 返回尾随逗号诊断。
            return Err(Diagnostic::new(
                // 指向右括号。
                cursor.point_span(),
                // 陈述失败原因。
                format!("{directive} 不允许尾随逗号"),
                // 给出修复建议。
                "删除最后一个逗号",
            ));
        }
    }
}

// 验证 PascalCase 组件名。
fn is_component_name(value: &str) -> bool {
    // 首字符必须是 ASCII 大写字母。
    value
        // 读取首字符。
        .chars()
        // 验证 PascalCase 起始。
        .next()
        // 返回检查结果。
        .is_some_and(|first| first.is_ascii_uppercase())
        // 后续必须是规范标识符字符。
        && value
            // 遍历全部字符。
            .chars()
            // 验证字符集合。
            .all(|item| item.is_ascii_alphanumeric() || matches!(item, '_' | '$'))
}
