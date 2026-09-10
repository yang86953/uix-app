// 引入卫生标识符、数值字面量与令牌流。
use proc_macro2::{Ident, Literal, TokenStream};
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入组件展开器与字段绑定类型。
use super::widget_codegen::{Binding, BindingKind, Bindings, WidgetExpander};
// 引入数字源码恢复辅助。
use super::widget_expression_lower::{collect_number_sources, restore_number_sources};
// 引入组件字段、属性、表达式与诊断 AST。
use super::{
    Attribute, AttributeValue, Diagnostic, ExpressionKind, ExpressionNode, WidgetProp,
    WidgetPropType, WidgetState, WidgetStateInitial, WidgetValueType, value_type_tokens,
};
// 引入既有受限表达式生成入口。
use super::generate_expression;

// 实现 props、私有状态与调用参数的类型化绑定。
impl WidgetExpander {
    // 把拥有型事件捕获登记到最近 For 的逐迭代克隆契约。
    pub(super) fn register_for_iteration_clone(&mut self, ident: &Ident) {
        // 已经在当前迭代内创建的值不会被前一行闭包移走。
        if !self.for_iteration_setup_stack.is_empty() {
            // 逐迭代准备语句本身就是所需的独立所有权边界。
            return;
        }
        // 只有位于 For 子树内时才需要额外克隆。
        if let Some(clones) = self.for_iteration_clone_stack.last_mut() {
            // 保存卫生标识符文本供后续控制流代码生成恢复。
            clones.push(ident.to_string());
        }
    }

    // 把逐迭代准备语句引用的组件级声明名登记到最近 For 的引用名契约。
    pub(super) fn register_for_iteration_outer_capture(&mut self, ident: &Ident) {
        // 只有位于 For 子树内时才需要记录（惰性行工厂据此克隆遮蔽）。
        if let Some(captures) = self.for_iteration_capture_stack.last_mut() {
            // 同一声明可能被多个事件准备引用，保留首次出现顺序去重。
            let name = ident.to_string();
            if !captures.contains(&name) {
                // 保存卫生标识符文本供 VirtualScroll 闭包外克隆遮蔽。
                captures.push(name);
            }
        }
    }

