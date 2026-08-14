// 引入组件字段 AST、通用元素、表达式解析与诊断。
use super::{
    AttributeValue, ComponentProp, ComponentPropType, ComponentState, ComponentStateInitial,
    ComponentValueType, Diagnostic, Element, RecordDeclaration, RecordField, SourceSpan,
    parse_expression,
};
// 引入名称去重集合。
use std::collections::HashSet;

// 解析逗号分隔的 props 声明。
pub(super) fn parse_props(
    source: &str,
    span: SourceSpan,
) -> Result<Vec<ComponentProp>, Diagnostic> {
    // 空字符串表示没有 props。
    if source.trim().is_empty() {
        // 返回空列表。
        return Ok(Vec::new());
    }
    // 保存有序 props。
    let mut props = Vec::new();
    // 保存名称去重集合。
    let mut names = HashSet::new();
    // 按顶层逗号切分字段，并允许 State<T> 泛型类型。
    for entry in split_top_level(source, span, true)? {
        // 切分字段名与类型。
        let (name, type_source) = split_field(entry, "props", span)?;
        // 验证字段名。
        validate_field_name(name, span)?;
        // 拒绝重复 prop。
        if !names.insert(name) {
            // 返回重复字段诊断。
            return Err(Diagnostic::new(
                // 指向 props 属性。
                span,
                // 说明重复名称。
                format!("prop {name} 重复声明"),
                // 给出修复动作。
                "合并同名 prop 或使用不同名称",
            ));
        }
        // 解析类型白名单。
        let kind = parse_prop_type(type_source, span)?;
        // 保存有序 prop。
        props.push(ComponentProp {
            // 保存名称。
            name: name.to_string(),
            // 保存类型。
            kind,
            // 保存所属属性跨度。
            span,
        });
    }
    // 返回完整 props 列表。
    Ok(props)
}

// 解析逗号分隔的私有 state 声明。
pub(super) fn parse_states(
    // 接收 state 声明源码。
    source: &str,
    // 接收所属属性跨度。
    span: SourceSpan,
) -> Result<Vec<ComponentState>, Diagnostic> {
    // 空字符串表示没有私有状态。
    if source.trim().is_empty() {
        // 返回空列表。
        return Ok(Vec::new());
    }
    // 保存有序状态槽。
    let mut states = Vec::new();
    // 保存名称去重集合。
    let mut names = HashSet::new();
    // 按顶层逗号切分状态，保留初始值中的小于号比较。
    for entry in split_top_level(source, span, false)? {
        // 切分名称与初始值。
        let (name, initial_source) = split_field(entry, "state", span)?;
        // 验证状态名。
        validate_field_name(name, span)?;
        // 拒绝重复状态。
        if !names.insert(name) {
            // 返回重复状态诊断。
            return Err(Diagnostic::new(
                // 指向 state 属性。
                span,
                // 说明重复名称。
                format!("state {name} 重复声明"),
                // 给出修复动作。
                "合并同名 state 或使用不同名称",
            ));
        }
        // 空数组是 state 初始值专用结构。
        let initial = if initial_source == "[]" {
            // 保存空数组事实。
            ComponentStateInitial::EmptyArray
        } else if let Some((value_type, typed_source)) =
            split_typed_state_initial(initial_source, span)?
        {
            // 保存带显式类型注解的初始值表达式。
            ComponentStateInitial::TypedExpression(
                // 保存白名单状态类型。
                value_type,
                // 使用受限表达式语法验证类型化初始值。
                parse_expression(typed_source, span)?,
            )
        } else {
            // 使用受限表达式语法验证普通初始值。
            ComponentStateInitial::Expression(parse_expression(initial_source, span)?)
        };
        // 保存有序状态槽。
        states.push(ComponentState {
            // 保存名称。
            name: name.to_string(),
            // 保存初始值。
            initial,
            // 保存所属属性跨度。
            span,
        });
    }
    // 返回完整状态列表。
    Ok(states)
}

