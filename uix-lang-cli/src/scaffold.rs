//! `uix scaffold visual` Adapter：从同模块 Rust 结构体生成 `<Visual>` 声明骨架。
//!
//! Rust 结构体是视觉字段的唯一类型事实；`.uix` 是唯一取值事实。
//! 本命令消除双侧手写字段的搬运成本，不改变该治理原则。

use std::path::Path;

/// 表示一个待生成字段的占位结果。
struct FieldTemplate {
    /// 已确定初始值的属性写入片段（含 `name=` 前缀）；None 代表需要人工补齐。
    assignment: Option<String>,
    /// 未识别类型的 Rust 类型文本，供 TODO 注释输出。
    unmatched_type: Option<String>,
}

/// 读取 Rust 文件并生成目标结构体的 `<Visual>` 骨架文本。
pub(crate) fn generate_visual_scaffold(
    rust_file: &Path,
    struct_name: &str,
) -> Result<String, String> {
    // 读取 Rust 源文件。
    let source = std::fs::read_to_string(rust_file)
        .map_err(|error| format!("无法读取 {}: {error}", rust_file.display()))?;
    // 解析为 syn AST。
    let syntax = syn::parse_file(&source)
        .map_err(|error| format!("{} 不是合法 Rust 文件：{error}", rust_file.display()))?;
    // 定位具名结构体。
    let item = syntax
        .items
        .iter()
        .find(|item| {
            matches!(
                item,
                syn::Item::Struct(value)
                    if value.ident == struct_name
            )
        })
        .ok_or_else(|| format!("{} 中未找到结构体 {struct_name}", rust_file.display()))?;
    // 提取命名字段集合；只接受命名字段结构体。
    let syn::Item::Struct(item_struct) = item else {
        unreachable!("匹配已保证为结构体");
    };
    // 收集源码顺序中的字段名与类型文本。
    let mut fields: Vec<(String, String)> = Vec::new();
    // 按 syn 字段容器形状分派。
    match &item_struct.fields {
        // 命名字段才符合 Visual 形状。
        syn::Fields::Named(named) => {
            // 逐字段提取。
            for field in &named.named {
                // 无名（元组/单元）字段直接拒绝。
                let Some(name) = field.ident.as_ref() else {
                    // 返回形状诊断。
                    return Err(format!(
                        "结构体 {struct_name} 含无名字段，不符合 Visual 形状"
                    ));
                };
                // 保存 snake_case 名与类型文本。
                fields.push((name.to_string(), quote_type(&field.ty)));
            }
        }
        // 元组或单元结构体不符合 Visual 形状。
        _ => {
            // 返回形状诊断。
            return Err(format!("结构体 {struct_name} 不是命名字段结构体"));
        }
    }
    // 空结构体没有可生成的骨架。
    if fields.is_empty() {
        return Err(format!("结构体 {struct_name} 没有字段"));
    }
    // 按 Visual 名称规范推导默认常量名。
    let default_visual = derive_visual_name(struct_name);
    // 逐字段生成模板。
    let templates = fields
        .iter()
        .map(|(field, field_type)| field_template(field, field_type))
        .collect::<Vec<_>>();
    // 组装骨架文本。
    Ok(render(&default_visual, struct_name, &fields, &templates))
}

/// 把 snake_case 字段名转换为 Visual 属性使用的 camelCase。
fn camel_case(snake: &str) -> String {
    // 按下划线切段后重组驼峰。
    let mut output = String::with_capacity(snake.len());
    // 标记下一字符是否需要大写。
    let mut capitalize = false;
    // 逐字符处理。
    for character in snake.chars() {
        // 下划线触发大写标记。
        if character == '_' {
            // 打开大写标记。
            capitalize = true;
        // 首段保持小写，后续段落首字母大写。
        } else if capitalize {
            // 写入大写形式。
            output.extend(character.to_uppercase());
            // 关闭标记。
            capitalize = false;
        // 其余字符原样保留。
        } else {
            // 原样写入。
            output.push(character);
        }
    }
    // 返回驼峰名称。
    output
}

