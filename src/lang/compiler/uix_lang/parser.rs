// 元素、属性与文本解析器拆分到独立子模块。
mod control_binding;
// 控制绑定解析器拆分到独立子模块。
mod element_parser;
// 引入核心 AST、词法游标和诊断。
use super::{
    Cursor, Declaration, Diagnostic, Document, Element, SourceSpan, WidgetStateInitial,
    WidgetValueType, parse_at_declaration, parse_record_declaration, parse_style_class,
    parse_visual_declaration, parse_widget_declaration, register_declaration_name,
    starts_record_declaration, starts_visual_declaration, starts_widget_declaration,
};
// 引入拆分后的元素解析入口。
use element_parser::parse_element;
// 测试模块观测解析产出的 AST 值联合。
#[cfg(test)]
use super::{AttributeValue, Node};
// 引入顶层名称去重集合。
use std::collections::HashSet;

// 把普通 UIX 源码解析为具有唯一根元素的核心文档 AST。
pub(crate) fn parse_document(source: &str) -> Result<Document, Diagnostic> {
    parse_document_with_mode(source, false)
}

// 允许 uix_items! 资源只包含 Record 与 Visual 等模块级声明。
pub(crate) fn parse_items_document(source: &str) -> Result<Document, Diagnostic> {
    parse_document_with_mode(source, true)
}

// 共享完整语法解析，仅由入口目标决定声明资源能否省略视图根。
fn parse_document_with_mode(
    source: &str,
    allow_declaration_only: bool,
) -> Result<Document, Diagnostic> {
    // 创建 UTF-8 安全词法游标。
    let mut cursor = Cursor::new(source);
    // 跳过文档起始 trivia。
    cursor.skip_trivia()?;
    // 保存源码顺序中的顶层声明。
    let mut declarations = Vec::new();
    // 保存已声明样式类名。
    let mut style_names = HashSet::new();
    // 保存已声明主题名。
    let mut theme_names = HashSet::new();
    // 保存已声明关键帧名。
    let mut keyframe_names = HashSet::new();
    // 保存已声明组件名。
    let mut widget_names = HashSet::new();
    // 保存已声明 Visual 静态项名。
    let mut visual_names = HashSet::new();
    // 解析根元素之前的声明区。
    loop {
        // @ 前缀开始导入、导出或主题声明。
        let declaration = if cursor.starts_with("@") {
            // 解析 @ 顶层声明。
            Some(parse_at_declaration(&mut cursor)?)
        // 裸标识符开始样式类声明。
        } else if cursor
            // 查看当前字符。
            .peek()
            // 样式类必须以标识符首字符开始。
            .is_some_and(|value| value.is_ascii_alphabetic() || value == '_')
        {
            // 解析具名样式类。
            Some(parse_style_class(&mut cursor)?)
        // 解析并验证顶层 Widget 声明。
        } else if starts_widget_declaration(&cursor) {
            // 先复用通用元素解析器读取组件体。
            let element = parse_element(&mut cursor, true)?;
            // 再验证 Widget 元数据与字段类型。
            Some(Declaration::Widget(parse_widget_declaration(element)?))
        // 解析并验证顶层 Record 声明。
        } else if starts_record_declaration(&cursor) {
            // 先复用通用元素解析器读取自闭合 record 标签。
            let element = parse_element(&mut cursor, true)?;
            // 再验证 Record 元数据与字段类型。
            Some(Declaration::Record(parse_record_declaration(element)?))
        // 解析并验证顶层 Visual 静态记录声明。
        } else if starts_visual_declaration(&cursor) {
            let element = parse_element(&mut cursor, true)?;
            Some(Declaration::Visual(parse_visual_declaration(element)?))
        } else {
            // 当前输入应为文档根元素。
            None
        };
        // 没有声明时结束声明区。
        let Some(declaration) = declaration else {
            // 退出声明循环。
            break;
        };
        // 验证具名声明唯一性。
        register_declaration_name(
            // 传递当前声明。
            &declaration,
            // 传递样式名称集合。
            &mut style_names,
            // 传递主题名称集合。
            &mut theme_names,
            // 传递关键帧名称集合。
            &mut keyframe_names,
            // 传递组件名称集合。
            &mut widget_names,
            &mut visual_names,
        )?;
        // 保存声明顺序。
        declarations.push(declaration);
        // 跳过声明间 trivia。
        cursor.skip_trivia()?;
    }
    // 普通 View/App 文档始终要求显式根元素。
    if cursor.is_eof() {
        if !allow_declaration_only {
            // 返回带修复建议的空文档诊断。
            return Err(Diagnostic::new(
                // 指向文件起点。
                cursor.point_span(),
                // 陈述失败原因。
                "UIX 文档缺少根元素",
                // 给出确定修复动作。
                "添加一个根元素，例如 <App />",
            ));
        }
        // Items 生成器不消费根；用零节点内部占位维持统一 AST/IR 形状。
        let document = Document {
            declarations,
            root: Element {
                name: "KernelView".to_owned(),
                attributes: Vec::new(),
                children: Vec::new(),
                control: None,
                span: cursor.point_span(),
                widget_scopes: Vec::new(),
                for_iteration_clones: Vec::new(),
                for_iteration_setup: Vec::new(),
                for_iteration_outer_captures: Vec::new(),
            reactive_setup: None,
            },
        };
        validate_record_references(&document)?;
        return Ok(document);
    }
    // 解析唯一根元素。
    let root = parse_element(&mut cursor, false)?;
    // 跳过根元素后的 trivia。
    cursor.skip_trivia()?;
    // 任何剩余内容都违反唯一根约束。
    if !cursor.is_eof() {
        // 根元素后的声明违反声明区顺序。
        let declaration_after_root = cursor.starts_with("@")
            // 裸标识符可能开始样式类。
            || cursor
                // 查看尾随首字符。
                .peek()
                // 验证标识符首字符。
                .is_some_and(|value| value.is_ascii_alphabetic() || value == '_')
            // Widget 也属于顶层声明。
            || starts_widget_declaration(&cursor)
            || starts_record_declaration(&cursor)
            || starts_visual_declaration(&cursor);
        // 为声明顺序提供专用诊断。
        if declaration_after_root {
            // 返回声明位置诊断。
            return Err(Diagnostic::new(
                // 指向尾随声明。
                cursor.point_span(),
                // 陈述失败原因。
                "顶层声明必须位于根元素之前",
                // 给出修复建议。
                "把 @import、@export、@theme、样式类、Widget、Record 或 Visual 移到文档开头",
            ));
        }
        // 返回第二根或尾随内容诊断。
        return Err(Diagnostic::new(
            // 指向未消费内容。
            cursor.point_span(),
            // 陈述失败原因。
            "UIX 文档只能包含一个根元素",
            // 给出结构修复建议。
            "把其余元素移动到当前根元素内部",
        ));
    }
    // 构建已验证文档。
    let document = Document {
        // 保存声明顺序。
        declarations,
        // 保存唯一根元素。
        root,
    };
    // 校验全部 record 类型引用都指向已声明 record。
    validate_record_references(&document)?;
    // 返回已验证文档。
    Ok(document)
}

