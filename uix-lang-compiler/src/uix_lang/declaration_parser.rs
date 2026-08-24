// 引入顶层声明、共享游标、样式解析与诊断。
use super::{
    Cursor, Declaration, Diagnostic, ExportDeclaration, ImportDeclaration, KeyframeDeclaration,
    KeyframesDeclaration, StyleClassDeclaration, ThemeDeclaration, parse_style_properties,
};
// 引入顶层名称去重集合。
use std::collections::HashSet;

// 判断当前位置是否为保留的顶层 Widget 声明。
pub(crate) fn starts_widget_declaration(cursor: &Cursor<'_>) -> bool {
    // 委托共享标签前缀判断。
    starts_tag_declaration(cursor, "<Widget")
}

// 判断当前位置是否为保留的顶层 Record 声明。
pub(crate) fn starts_record_declaration(cursor: &Cursor<'_>) -> bool {
    // 委托共享标签前缀判断。
    starts_tag_declaration(cursor, "<Record")
}

// 判断当前位置是否为保留的顶层 Visual 声明。
pub(crate) fn starts_visual_declaration(cursor: &Cursor<'_>) -> bool {
    // 委托共享标签前缀判断。
    starts_tag_declaration(cursor, "<Visual")
}

// 验证标签声明前缀后必须出现标签边界。
fn starts_tag_declaration(cursor: &Cursor<'_>, prefix: &str) -> bool {
    // 借用当前位置之后的源码。
    let remaining = &cursor.source()[cursor.offset()..];
    // 要求精确标签前缀。
    let Some(after) = remaining.strip_prefix(prefix) else {
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
    keyframe_names: &mut HashSet<String>,
    widget_names: &mut HashSet<String>,
    visual_names: &mut HashSet<String>,
) -> Result<(), Diagnostic> {
    // 样式类名称不得重复。
    if let Declaration::StyleClass(style) = declaration {
        // 基础类与每个状态变体使用独立的闭合登记键。
        let key = style
            // 借用可选状态。
            .state
            // 拼接规范伪类名称。
            .map(|state| format!("{}:{}", style.name, state.as_str()))
            // 基础类直接使用原名。
            .unwrap_or_else(|| style.name.clone());
        // 首次插入成功时通过。
        if style_names.insert(key) {
            // 返回成功。
            return Ok(());
        }
        // 返回重复样式类诊断。
        return Err(Diagnostic::new(
            // 指向重复声明。
            style.span,
            // 陈述失败原因。
            format!(
                "样式类 {}{} 重复声明",
                style.name,
                style
                    .state
                    .map(|state| format!(":{}", state.as_str()))
                    .unwrap_or_default()
            ),
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
    // 关键帧名称在独立动画命名空间内不得重复。
    if let Declaration::Keyframes(keyframes) = declaration {
        // 首次插入成功时通过。
        if keyframe_names.insert(keyframes.name.clone()) {
            // 返回成功。
            return Ok(());
        }
        // 返回重复关键帧声明诊断。
        return Err(Diagnostic::new(
            // 指向重复声明。
            keyframes.span,
            // 陈述失败原因。
            format!("关键帧 {} 重复声明", keyframes.name),
            // 给出修复建议。
            "合并同名 @keyframes 或使用不同名称",
        ));
    }
    // 自定义组件名称不得重复。
    if let Declaration::Widget(widget) = declaration {
        // 首次插入成功时通过。
        if widget_names.insert(widget.name.clone()) {
            // 返回成功。
            return Ok(());
        }
        // 返回重复组件诊断。
        return Err(Diagnostic::new(
            // 指向重复声明。
            widget.span,
            // 陈述失败原因。
            format!("组件 {} 重复声明", widget.name),
            // 给出修复建议。
            "合并同名组件或使用不同名称",
        ));
    }
    // record 名与组件共享 PascalCase 命名空间。
    if let Declaration::Record(record) = declaration {
        // 首次插入成功时通过。
        if widget_names.insert(record.name.clone()) {
            // 返回成功。
            return Ok(());
        }
        // 返回重复声明诊断。
        return Err(Diagnostic::new(
            // 指向重复声明。
            record.span,
            // 陈述失败原因。
            format!("record 或组件 {} 重复声明", record.name),
            // 给出修复建议。
            "record 与组件共享 PascalCase 命名空间，请使用不同名称",
        ));
    }
    // Visual 静态项在独立模块项目标命名空间中必须唯一。
    if let Declaration::Visual(visual) = declaration {
        if visual_names.insert(visual.name.clone()) {
            return Ok(());
        }
        return Err(Diagnostic::new(
            visual.span,
            format!("Visual 静态项 {} 重复声明", visual.name),
            "合并同名视觉记录或使用不同的 SCREAMING_SNAKE_CASE 名称",
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
            "使用 @import、@export、@theme 或 @keyframes",
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
        // 解析关键帧动画声明。
        "keyframes" => parse_keyframes(cursor, start),
        // 其他 @ 指令不属于当前规范。
        _ => Err(Diagnostic::new(
            // 指向未知名称。
            name_span,
            // 陈述失败原因。
            format!("不支持顶层指令 @{name}"),
            // 给出支持集合。
            "使用 @import、@export、@theme 或 @keyframes",
        )),
    }
}

// 解析具名关键帧动画声明。
fn parse_keyframes(cursor: &mut Cursor<'_>, start: usize) -> Result<Declaration, Diagnostic> {
    // 指令名后允许换行与注释。
    cursor.skip_trivia()?;
    // 读取 animation 属性引用的关键帧名称。
    let (name, _) = cursor.identifier().ok_or_else(|| {
        // 返回缺少名称诊断。
        Diagnostic::new(
            // 指向名称应出现的位置。
            cursor.point_span(),
            // 陈述失败原因。
            "@keyframes 缺少名称",
            // 给出合法示例。
            "使用 @keyframes fade { from { opacity: 0; } to { opacity: 1; } }",
        )
    })?;
    // 名称与外围块之间允许 trivia。
    cursor.skip_trivia()?;
    // 外围左花括号必须显式出现。
    if !cursor.consume("{") {
        // 返回缺少动画块诊断。
        return Err(Diagnostic::new(
            // 指向当前位置。
            cursor.point_span(),
            // 陈述失败原因。
            "@keyframes 声明缺少 {",
            // 给出修复建议。
            "在关键帧名称后添加 { ... }",
        ));
    }
    // 保存解析后的关键帧序列。
    let mut frames = Vec::new();
    // 保存规范化偏移位模式以拒绝 from 与 0% 等语义重复。
    let mut offsets = HashSet::new();
    // 持续解析到外围右花括号。
    loop {
        // 跳过相邻关键帧之间的空白与注释。
        cursor.skip_trivia()?;
        // 外围右花括号结束声明。
        if cursor.consume("}") {
            // 退出关键帧循环。
            break;
        }
        // 输入结束表示外围声明没有闭合。
        if cursor.is_eof() {
            // 返回未闭合声明诊断。
            return Err(Diagnostic::new(
                // 覆盖从指令起点到输入末尾。
                cursor.span_from(start),
                // 陈述失败原因。
                "@keyframes 声明缺少结束花括号",
                // 给出修复建议。
                "在最后一个关键帧之后添加 }",
            ));
        }
        // 保存当前选择器与帧块的起点。
        let frame_start = cursor.offset();
        // 扫描到当前帧样式块的左花括号。
        while !cursor.starts_with("{") {
            // 外围结束或输入结束都表示当前选择器缺少样式块。
            if cursor.is_eof() || cursor.starts_with("}") {
                // 返回缺少帧块诊断。
                return Err(Diagnostic::new(
                    // 指向当前不完整帧。
                    cursor.span_from(frame_start),
                    // 陈述失败原因。
                    "关键帧偏移后缺少样式块",
                    // 给出修复建议。
                    "使用 from { ... }、to { ... } 或 50% { ... }",
                ));
            }
            // 按 UTF-8 字符边界推进选择器扫描。
            cursor.bump();
        }
        // 保存选择器结束位置。
        let selector_end = cursor.offset();
        // 去除选择器外围空白但保留原始诊断跨度。
        let selector = cursor.source()[frame_start..selector_end].trim();
        // 把 from、to 或百分比转换到零到一闭区间。
        let offset_millionths = parse_keyframe_offset(
            // 传递规范化前的选择器文本。
            selector,
            // 传递选择器精确跨度。
            cursor.span_between(frame_start, selector_end),
        )?;
        // 使用共享样式块扫描器读取当前帧内容。
        let (source, _, content_span) = cursor.braced_style_source()?;
        // 复用样式属性语法且禁止 extends。
        let (_, properties) = parse_style_properties(&source, content_span, false)?;
        // 空帧不能形成任何动画值。
        if properties.is_empty() {
            // 返回空帧诊断。
            return Err(Diagnostic::new(
                // 覆盖选择器和空块。
                cursor.span_from(frame_start),
                // 陈述失败原因。
                "关键帧样式块不能为空",
                // 给出修复建议。
                "至少声明一个可动画样式属性，例如 opacity: 1;",
            ));
        }
        // 相同规范化偏移只能声明一次。
        if !offsets.insert(offset_millionths) {
            // 返回重复偏移诊断。
            return Err(Diagnostic::new(
                // 覆盖重复帧。
                cursor.span_from(frame_start),
                // 陈述失败原因。
                format!("关键帧偏移 {selector} 重复声明"),
                // 给出修复建议。
                "合并相同偏移的样式属性",
            ));
        }
        // 保存已经结构化的关键帧。
        frames.push(KeyframeDeclaration {
            // 保存确定性的百万分比时间偏移。
            offset_millionths,
            // 保存当前帧属性。
            properties,
            // 保存完整帧跨度。
            span: cursor.span_from(frame_start),
        });
    }
    // 空关键帧声明不能被 animation 消费。
    if frames.is_empty() {
        // 返回空声明诊断。
        return Err(Diagnostic::new(
            // 覆盖完整关键帧声明。
            cursor.span_from(start),
            // 陈述失败原因。
            "@keyframes 至少需要一个关键帧",
            // 给出修复建议。
            "添加 from、to 或百分比关键帧",
        ));
    }
    // 生成器按偏移顺序构造稳定 typed keyframe 序列。
    frames.sort_by_key(|frame| frame.offset_millionths);
    // 返回具名关键帧声明。
    Ok(Declaration::Keyframes(KeyframesDeclaration {
        // 保存声明名。
        name,
        // 保存有序帧。
        frames,
        // 保存完整声明跨度。
        span: cursor.span_from(start),
    }))
}

// 把关键帧选择器规范化为零到一闭区间偏移。
fn parse_keyframe_offset(selector: &str, span: super::SourceSpan) -> Result<u32, Diagnostic> {
    // from 是零偏移别名。
    if selector == "from" {
        // 返回动画起点。
        return Ok(0);
    }
    // to 是一偏移别名。
    if selector == "to" {
        // 返回动画终点。
        return Ok(1_000_000);
    }
    // 百分比选择器必须以百分号结尾。
    let Some(number) = selector.strip_suffix('%') else {
        // 返回未知选择器诊断。
        return Err(Diagnostic::new(
            // 指向完整选择器。
            span,
            // 陈述失败原因。
            format!("不支持关键帧偏移 {selector}"),
            // 给出合法集合。
            "使用 from、to 或 0% 到 100% 的百分比",
        ));
    };
    // 百分比主体必须是有限数值。
    let value = number.parse::<f32>().ok().filter(|value| value.is_finite());
    // 拒绝缺失、非数值与非有限值。
    let Some(value) = value else {
        // 返回非法百分比诊断。
        return Err(Diagnostic::new(
            // 指向完整选择器。
            span,
            // 陈述失败原因。
            format!("关键帧偏移 {selector} 不是有限百分比"),
            // 给出合法示例。
            "使用 0%、50% 或 100%",
        ));
    };
    // 百分比必须位于闭区间内。
    if !(0.0..=100.0).contains(&value) {
        // 返回越界诊断。
        return Err(Diagnostic::new(
            // 指向完整选择器。
            span,
            // 陈述失败原因。
            format!("关键帧偏移 {selector} 超出 0% 到 100%"),
            // 给出修复建议。
            "把偏移调整到闭区间 0%..=100%",
        ));
    }
    // 把百分比确定性量化为百万分比，避免浮点值破坏 AST 的 Eq 契约。
    let millionths = (value * 10_000.0).round() as u32;
    // 返回零到一百万闭区间偏移。
    Ok(millionths)
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
    // 可选冒号开始状态伪类名称。
    let state = if cursor.consume(":") {
        // 冒号后必须紧跟规范状态名称。
        let (state, span) = cursor.identifier().ok_or_else(|| {
            // 返回缺失状态诊断。
            Diagnostic::new(
                cursor.point_span(),
                "样式伪类缺少状态名称",
                "使用 :hover、:disabled 或 :checked",
            )
        })?;
        // 把闭合状态名称映射到 AST。
        Some(match state.as_str() {
            // 登记悬停状态。
            "hover" => super::StylePseudoState::Hover,
            // 登记禁用状态。
            "disabled" => super::StylePseudoState::Disabled,
            // 登记勾选状态。
            "checked" => super::StylePseudoState::Checked,
            // 拒绝未批准状态。
            _ => {
                return Err(Diagnostic::new(
                    span,
                    format!("不支持样式伪类 :{state}"),
                    "使用 :hover、:disabled 或 :checked",
                ));
            }
        })
    } else {
        // 没有冒号表示普通基础类。
        None
    };
    // 跳过名称后 trivia。
    cursor.skip_trivia()?;
    // 提取块源码和内容位置。
    let (source, _, content_span) = cursor.braced_style_source()?;
    // 使用共享样式语法并允许 extends。
    let (extends, properties) = parse_style_properties(&source, content_span, true)?;
    // 状态变体固定隐含继承同前缀基础类，不接受其他父类。
    if state.is_some() && extends.is_some() {
        // 返回伪类继承诊断。
        return Err(Diagnostic::new(
            cursor.span_from(start),
            "状态伪类不能显式声明 extends",
            "删除 extends；状态变体会自动叠加到同前缀基础类",
        ));
    }
    // 返回样式类声明。
    Ok(Declaration::StyleClass(StyleClassDeclaration {
        // 保存名称。
        name,
        // 保存可选状态伪类。
        state,
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
            "使用 @import('./file.uix') 或 @import('./file.uix', 'Widget')",
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
            "使用 @import('./widget.uix')",
        ));
    }
    // 提取可选组件名。
    let widget = arguments.get(1).cloned();
    // 具名导入必须使用 PascalCase 组件名。
    if widget
        // 借用可选名称。
        .as_deref()
        // 验证组件名。
        .is_some_and(|value| !is_widget_name(value))
    {
        // 返回组件名诊断。
        return Err(Diagnostic::new(
            // 覆盖完整指令。
            cursor.span_from(start),
            // 陈述失败原因。
            "@import 的组件名必须使用 PascalCase",
            // 给出合法示例。
            "使用 @import('./file.uix', 'MyWidget')",
        ));
    }
    // 返回导入声明。
    Ok(Declaration::Import(ImportDeclaration {
        // 保存路径。
        path,
        // 保存可选组件。
        widget,
        // 保存完整跨度。
        span: cursor.span_from(start),
    }))
}

// 解析 @export 指令。
fn parse_export(cursor: &mut Cursor<'_>, start: usize) -> Result<Declaration, Diagnostic> {
    // 读取一个或多个组件名。
    let widgets = parse_string_arguments(cursor, "@export")?;
    // 导出列表不能为空。
    if widgets.is_empty() {
        // 返回空列表诊断。
        return Err(Diagnostic::new(
            // 覆盖完整指令。
            cursor.span_from(start),
            // 陈述失败原因。
            "@export 至少需要一个组件名",
            // 给出合法示例。
            "使用 @export('WidgetName')",
        ));
    }
    // 验证名称与列表内唯一性。
    for (index, widget) in widgets.iter().enumerate() {
        // 每个导出名必须使用 PascalCase。
        if !is_widget_name(widget) {
            // 返回名称诊断。
            return Err(Diagnostic::new(
                // 覆盖完整指令。
                cursor.span_from(start),
                // 陈述失败原因。
                format!("导出组件名 {widget} 必须使用 PascalCase"),
                // 给出合法示例。
                "使用 @export('MyWidget')",
            ));
        }
        // 后续列表不能重复当前名称。
        if widgets[index + 1..].contains(widget) {
            // 返回重复名称诊断。
            return Err(Diagnostic::new(
                // 覆盖完整指令。
                cursor.span_from(start),
                // 陈述失败原因。
                format!("@export 重复列出组件 {widget}"),
                // 给出修复建议。
                "每个组件只导出一次",
            ));
        }
    }
    // 返回导出声明。
    Ok(Declaration::Export(ExportDeclaration {
        // 保存有序组件列表。
        widgets,
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
fn is_widget_name(value: &str) -> bool {
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