// 解析基础值、State<T> 或回调类型。
fn parse_prop_type(source: &str, span: SourceSpan) -> Result<ComponentPropType, Diagnostic> {
    // 基础类型直接映射。
    if let Some(value) = parse_value_type(source) {
        // 返回基础值 prop。
        return Ok(ComponentPropType::Value(value));
    }
    // 解析 State<T> 共享状态引用。
    if let Some(inner) = source
        // 去除 State< 前缀。
        .strip_prefix("State<")
        // 去除末尾右尖括号。
        .and_then(|value| value.strip_suffix('>'))
    {
        // State 泛型只允许基础类型。
        let value =
            parse_value_type(inner.trim()).ok_or_else(|| invalid_prop_type(source, span))?;
        // 返回响应式状态 prop。
        return Ok(ComponentPropType::State(value));
    }
    // 回调类型必须以左括号开始。
    if let Some(rest) = source.strip_prefix('(') {
        // 查找参数列表的闭合右括号。
        let Some(close) = rest.find(')') else {
            // 返回未闭合回调诊断。
            return Err(invalid_prop_type(source, span));
        };
        // 提取参数类型列表。
        let parameters_source = &rest[..close];
        // 提取可选返回类型后缀。
        let suffix = rest[close + 1..].trim();
        // 解析逗号分隔基础参数类型。
        let parameters = if parameters_source.trim().is_empty() {
            // 空括号表示无参数回调。
            Vec::new()
        } else {
            // 转换每个参数基础类型。
            parameters_source
                // 按逗号切分。
                .split(',')
                // 去除空白并解析白名单。
                .map(|value| {
                    // 返回基础类型或统一诊断。
                    parse_value_type(value.trim()).ok_or_else(|| invalid_prop_type(source, span))
                })
                // 收集或返回首个错误。
                .collect::<Result<Vec<_>, _>>()?
        };
        // 解析可选返回类型。
        let returns = if suffix.is_empty() {
            // 无后缀表示无返回值。
            None
        } else if let Some(value) = suffix.strip_prefix("->") {
            // 箭头后只允许基础返回类型。
            Some(
                // 解析返回类型。
                parse_value_type(value.trim()).ok_or_else(|| invalid_prop_type(source, span))?,
            )
        } else {
            // 其他后缀不是合法回调类型。
            return Err(invalid_prop_type(source, span));
        };
        // 返回回调类型。
        return Ok(ComponentPropType::Callback {
            // 保存参数类型。
            parameters,
            // 保存可选返回类型。
            returns,
        });
    }
    // 其他结构不在类型白名单。
    Err(invalid_prop_type(source, span))
}

// 解析三个基础类型关键字。
fn parse_value_type(source: &str) -> Option<ComponentValueType> {
    // 按精确关键字映射。
    match source {
        // 映射 String。
        "String" => Some(ComponentValueType::String),
        // 映射 number。
        "number" => Some(ComponentValueType::Number),
        // 映射 bool。
        "bool" => Some(ComponentValueType::Bool),
        // 映射可空字符串选择。
        "Option<String>" => Some(ComponentValueType::OptionalString),
        // 其他名称不在白名单。
        _ => None,
    }
}

// 识别 state 声明中可选的 `Type = initial` 类型注解前缀。
// 返回类型与剩余初始值源码；没有独立等号时保持普通表达式语义。
fn split_typed_state_initial(
    source: &str,
    span: SourceSpan,
) -> Result<Option<(ComponentValueType, &str)>, Diagnostic> {
    // 读取 UTF-8 字节以扫描等号位置。
    let bytes = source.as_bytes();
    // 寻找不与相邻等号组成比较运算符的独立分隔等号。
    let separator = (0..bytes.len()).find(|&index| {
        // 当前字符必须是等号。
        bytes[index] == b'='
            // 前一字符不能是等号，排除 ==。
            && index.checked_sub(1).is_none_or(|previous| bytes[previous] != b'=')
            // 后一字符不能是等号，排除 ==。
            && bytes.get(index + 1).is_none_or(|next| *next != b'=')
    });
    // 没有独立等号时保持普通表达式语义。
    let Some(separator) = separator else {
        // 返回未识别注解。
        return Ok(None);
    };
    // 等号左侧必须是普通类型名。
    let type_source = source[..separator].trim();
    // 类型名允许 Ident 或 Ident<Ident> 泛型形状。
    let is_type_source = !type_source.is_empty()
        && type_source
            .chars()
            .enumerate()
            .all(|(position, character)| {
                // 首字符允许字母或下划线。
                if position == 0 {
                    character.is_ascii_alphabetic() || character == '_'
                } else {
                    // 其余字符允许字母数字、下划线或泛型尖括号。
                    character.is_ascii_alphanumeric()
                        || character == '_'
                        || matches!(character, '<' | '>')
                }
            });
    // 左侧不是类型形状时保持普通表达式语义，交给表达式解析器诊断。
    if !is_type_source {
        // 返回未识别注解。
        return Ok(None);
    }
    // 按 state 专用白名单映射类型关键字，未知 PascalCase 名称暂存为 record 引用。
    let value_type = parse_typed_value(type_source).ok_or_else(|| {
        // 构造未知 state 类型诊断。
        Diagnostic::new(
            // 指向 state 声明。
            span,
            // 说明类型不在白名单。
            format!("不支持 state 类型 {type_source:?}"),
            // 给出完整允许集合。
            "使用 String、number、bool、u32、usize、f32、i32、Date、Time、Color、Point、CascaderValue、HashSet<String>、Vec<String>、Vec<UploadFile>、Option<String> 或文档内声明的 record 名",
        )
    })?;
    // 返回类型与等号右侧的初始值源码。
    Ok(Some((value_type, source[separator + 1..].trim())))
}

