// 引入过程宏标识符与令牌流。
use proc_macro2::{Ident, TokenStream};
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入组件展开器。
use super::widget_codegen::WidgetExpander;
// 引入类型、表达式、对象字段与生成辅助。
use super::{
    Diagnostic, Expression, ExpressionKind, ObjectField, WidgetValueType, generate_expression,
    rust_identifier, value_type_tokens,
};

// 实现需要递归目标类型信息的复合初始值生成。
impl WidgetExpander {
    // 生成 Option、集合、CascaderValue 或 record 的类型化初始值。
    pub(super) fn complex_typed_initial_tokens(
        // 只读借用声明注册表。
        &self,
        // 接收已完成组件字段改写的初始表达式。
        expression: &Expression,
        // 接收目标复合类型。
        value_type: &WidgetValueType,
    ) -> Result<(TokenStream, TokenStream), Diagnostic> {
        // 按目标类型选择递归生成路径。
        let initial = match value_type {
            // 可空字符串支持 None 或 Some('text')。
            WidgetValueType::OptionalString => self.optional_string_tokens(expression)?,
            // 字符串集合按元素生成拥有型 String 后收集。
            WidgetValueType::HashSetOfString => {
                // 生成字符串数组元素。
                let items = self.array_items_tokens(expression, &WidgetValueType::String)?;
                // 收集为确定类型 HashSet。
                quote! {
                    ::std::iter::IntoIterator::into_iter(::std::vec![#(#items),*])
                        .collect::<::std::collections::HashSet<::std::string::String>>()
                }
            }
            // 字符串向量支持非空数组字面量。
            WidgetValueType::VecOfString => {
                // 生成拥有型字符串项目。
                let items = self.array_items_tokens(expression, &WidgetValueType::String)?;
                // 构造类型可推断向量。
                quote! { ::std::vec![#(#items),*] }
            }
            // 数值向量支持非空数组字面量。
            WidgetValueType::VecOfNumber => {
                // 生成 f64 项目。
                let items = self.array_items_tokens(expression, &WidgetValueType::Number)?;
                // 构造类型可推断向量。
                quote! { ::std::vec![#(#items),*] }
            }
            // record 向量按声明字段递归物化每个对象。
            WidgetValueType::VecOfRecord(name) => {
                // 构造 record 元素目标类型。
                let item_type = WidgetValueType::Record(name.clone());
                // 生成全部 record 项目。
                let items = self.array_items_tokens(expression, &item_type)?;
                // 构造 record 向量。
                quote! { ::std::vec![#(#items),*] }
            }
            WidgetValueType::Library(name) => {
                let declaration = crate::lang::compiler::components::value_declaration(name).expect("library value remains in compilation scope");
                super::component_codegen::expression_value(expression, &declaration.initial, None)?
            }
            // CascaderValue 与 record 使用递归结构体字面量。
            WidgetValueType::Record(_) => {
                // 生成结构体字面量。
                self.struct_literal_tokens(expression, value_type)?
            }
            // 调用方只应路由复合类型。
            _ => {
                // 返回内部路由诊断。
                return Err(Diagnostic::new(
                    // 指向初始表达式。
                    expression.span,
                    // 陈述错误路由类型。
                    "复合初始值生成器收到非复合类型",
                    // 给出内部修复方向。
                    "把基础或语义类型交给普通 state 初始值生成路径",
                ));
            }
        };
        // 生成目标 Rust 类型。
        let rust_type = value_type_tokens(value_type.clone());
        // 返回初始值与显式类型。
        Ok((initial, rust_type))
    }

    // 生成 None 或 Some(String) 可空字符串。
    fn optional_string_tokens(&self, expression: &Expression) -> Result<TokenStream, Diagnostic> {
        // None 标识符映射为完整标准库路径。
        if matches!(&expression.kind, ExpressionKind::Identifier(name) if name == "None") {
            // 返回空可选值。
            return Ok(quote! { ::std::option::Option::None });
        }
        // Some 必须是单个位置参数调用。
        let ExpressionKind::Call { callee, arguments } = &expression.kind else {
            // 返回可空初始化形状诊断。
            return Err(optional_string_diagnostic(expression));
        };
        // 调用目标必须是精确 Some 标识符。
        if !matches!(&callee.kind, ExpressionKind::Identifier(name) if name == "Some")
            // 参数数量必须恰好为一。
            || arguments.len() != 1
            // 参数不能使用命名形式。
            || arguments[0].name.is_some()
        {
            // 返回统一形状诊断。
            return Err(optional_string_diagnostic(expression));
        }
        // Some 首版只接受确定字符串字面量或其组件所有权包装。
        let value = match &arguments[0].value.kind {
            // 未经组件改写的直接字符串字面量。
            ExpressionKind::String(value) => {
                // 显式取得 String 所有权。
                quote! { ::std::string::String::from(#value) }
            }
            // 组件表达式降低会把字符串字面量包装为拥有型零参函数。
            ExpressionKind::Call {
                callee,
                arguments: inner_arguments,
            } if inner_arguments.len() == 1
                && matches!(
                    &callee.kind,
                    ExpressionKind::Identifier(name)
                        if name.starts_with("__uix_owned_string_")
                ) =>
            {
                // 复用已生成的拥有型字符串包装调用。
                generate_expression(&arguments[0].value, None)?
            }
            // 其他参数不是规范字符串字面量。
            _ => return Err(optional_string_diagnostic(expression)),
        };
        // 返回拥有型 Some(String)。
        Ok(quote! { ::std::option::Option::Some(#value) })
    }

    // 从数组表达式生成指定目标元素类型的令牌。
    fn array_items_tokens(
        // 只读借用声明注册表。
        &self,
        // 接收数组表达式。
        expression: &Expression,
        // 接收目标元素类型。
        item_type: &WidgetValueType,
    ) -> Result<Vec<TokenStream>, Diagnostic> {
        // 要求数组字面量形状。
        let ExpressionKind::Array(items) = &expression.kind else {
            // 返回目标集合形状诊断。
            return Err(array_shape_diagnostic(
                // 传递初始表达式。
                expression,
                // 展示目标元素 Rust 类型。
                &value_type_tokens(item_type.clone()).to_string(),
            ));
        };
        // 按源码顺序递归生成每个项目。
        items
            // 遍历数组项目。
            .iter()
            // 依照目标元素类型物化。
            .map(|item| self.typed_value_tokens(item, item_type))
            // 收集或返回首个字段诊断。
            .collect()
    }

    // 按已知目标类型递归生成一个值。
    fn typed_value_tokens(
        // 只读借用声明注册表。
        &self,
        // 接收当前值表达式。
        expression: &Expression,
        // 接收目标字段或元素类型。
        value_type: &WidgetValueType,
    ) -> Result<TokenStream, Diagnostic> {
        // 按目标类型决定所有权与递归结构。
        match value_type {
            // 字符串字面量转成拥有型 String。
            WidgetValueType::String => {
                // 字面量需要显式取得所有权。
                if let ExpressionKind::String(value) = &expression.kind {
                    // 返回拥有型字符串。
                    return Ok(quote! { ::std::string::String::from(#value) });
                }
                // 其他表达式由 Rust 验证 String 类型。
                generate_expression(expression, None)
            }
            // number 统一转换为 f64。
            WidgetValueType::Number => {
                // 生成原始数值表达式。
                let value = generate_expression(expression, None)?;
                // 固定 number 语义类型。
                Ok(quote! { (#value) as f64 })
            }
            // 整数与单精度字段按目标类型显式转换。
            WidgetValueType::U32
            | WidgetValueType::USize
            | WidgetValueType::F32
            | WidgetValueType::I32 => {
                // 生成原始数值表达式。
                let value = generate_expression(expression, None)?;
                // 生成目标 Rust 类型。
                let rust_type = value_type_tokens(value_type.clone());
                // 返回显式转换。
                Ok(quote! { (#value) as #rust_type })
            }
            // record 字段递归物化嵌套对象。
            WidgetValueType::Record(_) => self.struct_literal_tokens(expression, value_type),
            // 其余复合类型递归使用统一复合生成器。
            WidgetValueType::OptionalString
            | WidgetValueType::HashSetOfString
            | WidgetValueType::VecOfString
            | WidgetValueType::VecOfNumber
            | WidgetValueType::VecOfRecord(_)
            | WidgetValueType::Library(_) => self
                // 生成复合初始值并丢弃重复类型令牌。
                .complex_typed_initial_tokens(expression, value_type)
                // 只返回初始值。
                .map(|(initial, _)| initial),
            // 布尔与语义数据构造由共享表达式生成器处理。
            WidgetValueType::Bool

            | WidgetValueType::Color
            | WidgetValueType::Point => generate_expression(expression, None),
        }
    }

    // 生成结构类型 state 或字段的对象字面量并校验字段集合。
    fn struct_literal_tokens(
        // 只读借用展开状态。
        &self,
        // 接收改写后的对象字面量。
        expression: &Expression,
        // 接收目标结构类型。
        value_type: &WidgetValueType,
    ) -> Result<TokenStream, Diagnostic> {
        // 要求对象字面量形状。
        let ExpressionKind::Object(fields) = &expression.kind else {
            // 返回结构初始值形状诊断。
            return Err(Diagnostic::new(
                // 指向初始值表达式。
                expression.span,
                // 说明结构类型需要对象字面量。
                "结构类型必须使用对象字面量初始化",
                // 给出规范示例。
                "使用 { labels: [], values: [] } 或 record 字段对象",
            ));
        };
        // 取得目标字段表与类型令牌。
        let (type_tokens, target_fields) = self.struct_target_fields(expression, value_type)?;
        // 校验对象字段与目标字段一一对应。
        validate_object_fields(expression, fields, &target_fields)?;
        // 生成字段令牌。
        let field_tokens = fields
            // 按源码顺序生成。
            .iter()
            // 转换每个字段。
            .map(|field| {
                // 验证字段名。
                let name = rust_identifier(&field.name, field.span)?;
                // 查找已经验证存在的目标字段类型。
                let field_type = target_fields
                    // 遍历目标字段表。
                    .iter()
                    // 查找同名声明。
                    .find(|(target, _)| target == &field.name)
                    // 完整性校验保证存在。
                    .map(|(_, value_type)| value_type)
                    // 防御性断言字段已验证。
                    .expect("对象字段已完成集合校验");
                // 按目标字段类型递归生成值。
                let value = self.typed_value_tokens(&field.value, field_type)?;
                // 返回字段令牌。
                Ok(quote! { #name: #value })
            })
            // 收集或返回首个诊断。
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        // 返回结构体字面量。
        Ok(quote! { #type_tokens { #(#field_tokens),* } })
    }

    // 查询 CascaderValue 或文档 record 的目标字段表。
    fn struct_target_fields(
        // 只读借用声明注册表。
        &self,
        // 接收诊断定位表达式。
        expression: &Expression,
        // 接收目标结构类型。
        value_type: &WidgetValueType,
    ) -> Result<(TokenStream, Vec<(String, WidgetValueType)>), Diagnostic> {
        // 按结构类型返回公开类型路径和字段声明。
        match value_type {
            // record 使用文档声明的字段集合。
            WidgetValueType::Record(name) => {
                // 查找已声明 record。
                let record = self.records.get(name).ok_or_else(|| {
                    // 构造未声明 record 诊断。
                    Diagnostic::new(
                        // 指向初始值表达式。
                        expression.span,
                        // 说明记录缺失。
                        format!("record 类型 {name} 未在当前文档声明"),
                        // 给出修复动作。
                        "在顶层声明区添加 <Record name=\"...\" fields=\"...\" />",
                    )
                })?;
                // 验证名称可映射为 Rust 标识符。
                let ident = syn::parse_str::<Ident>(name).expect("record 名已在解析期验证");
                // 收集字段名与类型。
                let fields = record
                    // 遍历声明字段。
                    .fields
                    // 借用字段序列。
                    .iter()
                    // 复制字段名与类型。
                    .map(|field| (field.name.clone(), field.kind.clone()))
                    // 收集字段表。
                    .collect();
                // 返回标识符与字段表。
                Ok((quote! { #ident }, fields))
            }
            // 其他类型不进入本路径。
            _ => Err(Diagnostic::new(
                // 指向初始值表达式。
                expression.span,
                // 陈述路由错误。
                "结构字段查询收到非结构类型",
                // 给出内部修复方向。
                "仅把 CascaderValue 或 record 交给结构生成器",
            )),
        }
    }
}

// 校验对象字段集合与目标声明一一对应。
fn validate_object_fields(
    // 接收完整对象表达式。
    expression: &Expression,
    // 接收源码字段。
    fields: &[ObjectField],
    // 接收目标字段声明。
    target_fields: &[(String, WidgetValueType)],
) -> Result<(), Diagnostic> {
    // 拒绝所有未知字段。
    for field in fields {
        // 当前字段必须存在目标声明。
        if !target_fields.iter().any(|(name, _)| name == &field.name) {
            // 返回未知字段诊断。
            return Err(Diagnostic::new(
                // 指向当前字段。
                field.span,
                // 说明字段不属于目标结构。
                format!("字段 {} 不属于该结构类型", field.name),
                // 给出合法边界。
                "只使用结构类型声明的字段名",
            ));
        }
    }
    // 拒绝所有缺失字段。
    for (name, _) in target_fields {
        // 当前目标字段必须有源码值。
        if !fields.iter().any(|field| field.name == *name) {
            // 返回缺字段诊断。
            return Err(Diagnostic::new(
                // 指向完整对象表达式。
                expression.span,
                // 说明缺失字段。
                format!("对象字面量缺少字段 {name}"),
                // 给出补全动作。
                "为结构类型补齐全部声明字段",
            ));
        }
    }
    // 字段集合完全匹配。
    Ok(())
}

// 构造数组初始值形状诊断。
fn array_shape_diagnostic(expression: &Expression, item_type: &str) -> Diagnostic {
    // 返回目标数组写法。
    Diagnostic::new(
        // 指向初始表达式。
        expression.span,
        // 陈述集合初始值要求。
        format!("集合类型 state 需要 {item_type} 数组字面量"),
        // 给出规范数组示例。
        "使用 [] 或 [item1, item2] 初始化 Vec/HashSet",
    )
}

// 构造 Option<String> 初始化诊断。
fn optional_string_diagnostic(expression: &Expression) -> Diagnostic {
    // 返回 None/Some 唯一写法。
    Diagnostic::new(
        // 指向非法初始表达式。
        expression.span,
        // 陈述可空字符串构造集合。
        "Option<String> state 只接受 None 或 Some('text') 初始化",
        // 给出两种合法示例。
        "使用 selected: Option<String> = None 或 selected: Option<String> = Some('x')",
    )
}