/// 提取类型的规范文本表示。
fn quote_type(ty: &syn::Type) -> String {
    // 借助 ToTokens 拿到源码文本，规范化空白便于闭合匹配。
    use ::quote::ToTokens as _;
    ty.to_token_stream().to_string().replace(' ', "")
}

// 按类型给出字段模板。
fn field_template(field: &str, field_type: &str) -> FieldTemplate {
    // 属性名为驼峰形式。
    let attribute = camel_case(field);
    // 浮点类型给 0.0；整型给 0；布尔给 false；字符串用双引号字面量；其余交给作者补齐。
    let assignment = match field_type {
        "f32" | "f64" => Some(format!("{attribute}={{0.0}}")),
        "usize" | "u8" | "u16" | "u32" | "u64" | "i8" | "i16" | "i32" | "i64" | "isize" => {
            Some(format!("{attribute}={{0}}"))
        }
        "bool" => Some(format!("{attribute}={{false}}")),
        "String" => Some(format!("{attribute}=\"标签\"")),
        // 其他类型交由作者补齐。
        _ => None,
    };
    // 先确定是否未命中已知类型，避免移动后读取。
    let unmatched_type = assignment.is_none().then(|| field_type.to_string());
    // 组装模板条目。
    FieldTemplate {
        // 保存已确定的赋值或 None。
        assignment,
        // 记录未识别类型。
        unmatched_type,
    }
}

// 组装最终骨架文本。
fn render(
    visual_name: &str,
    struct_name: &str,
    fields: &[(String, String)],
    templates: &[FieldTemplate],
) -> String {
    // 收集已确定赋值的行。
    let mut output = String::new();
    // 记录需要人工补齐的字段下标。
    let mut pending = Vec::new();
    // 打开声明。
    output.push_str("<Visual\n");
    // 写入常量名与类型名。
    output.push_str(&format!("  name=\"{visual_name}\"\n"));
    // type 指向同模块结构体名。
    output.push_str(&format!("  type=\"{struct_name}\"\n"));
    // 按源码顺序写入口径一致的字段行。
    for (index, template) in templates.iter().enumerate() {
        // 已确定初值时直接输出属性行。
        if let Some(assignment) = &template.assignment {
            // 追加缩进行。
            output.push_str(&format!("  {assignment}\n"));
        } else {
            // 记录待补齐字段下标。
            pending.push(index);
        }
    }
    // 关闭声明。
    output.push_str("/>\n");
    // 存在未识别字段时输出 TODO 注释，保持骨架其余部分可直接编译。
    if !pending.is_empty() {
        // 分隔注释块。
        output.push_str("// TODO: 以下字段类型无法自动给初值，请补齐后并入上方 <Visual>：\n");
        // 按记录的下标逐条列出来源字段与类型。
        for index in pending {
            // 读取字段名（snake_case 来源便于对照 Rust 定义）。
            let (field, _) = &fields[index];
            // 读取未识别类型文本。
            let field_type = templates[index]
                .unmatched_type
                .as_deref()
                .unwrap_or("未知类型");
            // 输出补齐提示行。
            output.push_str(&format!(
                "//   {field}: {field_type} → {}={{ /* 表达式或具名 Visual 引用 */ }}\n",
                camel_case(field)
            ));
        }
    }
    // 返回骨架文本。
    output
}

// 由结构体名推导 SCREAMING_SNAKE_CASE Visual 常量名。
fn derive_visual_name(struct_name: &str) -> String {
    // 在小写字母前的大写边界与字母→数字处插入下划线。
    let mut output = String::new();
    // 保留前序字符判断。
    let mut previous_uppercase = true;
    // 遍历名称字符。
    for character in struct_name.chars() {
        // 大写且前一段不是大写时插入分隔。
        if character.is_ascii_uppercase() && !previous_uppercase {
            // 插入下划线分隔。
            output.push('_');
        }
        // 更新前序状态。
        previous_uppercase = character.is_ascii_uppercase();
        // 统一大写写入。
        output.extend(character.to_uppercase());
    }
    // 返回常量名。
    output
}