// 解析 state 注解与 record 字段的类型白名单；未知 PascalCase 名暂存为 record 引用。
pub(crate) fn parse_typed_value(source: &str) -> Option<ComponentValueType> {
    // 按精确关键字映射基础与语义类型。
    let value = match source {
        // 映射拥有所有权的字符串。
        "String" => ComponentValueType::String,
        // 映射 f64 数值。
        "number" => ComponentValueType::Number,
        // 映射布尔值。
        "bool" => ComponentValueType::Bool,
        // 映射无符号计数。
        "u32" => ComponentValueType::U32,
        // 映射索引。
        "usize" => ComponentValueType::USize,
        // 映射单精度浮点。
        "f32" => ComponentValueType::F32,
        // 映射有符号整数。
        "i32" => ComponentValueType::I32,
        // 映射公开日期类型。
        "Date" => ComponentValueType::Date,
        // 映射公开时间类型。
        "Time" => ComponentValueType::Time,
        // 映射公开颜色类型。
        "Color" => ComponentValueType::Color,
        // 映射公开坐标类型。
        "Point" => ComponentValueType::Point,
        // 映射级联路径类型。
        "CascaderValue" => ComponentValueType::CascaderValue,
        // 映射多选集合。
        "HashSet<String>" => ComponentValueType::HashSetOfString,
        // 映射字符串向量。
        "Vec<String>" => ComponentValueType::VecOfString,
        // 映射受控上传文件队列。
        "Vec<UploadFile>" => ComponentValueType::VecOfUploadFile,
        // 映射可空字符串单选。
        "Option<String>" => ComponentValueType::OptionalString,
        // 未知 PascalCase 标识符暂存为 record 引用，由文档级校验兑底。
        _ if is_pascal_identifier(source) => ComponentValueType::Record(source.to_string()),
        // 其余名称不在白名单。
        _ => return None,
    };
    // 返回映射结果。
    Some(value)
}

// 判断名称是否符合 PascalCase 类型名形状。
fn is_pascal_identifier(source: &str) -> bool {
    // 非空且首字符大写字母。
    let mut characters = source.chars();
    // 首字符必须是大写字母。
    let Some(first) = characters.next() else {
        // 空名称不合法。
        return false;
    };
    // 首字符检查。
    if !first.is_ascii_uppercase() {
        // 非大写开头。
        return false;
    }
    // 其余字符允许字母、数字或下划线。
    characters.all(|character| {
        // 逐一验证形状。
        character.is_ascii_alphanumeric() || character == '_'
    })
}