    // 生成一个类型化 prop 的 Rust 局部绑定。
    pub(super) fn emit_prop_binding(
        // 可变借用展开状态。
        &mut self,
        // 接收 prop 声明。
        prop: &WidgetProp,
        // 接收调用处同名属性。
        attribute: &Attribute,
        // 接收调用方组件绑定。
        outer_bindings: &Bindings,
        // 接收被调用组件的绑定表。
        bindings: &mut Bindings,
    ) -> Result<(), Diagnostic> {
        // 为字段值分配卫生名称。
        let value_ident = self.fresh_ident("prop", &prop.name);
        // 按 prop 类别生成准备语句。
        match &prop.kind {
            // 基础值 prop 使用显式 Rust 类型检查。
            WidgetPropType::Value(value_type) => {
                // 生成调用属性值。
                let value =
                    self.widget_value_tokens(attribute, outer_bindings, value_type.clone())?;
                // 生成目标 Rust 类型。
                let rust_type = value_type_tokens(value_type.clone());
                // 声明类型化局部值。
                self.push_setup(quote! {
                    // 让 Rust 编译器验证基础 prop 类型。
                    let #value_ident: #rust_type = #value;
                });
                // 登记组件体中的值读取绑定。
                bindings.insert(
                    // 使用源码 prop 名称作为键。
                    prop.name.clone(),
                    // 保存生成后的字段绑定。
                    Binding {
                        // 记录卫生局部名称。
                        value_name: value_ident.to_string(),
                        // 普通值不能由 setState 写入。
                        state_name: None,
                        // 标记基础值类别。
                        kind: BindingKind::Value,
                        // 基础值 prop 不参与数字形状保留。
                        authored_numbers: false,
                    },
                );
            }
            // 共享状态 prop 保留同一 State 句柄。
            WidgetPropType::State(value_type) => {
                // 生成调用方 State 表达式。
                let value = self.state_argument_tokens(attribute, outer_bindings)?;
                // 生成 State 内部值类型。
                let rust_type = value_type_tokens(value_type.clone());
                // 克隆共享句柄但不在组件构建入口读取当前值。
                self.push_setup(quote! {
                    // 克隆句柄并保持同一底层状态槽。
                    let #value_ident: ::uix_app::prelude::State<#rust_type> = (#value).clone();
                });
                // 登记可读写共享状态。
                bindings.insert(
                    // 使用源码 prop 名称作为键。
                    prop.name.clone(),
                    // 保存读值与句柄绑定。
                    Binding {
                        // 普通表达式按实际使用位置从该句柄读取当前值。
                        value_name: value_ident.to_string(),
                        // setState 写回共享句柄。
                        state_name: Some(value_ident.to_string()),
                        // 标记响应式状态类别。
                        kind: BindingKind::State,
                        // State prop 白名单只有 number 语义，不保留作者数字形状。
                        authored_numbers: false,
                    },
                );
            }
            // 回调 prop 适配为可克隆的类型擦除 Fn。
            WidgetPropType::Callback {
                // 借用参数类型列表。
                parameters,
                // 借用可选返回类型。
                returns,
            } => {
                // 生成调用方回调表达式。
                let callback = self.callback_argument_tokens(attribute, outer_bindings)?;
                // 为可重复 View 根中的当前回调来源分配卫生名称。
                let callback_source = self.fresh_ident("callback_source", &prop.name);
                // 在创建 move 适配器前克隆调用方回调，避免移出外层 Fn 根工厂。
                self.push_setup(quote! {
                    // 每次 View 构建取得一份独立可调用所有权。
                    let #callback_source = ::std::clone::Clone::clone(&(#callback));
                });
                // 为适配器参数生成卫生名称。
                let arguments = parameters
                    // 遍历参数位置。
                    .iter()
                    // 同时读取位置编号。
                    .enumerate()
                    // 按位置分配标识符。
                    .map(|(index, _)| self.fresh_ident("callback_arg", &index.to_string()))
                    // 收集全部参数名称。
                    .collect::<Vec<_>>();
                // 生成每个参数的 Rust 类型。
                let parameter_types = parameters
                    // 遍历参数类型。
                    .iter()
                    // 复制基础类型枚举。
                    .cloned()
                    // 映射为 Rust 类型。
                    .map(value_type_tokens)
                    // 收集类型列表。
                    .collect::<Vec<_>>();
                // 无显式返回值时使用单元类型。
                let return_type = returns
                    // 复制存在的返回类型。
                    .as_ref()
                    // 复制基础类型枚举。
                    .cloned()
                    // 映射为 Rust 类型。
                    .map(value_type_tokens)
                    // 回退为单元类型。
                    .unwrap_or_else(|| quote! { () });
                // 生成类型擦除回调适配器。
                self.push_setup(quote! {
                    // 显式 Fn 签名验证回调参数和返回值。
                    let #value_ident: ::std::sync::Arc<dyn Fn(#(#parameter_types),*) -> #return_type> =
                        // 捕获调用方函数或闭包并提供可克隆句柄。
                        ::std::sync::Arc::new(move |#(#arguments: #parameter_types),*| {
                            // 按声明顺序转发全部参数。
                            (#callback_source)(#(#arguments),*)
                        });
                });
                // 登记组件体中的回调绑定。
                bindings.insert(
                    // 使用源码 prop 名称作为键。
                    prop.name.clone(),
                    // 保存回调字段绑定。
                    Binding {
                        // 记录类型擦除回调名称。
                        value_name: value_ident.to_string(),
                        // 回调不能由 setState 写入。
                        state_name: None,
                        // 标记回调类别。
                        kind: BindingKind::Callback,
                        // 回调不参与数字形状保留。
                        authored_numbers: false,
                    },
                );
            }
        }
        // 报告当前 prop 绑定成功。
        Ok(())
    }