// 用纯函数级测试锁定骨架输出：占位初值、TODO 提示与形状诊断不回退。
#[cfg(test)]
mod tests {
    use super::{camel_case, derive_visual_name, generate_visual_scaffold};
    use std::path::PathBuf;

    // 把结构体源码写入按名称隔离的临时文件并返回路径，避免并行测试互踩。
    fn write_struct_source(struct_name: &str, definition: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("uix-scaffold-{struct_name}.rs"));
        std::fs::write(&path, format!("// 临时测试源。\n{definition}\n"))
            .expect("临时 Rust 源必须可写");
        path
    }

    #[test]
    fn known_scalar_fields_get_literal_placeholders() {
        let definition = r#"
pub struct IconVisual {
    default_size: f32,
    glyph_scale: f64,
    glyph_count: u32,
    visible: bool,
    label: String,
}
"#;
        let path = write_struct_source("IconVisualKnown", definition);
        let scaffold = generate_visual_scaffold(&path, "IconVisual").expect("已知标量结构必须成功");
        std::fs::remove_file(&path).ok();

        assert!(
            scaffold.contains("name=\"ICON_VISUAL\""),
            "常量名推导错误：{scaffold}"
        );
        assert!(scaffold.contains("type=\"IconVisual\""));
        assert!(
            scaffold.contains("defaultSize={0.0}"),
            "浮点占位缺失：{scaffold}"
        );
        assert!(scaffold.contains("glyphScale={0.0}"));
        assert!(scaffold.contains("glyphCount={0}"));
        assert!(scaffold.contains("visible={false}"));
        assert!(scaffold.contains("label=\"标签\""));
        assert!(
            !scaffold.contains("TODO"),
            "全部字段可给初值时不得出现补齐提示：{scaffold}"
        );
    }

    #[test]
    fn unknown_fields_land_in_todo_comment() {
        let definition = r#"
pub struct MixedVisual {
    ratio: f64,
    color: ColorValue,
}
"#;
        let path = write_struct_source("MixedVisualUnknown", definition);
        let scaffold = generate_visual_scaffold(&path, "MixedVisual").expect("混合结构必须成功");
        std::fs::remove_file(&path).ok();

        // 已知类型保持可直接编译的属性行，未识别类型只进入注释提示。
        assert!(scaffold.contains("ratio={0.0}"));
        assert!(
            !scaffold.contains("\n  color="),
            "未识别类型不得出现在属性行：{scaffold}"
        );
        assert!(
            scaffold.contains("TODO: 以下字段类型无法自动给初值"),
            "{scaffold}"
        );
        assert!(
            scaffold.contains("//   color: ColorValue → color="),
            "{scaffold}"
        );
    }

    #[test]
    fn missing_struct_reports_targeted_error() {
        let definition = r#"
pub struct Unrelated {
    value: f32,
}
"#;
        let path = write_struct_source("MissingStructProbe", definition);
        let error = generate_visual_scaffold(&path, "AbsentVisual")
            .expect_err("缺失目标结构体必须定向失败");
        std::fs::remove_file(&path).ok();

        assert!(error.contains("未找到结构体 AbsentVisual"), "{error}");
    }

    #[test]
    fn tuple_struct_is_rejected() {
        let definition = "pub struct Pair(u32, f32);\n";
        let path = write_struct_source("TupleStructProbe", definition);
        let error =
            generate_visual_scaffold(&path, "Pair").expect_err("元组结构体不符合 Visual 形状");
        std::fs::remove_file(&path).ok();

        assert!(error.contains("不是命名字段结构体"), "{error}");
    }

    #[test]
    fn helper_naming_conversions_stay_stable() {
        assert_eq!(camel_case("default_size"), "defaultSize");
        assert_eq!(camel_case("label"), "label");
        assert_eq!(derive_visual_name("IconVisual"), "ICON_VISUAL");
        assert_eq!(derive_visual_name("ProfileCard"), "PROFILE_CARD");
    }
}
