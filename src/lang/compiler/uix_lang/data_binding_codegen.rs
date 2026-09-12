// 引入过程宏标识符与令牌流。
use proc_macro2::{Ident, Span, TokenStream};
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入 Compiler System 唯一的数据构造登记表。
use crate::lang::compiler::projection_schema::UI_PROJECTION_SCHEMA;

// 引入表达式语法树节点。
use super::{Expression, ExpressionKind};

// 保存一个数据类型构造的完整生成规格。
pub(crate) struct DataConstructorSpec {
    // 保存公开 API 类型路径。
    pub(crate) path: TokenStream,
    // 保存公开构造函数名（None 表示 new）。
    pub(crate) method: Option<Ident>,
    // 标记整数参数是否规范化为小数形状。
    pub(crate) normalize_numbers: bool,
}

// 查找语言面类型名对应的公开构造规格。
pub(crate) fn data_constructor_spec(name: &str) -> Option<DataConstructorSpec> {
    if let Some(declaration) = crate::lang::compiler::components::data_declaration(name) {
        let path = syn::parse_str::<syn::Path>(&declaration.rust_path).ok()?;
        let method = syn::parse_str::<Ident>(&declaration.method).ok()?;
        return Some(DataConstructorSpec { path: quote! { #path }, method: Some(method), normalize_numbers: declaration.normalize_numbers });
    }
    let projection = UI_PROJECTION_SCHEMA.data_constructor(name)?;
    let path = syn::parse_str::<syn::Path>(projection.rust_path).ok()?;
    let method =
        (projection.method != "new").then(|| Ident::new(projection.method, Span::mixed_site()));
    Some(DataConstructorSpec {
        path: quote! { #path },
        method,
        normalize_numbers: projection.normalize_numbers,
    })
}

// 判断语言面类型名是否属于已登记数据类型。
pub(crate) fn is_registered_data_type(name: &str) -> bool {
    crate::lang::compiler::components::data_declaration(name).is_some() || UI_PROJECTION_SCHEMA.data_constructor(name).is_some()
}
// 把表达式树中的整数数字字面量规范化为小数形状。
pub(crate) fn normalize_number_literals(expression: &mut Expression) {
    // 递归访问当前节点。
    match &mut expression.kind {
        // action 块已在组件事件降低阶段分别规范化内部表达式。
        ExpressionKind::LoweredAction(_) => {}
        // 整数形状补充小数点。
        ExpressionKind::Number(source) => {
            // 只改写没有小数点或指数的整数形态。
            if !source.contains('.') && !source.contains('e') && !source.contains('E') {
                // 追加零小数以匹配 f32 参数。
                source.push_str(".0");
            }
        }
        // 一元表达式递归操作数。
        ExpressionKind::Unary { operand, .. } => normalize_number_literals(operand),
        // 二元表达式递归两侧。
        ExpressionKind::Binary { left, right, .. } => {
            // 规范化左侧。
            normalize_number_literals(left);
            // 规范化右侧。
            normalize_number_literals(right);
        }
        // 三元表达式递归条件与分支。
        ExpressionKind::Ternary {
            condition,
            then_branch,
            else_branch,
        } => {
            // 规范化条件。
            normalize_number_literals(condition);
            // 规范化真分支。
            normalize_number_literals(then_branch);
            // 规范化假分支。
            normalize_number_literals(else_branch);
        }
        // 成员访问递归对象。
        ExpressionKind::Member { object, .. } => normalize_number_literals(object),
        // 索引访问递归对象与下标。
        ExpressionKind::Index { object, index } => {
            // 规范化被索引对象。
            normalize_number_literals(object);
            // 规范化下标。
            normalize_number_literals(index);
        }
        // 调用递归目标与参数。
        ExpressionKind::Call { callee, arguments } => {
            // 规范化调用目标。
            normalize_number_literals(callee);
            // 规范化参数值。
            for argument in arguments {
                // 递归当前参数。
                normalize_number_literals(&mut argument.value);
            }
        }
        // 对象递归字段值。
        ExpressionKind::Object(fields) => {
            // 规范化字段值。
            for field in fields {
                // 递归当前字段。
                normalize_number_literals(&mut field.value);
            }
        }
        // 数组递归元素。
        ExpressionKind::Array(items) => {
            // 规范化元素。
            for item in items {
                // 递归当前元素。
                normalize_number_literals(item);
            }
        }
        // 受限闭包递归规范化唯一表达式体。
        ExpressionKind::Closure { body, .. } => normalize_number_literals(body),
        // 其余叶节点不需要规范化。
        ExpressionKind::Identifier(_) | ExpressionKind::String(_) | ExpressionKind::Boolean(_) => {}
    }
}

// 递归穿透成员与调用链，返回最内层标识符。
pub(crate) fn data_chain_root(expression: &Expression) -> Option<&str> {
    // 保存当前游标。
    let mut cursor = expression;
    // 循环穿透构造链结构。
    loop {
        // 按当前节点形状继续。
        match &cursor.kind {
            // 标识符是链的根。
            ExpressionKind::Identifier(name) => return Some(name.as_str()),
            // 成员访问继续穿透对象。
            ExpressionKind::Member { object, .. } => cursor = object,
            // 调用继续穿透目标。
            ExpressionKind::Call { callee, .. } => cursor = callee,
            // 其余结构不是数据构造链。
            _ => return None,
        }
    }
}

// 判断表达式是否属于已登记数据类型的构造链。
pub(crate) fn is_data_constructor_chain(expression: &Expression) -> bool {
    // 穿透到链根并核对构造白名单。
    data_chain_root(expression).is_some_and(is_registered_data_type)
}

// Step.status 的字符串语义值到公开枚举路径令牌映射。