    // 生成一个私有 State 句柄和当前读值。
    pub(super) fn emit_private_state(
        // 可变借用展开状态。
        &mut self,
        // 接收当前静态组件实例的运行时作用域。
        scope: &Ident,
        // 接收 state 声明。
        state: &WidgetState,
        // 接收当前组件绑定表。
        bindings: &mut Bindings,
    ) -> Result<(), Diagnostic> {
        // 为 State 句柄生成卫生名称。
        let state_ident = self.fresh_ident("state", &state.name);
        // 生成规范化初始值与可选显式类型。
        let (initial, rust_type, authored_numbers) =
            self.private_state_initial(&state.initial, bindings)?;
        // 用字段名称与声明跨度区分同一组件实例内的多个私有 state。
        // 先拼接 state 名称与声明跨度，区分同一组件实例内的多个私有字段。
        let field_source = format!("{}:{}:{}", state.name, state.span.start, state.span.end);
        // 使用与调用声明一致的固定哈希算法生成运行时字段编号。
        let field_id = Self::stable_widget_id(&field_source);
        // 基础类型使用显式 State 类型。
        if let Some(rust_type) = rust_type {
            // 写入显式类型准备语句。
            self.push_setup(quote! {
                // 创建组件私有响应式状态槽。
                let #state_ident: ::uix_app::prelude::State<#rust_type> =
                    // 从当前组件实例作用域复用或创建规范化初始值对应的状态槽。
                    ::uix_app::ui::__private::uix_widget_state(&#scope, #field_id, || #initial);
            });
        } else {
            // 复合表达式或空数组由 Rust 推断类型。
            self.push_setup(quote! {
                // 从当前组件实例作用域复用或创建由 Rust 推断内部类型的状态槽。
                let #state_ident = ::uix_app::ui::__private::uix_widget_state(
                    // 传递当前静态组件实例的运行时作用域。
                    &#scope,
                    // 传递当前私有字段的稳定身份。
                    #field_id,
                    // 延迟构造初始值，避免重建时重复求值。
                    || #initial,
                );
            });
        }
        // 登记私有状态读写绑定。
        bindings.insert(
            // 使用源码 state 名称作为键。
            state.name.clone(),
            // 保存读值与句柄。
            Binding {
                // 普通表达式按实际使用位置从该句柄读取当前值。
                value_name: state_ident.to_string(),
                // setState 写回私有句柄。
                state_name: Some(state_ident.to_string()),
                // 标记响应式状态类别。
                kind: BindingKind::State,
                // 类型化整数状态在 setState 值中保留作者数字形状。
                authored_numbers,
            },
        );
        // 报告私有状态绑定成功。
        Ok(())
    }