// 把通用顶层 Record 元素验证为结构化 record 声明。
pub(crate) fn parse_record_declaration(element: Element) -> Result<RecordDeclaration, Diagnostic> {
    // 防御性检查调用方只传入保留标签。
    if element.name != "Record" {
        // 返回内部路由诊断。
        return Err(Diagnostic::new(
            // 指向完整元素。
            element.span,
            // 说明元素类型不匹配。
            "record 声明解析器只接受 <Record>",
            // 给出正确路由。
            "把普通元素交给 View 解析器",
        ));
    }
    // Record 不接受子节点。
    if !element.children.is_empty() {
        // 返回 record 形状诊断。
        return Err(Diagnostic::new(
            // 指向完整元素。
            element.span,
            // 说明 record 使用属性声明字段。
            "<Record> 不接受子节点",
            // 给出规范写法。
            "使用 <Record name=\"Profile\" fields=\"email: String, ...\" />",
        ));
    }
    // 保存已出现的属性名。
    let mut attribute_names = HashSet::new();
    // 保存必需 record 名。
    let mut name = None;
    // 保存必需字段声明。
    let mut fields_source = None;
    // 验证 Record 只包含两个声明属性。
    for attribute in &element.attributes {
        // 拒绝重复属性。
        if !attribute_names.insert(attribute.name.as_str()) {
            // 返回重复属性诊断。
            return Err(Diagnostic::new(
                // 指向重复属性。
                attribute.span,
                // 说明重复名称。
                format!("Record 属性 {} 重复声明", attribute.name),
                // 给出修复动作。
                "合并重复属性并只保留一次",
            ));
        }
        // Record 元数据必须使用字符串字面量。
        let AttributeValue::Literal(value) = &attribute.value else {
            // 返回元数据值形状诊断。
            return Err(Diagnostic::new(
                // 指向完整属性。
                attribute.span,
                // 说明不接受运行期表达式。
                format!("Record {} 必须使用字符串字面量", attribute.name),
                // 给出规范形式。
                "使用 name=\"Name\" 或 fields=\"name: Type, ...\"",
            ));
        };
        // 按保留属性名保存源码。
        match attribute.name.as_str() {
            // 保存 record 名。
            "name" => name = Some((value.clone(), attribute.span)),
            // 保存字段声明。
            "fields" => fields_source = Some((value.as_str(), attribute.span)),
            // 其他属性不属于 Record 元数据。
            _ => {
                // 返回未知属性诊断。
                return Err(Diagnostic::new(
                    // 指向完整属性。
                    attribute.span,
                    // 说明未知元数据。
                    format!("Record 不支持属性 {}", attribute.name),
                    // 给出允许集合。
                    "只使用 name 与 fields",
                ));
            }
        }
    }
    // name 是 record 声明的必需属性。
    let Some((name, name_span)) = name else {
        // 返回缺失名称诊断。
        return Err(Diagnostic::new(
            // 指向完整 record。
            element.span,
            // 说明缺少必需名称。
            "<Record> 缺少必需的 name 属性",
            // 给出规范示例。
            "使用 <Record name=\"Profile\" fields=\"...\" />",
        ));
    };
    // record 名必须可映射为 PascalCase Rust 标识符。
    validate_component_name(&name, name_span)?;
    // fields 是 record 声明的必需属性。
    let (fields_source, fields_span) = fields_source.ok_or_else(|| {
        // 返回缺失字段诊断。
        Diagnostic::new(
            // 指向完整 record。
            element.span,
            // 说明缺少字段列表。
            "<Record> 缺少必需的 fields 属性",
            // 给出规范示例。
            "使用 <Record name=\"Profile\" fields=\"email: String\" />",
        )
    })?;
    // 保存有序字段。
    let mut fields = Vec::new();
    // 保存字段名去重集合。
    let mut field_names = HashSet::new();
    // 按顶层逗号切分字段声明。
    for entry in split_top_level(fields_source, fields_span, true)? {
        // 切分字段名与类型。
        let (field_name, type_source) = split_field(entry, "fields", fields_span)?;
        // 验证字段名。
        validate_field_name(field_name, fields_span)?;
        // 拒绝重复字段。
        if !field_names.insert(field_name) {
            // 返回重复字段诊断。
            return Err(Diagnostic::new(
                // 指向 fields 属性。
                fields_span,
                // 说明重复名称。
                format!("record 字段 {field_name} 重复声明"),
                // 给出修复动作。
                "合并同名字段或使用不同名称",
            ));
        }
        // 解析字段类型白名单。
        let kind = parse_typed_value(type_source).ok_or_else(|| {
            // 构造未知字段类型诊断。
            Diagnostic::new(
                // 指向 fields 属性。
                fields_span,
                // 说明类型不在白名单。
                format!("不支持 record 字段类型 {type_source:?}"),
                // 给出允许集合。
                "使用 String、number、bool、u32、usize、f32、i32、Date、Time、Color、Point、CascaderValue、HashSet<String>、Vec<String>、Vec<UploadFile>、Option<String> 或文档内声明的 record 名",
            )
        })?;
        // 保存有序字段。
        fields.push(RecordField {
            // 保存字段名。
            name: field_name.to_string(),
            // 保存字段类型。
            kind,
            // 保存所属声明跨度。
            span: fields_span,
        });
    }
    // 空字段列表不能形成有效 record。
    if fields.is_empty() {
        // 返回空 record 诊断。
        return Err(Diagnostic::new(
            // 指向 fields 属性。
            fields_span,
            // 说明需要至少一个字段。
            "<Record> 的 fields 不能为空",
            // 给出最小修复动作。
            "声明至少一个字段，例如 fields=\"email: String\"",
        ));
    }
    // 返回结构化 record 声明。
    Ok(RecordDeclaration {
        // 保存 record 名。
        name,
        // 保存有序字段。
        fields,
        // 保存完整声明跨度。
        span: element.span,
    })
}