// 校验组件 state 与 record 字段中的 record 引用都存在对应声明。
fn validate_record_references(document: &Document) -> Result<(), Diagnostic> {
    // 收集全部已声明 record 名。
    let record_names = document
        // 遍历声明。
        .declarations
        // 借用声明序列。
        .iter()
        // 只保留 record 声明名。
        .filter_map(|declaration| match declaration {
            // 提取 record 名。
            Declaration::Record(record) => Some(record.name.as_str()),
            // Visual 不参与 Record 类型命名空间。
            Declaration::Visual(_) => None,
            // 其余声明不占用 record 命名空间。
            _ => None,
        })
        // 收集为去重集合。
        .collect::<HashSet<_>>();
    // 逐一声明校验类型引用。
    for declaration in &document.declarations {
        // 按声明类别校验。
        match declaration {
            // 组件私有 state 的类型注解可能引用 record。
            Declaration::Widget(widget) => {
                // 遍历组件 state。
                for state in &widget.states {
                    // 只有类型化初始值带类型引用。
                    if let WidgetStateInitial::TypedExpression(value_type, _) = &state.initial {
                        // 校验类型中的 record 引用。
                        validate_value_type_record(value_type, &record_names, state.span)?;
                    }
                }
            }
            // record 字段可能引用其他 record。
            Declaration::Record(record) => {
                // 遍历 record 字段。
                for field in &record.fields {
                    // 校验字段类型中的 record 引用。
                    validate_value_type_record(&field.kind, &record_names, field.span)?;
                }
            }
            // Visual 字段由 Rust const 类型检查，不引用 UIX Record 类型系统。
            Declaration::Visual(_) => {}
            // 其余声明不含类型引用。
            _ => {}
        }
    }
    // 全部引用已兑底。
    Ok(())
}

// 递归校验单个类型中的 record 引用。
fn validate_value_type_record(
    // 接收待校验类型。
    value_type: &WidgetValueType,
    // 接收已声明 record 名集合。
    record_names: &HashSet<&str>,
    // 接收诊断定位跨度。
    span: SourceSpan,
) -> Result<(), Diagnostic> {
    // 提取直接 record 或 Vec<Record> 的引用名称。
    let name = match value_type {
        // 直接 record 引用。
        WidgetValueType::Record(name) => name,
        // record 集合元素引用。
        WidgetValueType::VecOfRecord(name) => name,
        // 基础类型没有嵌套引用。
        _ => return Ok(()),
    };
    // 名称必须存在对应声明。
    if record_names.contains(name.as_str()) {
        // 引用已兑底。
        return Ok(());
    }
    // 返回未声明 record 诊断。
    Err(Diagnostic::new(
        // 指向类型引用。
        span,
        // 陈述失败原因。
        format!("record 类型 {name} 未在当前文档声明"),
        // 给出修复建议。
        "在顶层声明区添加 <Record name=\"...\" fields=\"...\" />，或改用基础类型",
    ))
}

// 递归解析一个普通或自闭合元素。
#[cfg(test)]
#[path = "../../tests-src/uix_lang/parser_tests.rs"]
mod tests;