    // 生成私有状态初始值并固定基础字面量类型。
    fn private_state_initial(
        // 可变借用展开状态。
        &mut self,
        // 接收解析后的初始值。
        initial: &WidgetStateInitial,
        // 接收此前声明的字段绑定。
        bindings: &Bindings,
    ) -> Result<(TokenStream, Option<TokenStream>, bool), Diagnostic> {
        // 按初始值形状生成 Rust 表达式。
        match initial {
            // 空数组映射为可推断元素类型的 Vec。
            WidgetStateInitial::EmptyArray => Ok((
                // 生成空向量。
                quote! { ::std::vec::Vec::new() },
                // 元素类型留给后续使用推断。
                None,
                // 空数组不保留作者数字形状。
                false,
            )),
            // 普通表达式按顶层字面量固定基础类型。
            WidgetStateInitial::Expression(expression) => {
                // 克隆表达式以改写此前字段读取。
                let mut expanded = expression.clone();
                // 初始值不能执行 setState。
                self.transform_expression(&mut expanded, bindings, false, false)?;
                // 按表达式形状生成拥有所有权的值。
                match &expanded.kind {
                    // 字符串状态映射为 String。
                    ExpressionKind::String(value) => Ok((
                        // 复制字符串字面量。
                        quote! { ::std::string::String::from(#value) },
                        // 固定 String 类型。
                        Some(quote! { ::std::string::String }),
                        // 字符串不保留作者数字形状。
                        false,
                    )),
                    // 数字状态固定为 f64。
                    ExpressionKind::Number(_) => {
                        // 生成原始数值令牌。
                        let value = generate_expression(&expanded, None)?;
                        // 返回显式 f64 转换。
                        Ok((
                            // 统一整数与小数初始值。
                            quote! { (#value) as f64 },
                            // 固定 f64 类型。
                            Some(quote! { f64 }),
                            // f64 状态沿用组件 number 语义。
                            false,
                        ))
                    }
                    // 布尔状态固定为 bool。
                    ExpressionKind::Boolean(_) => {
                        // 生成布尔值令牌。
                        let value = generate_expression(&expanded, None)?;
                        // 返回值与显式类型。
                        Ok((value, Some(quote! { bool }), false))
                    }
                    // 复合表达式交给 Rust 完整推断。
                    _ => {
                        // 生成复合表达式。
                        let value = generate_expression(&expanded, None)?;
                        // 不增加额外类型约束。
                        Ok((value, None, false))
                    }
                }
            }
            // 带显式类型注解的表达式按注解固定 State 内部类型。
            WidgetStateInitial::TypedExpression(value_type, expression) => {
                // 克隆表达式以改写此前字段读取。
                let mut expanded = expression.clone();
                // 记录作者数字源码，避免组件 number 语义污染整数字面量。
                let authored_numbers = collect_number_sources(&expanded);
                // 初始值不能执行 setState。
                self.transform_expression(&mut expanded, bindings, false, false)?;
                // 恢复类型化初始值中的作者数字形状。
                restore_number_sources(&mut expanded, &authored_numbers);
                // 按类型类别生成确定类型与初始值。
                match value_type {
                    // 数字与整数注解统一走 as 转换：impl FnOnce() -> T 不会把
                    // 期望类型传回闭包体，整数字面量会停留在 {integer}。
                    WidgetValueType::Number
                    | WidgetValueType::U32
                    | WidgetValueType::USize
                    | WidgetValueType::F32
                    | WidgetValueType::I32 => {
                        // 生成原始数值令牌。
                        let value = generate_expression(&expanded, None)?;
                        // 生成显式 Rust 类型。
                        let rust_type = value_type_tokens(value_type.clone());
                        // 生成显式转换表达式。
                        let initial = quote! { (#value) as #rust_type };
                        // 整数与单精度注解需要在 setState 值中保留作者数字形状。
                        let authored = matches!(
                            value_type,
                            WidgetValueType::U32
                                | WidgetValueType::USize
                                | WidgetValueType::F32
                                | WidgetValueType::I32
                        );
                        // 返回显式类型化初始值。
                        Ok((initial, Some(rust_type), authored))
                    }
                    // 字符串与布尔字面量自带确定类型。
                    WidgetValueType::String | WidgetValueType::Bool => {
                        // 生成字面量令牌。
                        let value = generate_expression(&expanded, None)?;
                        // 生成显式 Rust 类型。
                        let rust_type = value_type_tokens(value_type.clone());
                        // 返回字面量与显式类型。
                        Ok((value, Some(rust_type), false))
                    }
                    // Option、集合与结构类型使用递归目标类型生成器。
                    WidgetValueType::OptionalString
                    | WidgetValueType::HashSetOfString
                    | WidgetValueType::VecOfString
                    | WidgetValueType::VecOfNumber
                    | WidgetValueType::VecOfRecord(_)
                    | WidgetValueType::VecOfUploadFile
                    | WidgetValueType::CascaderValue
                    | WidgetValueType::Record(_) => {
                        // 生成递归初始值与显式 Rust 类型。
                        let (initial, rust_type) =
                            self.complex_typed_initial_tokens(&expanded, value_type)?;
                        // 返回复合初始值。
                        Ok((initial, Some(rust_type), false))
                    }
                    // 语义类型走已登记的数据构造调用。
                    WidgetValueType::Date
                    | WidgetValueType::Time
                    | WidgetValueType::Color
                    | WidgetValueType::Point => {
                        // 要求数据构造调用形状。
                        if !matches!(&expanded.kind, ExpressionKind::Call { .. }) {
                            // 返回构造调用形状诊断。
                            return Err(Diagnostic::new(
                                // 指向初始值表达式。
                                expanded.span,
                                // 说明语义类型需要构造调用。
                                "语义类型 state 必须使用数据类型构造调用初始化",
                                // 给出规范示例。
                                "使用 Date(2026, 8, 10)、Time(14, 30)、Color('#1677ff') 或 Point(0.0, 0.0)",
                            ));
                        }
                        // 生成构造调用令牌。
                        let value = generate_expression(&expanded, None)?;
                        // 生成显式 Rust 类型。
                        let rust_type = value_type_tokens(value_type.clone());
                        // 返回构造初始值。
                        Ok((value, Some(rust_type), false))
                    }
                }
            }
        }
    }

    // 为一个事件克隆全部组件字段。
    pub(super) fn clone_event_bindings(&mut self, bindings: &Bindings) -> Bindings {
        // 保存事件专用字段表。
        let mut event_bindings = Bindings::new();
        // 按名称顺序遍历字段。
        for (name, binding) in bindings {
            // 为事件值分配卫生名称。
            let value_ident = self.fresh_ident("event_value", name);
            // State 在注册事件时只克隆句柄，事件执行时再读取最新值。
            if binding.kind == BindingKind::State {
                // 读取必有的 State 句柄。
                let state_ident = super::widget_expression_lower::ident_from_name(
                    // State 类别保证句柄存在。
                    binding
                        .state_name
                        .as_deref()
                        .expect("State 绑定必须包含句柄"),
                );
                // 生成事件闭包专用状态句柄。
                self.push_setup(quote! {
                    // 每个事件持有同一底层状态槽的独立句柄所有权。
                    let #value_ident = #state_ident.clone();
                });
                // 惰性行工厂闭包外需要先克隆遮蔽该组件级句柄。
                self.register_for_iteration_outer_capture(&state_ident);
            } else {
                // 读取普通值或 Arc 回调名称。
                let source_ident = super::widget_expression_lower::ident_from_name(
                    // 借用卫生字段名称。
                    &binding.value_name,
                );
                // 克隆所有权给当前 move 闭包。
                self.push_setup(quote! {
                    // 避免多个事件闭包争用同一字段局部值。
                    let #value_ident = (#source_ident).clone();
                });
                // 惰性行工厂闭包外需要先克隆遮蔽该组件级值。
                self.register_for_iteration_outer_capture(&source_ident);
            }
            // For 子树中的事件必须在每次迭代重新克隆这份拥有型捕获。
            self.register_for_iteration_clone(&value_ident);
            // 保存事件专用绑定并复用 State 句柄。
            event_bindings.insert(
                // 保留源码字段名。
                name.clone(),
                // 写入事件字段绑定。
                Binding {
                    // 事件表达式读取专用副本。
                    value_name: value_ident.to_string(),
                    // State 事件值与 setState 都使用事件闭包拥有的同槽句柄。
                    state_name: if binding.kind == BindingKind::State {
                        // 返回事件闭包专用句柄名称。
                        Some(value_ident.to_string())
                    } else {
                        // 普通值与回调没有可写状态句柄。
                        binding.state_name.clone()
                    },
                    // 保留字段类别。
                    kind: binding.kind,
                    // 保留数字形状策略。
                    authored_numbers: binding.authored_numbers,
                },
            );
        }
        // 返回事件字段表。
        event_bindings
    }

    // 生成基础值 prop 的调用参数。
    fn widget_value_tokens(
        // 可变借用展开状态。
        &mut self,
        // 接收调用属性。
        attribute: &Attribute,
        // 接收调用方字段绑定。
        outer_bindings: &Bindings,
        // 接收目标基础类型。
        value_type: WidgetValueType,
    ) -> Result<TokenStream, Diagnostic> {
        // 按属性值形状生成表达式。
        match &attribute.value {
            // 字面量按目标类型严格解析。
            AttributeValue::Literal(value) => match value_type {
                // String prop 获取拥有所有权的字符串。
                WidgetValueType::String => {
                    // 返回 String 构造表达式。
                    Ok(quote! { ::std::string::String::from(#value) })
                }
                // number prop 解析为有限 f64。
                WidgetValueType::Number => {
                    // 解析十进制字面量。
                    let number = value.parse::<f64>().map_err(|_| {
                        // 构造数值类型诊断。
                        Diagnostic::new(
                            // 指向完整属性。
                            attribute.span,
                            // 说明类型不符。
                            format!("prop {} 需要 number，收到 {value:?}", attribute.name),
                            // 给出合法写法。
                            "使用十进制数字字面量或 {number_expression}",
                        )
                    })?;
                    // 拒绝非有限数值。
                    if !number.is_finite() {
                        // 返回有限数值诊断。
                        return Err(Diagnostic::new(
                            // 指向完整属性。
                            attribute.span,
                            // 说明 number 必须有限。
                            format!("prop {} 的 number 必须是有限值", attribute.name),
                            // 给出修复建议。
                            "使用有限十进制数",
                        ));
                    }
                    // 生成无后缀 f64 字面量。
                    let literal = Literal::f64_unsuffixed(number);
                    // 返回数值令牌。
                    Ok(quote! { #literal })
                }
                // bool prop 只接受 true 或 false。
                WidgetValueType::Bool => match value.as_str() {
                    // 生成 true。
                    "true" => Ok(quote! { true }),
                    // 生成 false。
                    "false" => Ok(quote! { false }),
                    // 拒绝其他字面量。
                    _ => Err(Diagnostic::new(
                        // 指向完整属性。
                        attribute.span,
                        // 说明类型不符。
                        format!("prop {} 需要 bool，收到 {value:?}", attribute.name),
                        // 给出合法值。
                        "使用 \"true\"、\"false\" 或 {boolean_expression}",
                    )),
                },
                // 扩展类型只供类型化私有 state 或 record 字段使用，props 解析器不会产生。
                WidgetValueType::U32
                | WidgetValueType::USize
                | WidgetValueType::F32
                | WidgetValueType::I32
                | WidgetValueType::Date
                | WidgetValueType::Time
                | WidgetValueType::Color
                | WidgetValueType::Point
                | WidgetValueType::CascaderValue
                | WidgetValueType::HashSetOfString
                | WidgetValueType::VecOfString
                | WidgetValueType::VecOfNumber
                | WidgetValueType::VecOfRecord(_)
                | WidgetValueType::VecOfUploadFile
                | WidgetValueType::OptionalString
                | WidgetValueType::Record(_) => Err(Diagnostic::new(
                    // 指向完整属性。
                    attribute.span,
                    // 说明类型仅限 state 注解。
                    format!(
                        "prop {} 不能使用仅限私有 state 或 record 字段的类型",
                        attribute.name
                    ),
                    // 给出 props 白名单。
                    "props 使用 String、number、bool、State<T> 或回调签名",
                )),
            },
            // 表达式由显式局部类型完成检查。
            AttributeValue::Expression(expression) => {
                // 生成改写后的表达式参数。
                self.expression_argument_tokens(expression, outer_bindings, true)
            }
            // 内联样式不能作为组件 prop。
            AttributeValue::InlineStyle(_) => Err(Diagnostic::new(
                // 指向完整属性。
                attribute.span,
                // 说明值形状不兼容。
                format!("组件 prop {} 不能使用内联样式值", attribute.name),
                // 给出普通值写法。
                "使用字面量或花括号表达式",
            )),
        }
    }

    // 生成 State<T> prop 的调用方句柄。
    fn state_argument_tokens(
        // 可变借用展开状态。
        &mut self,
        // 接收调用属性。
        attribute: &Attribute,
        // 接收调用方字段绑定。
        outer_bindings: &Bindings,
    ) -> Result<TokenStream, Diagnostic> {
        // State prop 必须使用表达式。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回句柄形状诊断。
            return Err(Diagnostic::new(
                // 指向完整属性。
                attribute.span,
                // 说明字面量不能表示 State。
                format!("State<T> prop {} 必须接收响应式状态表达式", attribute.name),
                // 给出规范写法。
                format!("使用 {}={{shared_state}}", attribute.name),
            ));
        };
        // 组件内状态字段要传递句柄而非读值。
        if let ExpressionKind::Identifier(name) = &expression.expression.kind {
            // 查找调用方字段。
            if let Some(binding) = outer_bindings.get(name) {
                // 来源必须可写。
                let state_name = binding.state_name.as_deref().ok_or_else(|| {
                    // 构造普通值冒充 State 的诊断。
                    Diagnostic::new(
                        // 指向完整属性。
                        attribute.span,
                        // 说明来源字段类型错误。
                        format!("{name} 不是 State<T> prop 或私有 state"),
                        // 给出合法来源。
                        "传入 Rust 侧 State<T>，或转发组件内的 State<T> 字段",
                    )
                })?;
                // 恢复句柄标识符。
                let state_ident = super::widget_expression_lower::ident_from_name(state_name);
                // 返回共享句柄表达式。
                return Ok(quote! { #state_ident });
            }
        }
        // Rust 外层 State 名称保持普通解析。
        self.expression_argument_tokens(expression, outer_bindings, false)
    }

    // 生成回调 prop 的调用方表达式。
    fn callback_argument_tokens(
        // 可变借用展开状态。
        &mut self,
        // 接收调用属性。
        attribute: &Attribute,
        // 接收调用方字段绑定。
        outer_bindings: &Bindings,
    ) -> Result<TokenStream, Diagnostic> {
        // 回调 prop 必须使用表达式。
        let AttributeValue::Expression(expression) = &attribute.value else {
            // 返回回调值形状诊断。
            return Err(Diagnostic::new(
                // 指向完整属性。
                attribute.span,
                // 说明字符串不是可调用值。
                format!("回调 prop {} 必须接收函数或闭包表达式", attribute.name),
                // 给出规范写法。
                format!("使用 {}={{handler}}", attribute.name),
            ));
        };
        // 生成可调用表达式并支持字段转发。
        self.expression_argument_tokens(expression, outer_bindings, true)
    }

    // 生成组件调用表达式参数并可克隆简单字段。
    fn expression_argument_tokens(
        // 可变借用展开状态。
        &mut self,
        // 接收表达式节点。
        expression: &ExpressionNode,
        // 接收调用方绑定。
        outer_bindings: &Bindings,
        // 标记简单字段是否克隆。
        clone_binding: bool,
    ) -> Result<TokenStream, Diagnostic> {
        // 简单字段转发可显式克隆所有权。
        if clone_binding {
            // 只特判单一标识符。
            if let ExpressionKind::Identifier(name) = &expression.expression.kind {
                // 查找调用方组件字段。
                if let Some(binding) = outer_bindings.get(name) {
                    // State 值 prop 必须读取当前快照，而不是克隆句柄本身。
                    if binding.kind == BindingKind::State {
                        // 读取必有的 State 句柄名称。
                        let state_name = binding
                            // 借用状态句柄。
                            .state_name
                            // State 类别必须始终拥有句柄。
                            .as_deref()
                            // 内部不变量失败时立即暴露。
                            .expect("State 绑定必须包含句柄");
                        // 恢复卫生 State 标识符。
                        let state_ident =
                            super::widget_expression_lower::ident_from_name(state_name);
                        // 值 prop 在当前构建位置读取并取得拥有型快照。
                        return Ok(quote! { (#state_ident).get() });
                    }
                    // 恢复字段标识符。
                    let ident = super::widget_expression_lower::ident_from_name(
                        // 借用卫生字段名称。
                        &binding.value_name,
                    );
                    // 返回拥有所有权的克隆值。
                    return Ok(quote! { (#ident).clone() });
                }
            }
        }
        // 克隆表达式以改写组件字段。
        let mut expanded = expression.expression.clone();
        // props 传值不能执行 setState。
        self.transform_expression(&mut expanded, outer_bindings, false, false)?;
        // 委托既有表达式生成器。
        generate_expression(&expanded, None)
    }
}

// 把语言基础类型映射为 Rust 类型。
// 类型映射入口已在 record_codegen 统一维护，这里删除重复实现。