// 构造统一非法 prop 类型诊断。
fn invalid_prop_type(source: &str, span: SourceSpan) -> Diagnostic {
    // 返回白名单诊断。
    Diagnostic::new(
        // 指向 props 属性。
        span,
        // 说明未知类型。
        format!("不支持 prop 类型 {source:?}"),
        // 给出完整允许集合。
        "使用 String、number、bool、State<T> 或基础类型组成的回调签名",
    )
}

// 按不位于括号、方括号、可选尖括号或字符串内的逗号切分。
fn split_top_level(
    source: &str,
    span: SourceSpan,
    track_angle_brackets: bool,
) -> Result<Vec<&str>, Diagnostic> {
    // 保存切分结果。
    let mut entries = Vec::new();
    // 保存当前片段起点。
    let mut start = 0;
    // 保存四类嵌套深度。
    let (mut paren, mut angle, mut bracket, mut brace) = (0_i32, 0_i32, 0_i32, 0_i32);
    // 保存单引号字符串状态。
    let mut quoted = false;
    // 保存反斜杠转义状态。
    let mut escaped = false;
    // 遍历 UTF-8 字符边界。
    for (index, character) in source.char_indices() {
        // 字符串内只处理转义与闭合引号。
        if quoted {
            // 前一字符为反斜杠时消费转义。
            if escaped {
                // 清除转义状态。
                escaped = false;
            } else if character == '\\' {
                // 标记下一字符被转义。
                escaped = true;
            } else if character == '\'' {
                // 结束单引号字符串。
                quoted = false;
            }
            // 字符串内容不参与结构计数。
            continue;
        }
        // 更新结构深度或切分顶层逗号。
        match character {
            // 开始单引号字符串。
            '\'' => quoted = true,
            // 增加圆括号深度。
            '(' => paren += 1,
            // 减少圆括号深度。
            ')' => paren -= 1,
            // props 类型中增加尖括号深度。
            '<' if track_angle_brackets => angle += 1,
            // props 类型中减少尖括号深度。
            '>' if track_angle_brackets && angle > 0 => angle -= 1,
            // 增加方括号深度。
            '[' => bracket += 1,
            // 减少方括号深度。
            ']' => bracket -= 1,
            // 增加花括号深度。
            '{' => brace += 1,
            // 减少花括号深度。
            '}' => brace -= 1,
            // 顶层逗号结束当前字段。
            ',' if paren == 0 && angle == 0 && bracket == 0 && brace == 0 => {
                // 提取并规范化字段。
                let entry = source[start..index].trim();
                // 空字段违反声明语法。
                if entry.is_empty() {
                    // 返回空字段诊断。
                    return Err(Diagnostic::new(
                        // 指向完整属性。
                        span,
                        // 说明多余逗号。
                        "组件字段列表包含空声明",
                        // 给出修复动作。
                        "删除连续逗号或补充字段声明",
                    ));
                }
                // 保存字段。
                entries.push(entry);
                // 下一字段从逗号后开始。
                start = index + character.len_utf8();
            }
            // 其他字符不改变结构。
            _ => {}
        }
        // 任一深度为负表示定界符不匹配。
        if paren < 0 || angle < 0 || bracket < 0 {
            // 返回定界符诊断。
            return Err(invalid_field_list(span));
        }
    }
    // 未闭合字符串或定界符违反语法。
    if quoted || paren != 0 || angle != 0 || bracket != 0 {
        // 返回定界符诊断。
        return Err(invalid_field_list(span));
    }
    // 保存最后一个字段。
    let tail = source[start..].trim();
    // 尾随逗号产生空字段。
    if tail.is_empty() {
        // 返回空尾字段诊断。
        return Err(Diagnostic::new(
            // 指向完整属性。
            span,
            // 说明尾随逗号。
            "组件字段列表不能以逗号结尾",
            // 给出修复动作。
            "删除末尾逗号",
        ));
    }
    // 保存尾字段。
    entries.push(tail);
    // 返回有序字段列表。
    Ok(entries)
}

// 切分单个 name: value 字段。
fn split_field<'a>(
    // 接收完整字段。
    entry: &'a str,
    // 接收字段类别名称。
    kind: &str,
    // 接收诊断跨度。
    span: SourceSpan,
) -> Result<(&'a str, &'a str), Diagnostic> {
    // 第一个冒号分隔名称与其余值。
    let Some((name, value)) = entry.split_once(':') else {
        // 返回缺少冒号诊断。
        return Err(Diagnostic::new(
            // 指向完整属性。
            span,
            // 说明字段结构错误。
            format!("{kind} 字段 {entry:?} 缺少冒号"),
            // 给出规范结构。
            format!(
                "使用 name: {}",
                if kind == "props" {
                    "Type"
                } else {
                    "initialValue"
                }
            ),
        ));
    };
    // 去除名称和值周围空白。
    let (name, value) = (name.trim(), value.trim());
    // 两侧都必须非空。
    if name.is_empty() || value.is_empty() {
        // 返回空名称或值诊断。
        return Err(Diagnostic::new(
            // 指向完整属性。
            span,
            // 说明字段不完整。
            format!("{kind} 字段 {entry:?} 不完整"),
            // 给出规范结构。
            "在冒号两侧补充名称和值",
        ));
    }
    // 返回规范化两部分。
    Ok((name, value))
}

// 验证组件名为 PascalCase Rust 标识符。
pub(super) fn validate_component_name(
    // 接收待验证名称。
    name: &str,
    // 接收名称属性跨度。
    span: SourceSpan,
) -> Result<(), Diagnostic> {
    // 首字符必须是 ASCII 大写且整体是合法字段字符。
    if name.starts_with(|value: char| value.is_ascii_uppercase()) && is_identifier(name) {
        // Component、If 与 For 是保留标签。
        if !matches!(name, "Component" | "If" | "For") {
            // 返回合法。
            return Ok(());
        }
    }
    // 返回组件名称诊断。
    Err(Diagnostic::new(
        // 指向 name 属性。
        span,
        // 说明命名要求。
        format!("组件名 {name:?} 不是可用的 PascalCase 名称"),
        // 给出规范示例。
        "使用非保留 PascalCase 名称，例如 Counter",
    ))
}

// 验证 props/state 字段名为 Rust 兼容标识符。
fn validate_field_name(name: &str, span: SourceSpan) -> Result<(), Diagnostic> {
    // 要求小写或下划线开头并通过 syn 校验。
    if name
        // 读取首字符。
        .chars()
        // 检查 camelCase 或 snake_case 起始形状。
        .next()
        // 返回首字符结果。
        .is_some_and(|value| value.is_ascii_lowercase() || value == '_')
        // 要求其余字符也在语言标识符集合。
        && is_identifier(name)
        // 要求不是 Rust 关键字。
        && syn::parse_str::<syn::Ident>(name).is_ok()
    {
        // 返回合法。
        return Ok(());
    }
    // 返回字段名诊断。
    Err(Diagnostic::new(
        // 指向所属属性。
        span,
        // 说明字段名不可映射。
        format!("组件字段名 {name:?} 不是合法 Rust 标识符"),
        // 给出规范示例。
        "使用 camelCase 或 snake_case 非关键字名称",
    ))
}

// 检查 ASCII 标识符字符集合。
fn is_identifier(name: &str) -> bool {
    // 要求非空且全部字符合法。
    !name.is_empty()
        // 检查全部字符。
        && name
            // 遍历字符。
            .chars()
            // 只允许 ASCII 字母、数字与下划线。
            .all(|value| value.is_ascii_alphanumeric() || value == '_')
}

// 构造字段列表定界符诊断。
fn invalid_field_list(span: SourceSpan) -> Diagnostic {
    // 返回统一结构诊断。
    Diagnostic::new(
        // 指向完整属性。
        span,
        // 说明定界符未配对。
        "组件字段列表包含未配对的括号、尖括号、方括号或引号",
        // 给出修复动作。
        "补齐定界符并确保逗号只分隔顶层字段",
    )
}
