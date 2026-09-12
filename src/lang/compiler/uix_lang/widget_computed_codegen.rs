// 引入确定性令牌拼接宏。
use quote::quote;

// 引入组件展开器与字段绑定类型。
use super::widget_codegen::{Binding, BindingKind, Bindings, WidgetExpander};
// 引入派生声明与诊断 AST。
use super::{Diagnostic, WidgetComputed};
// 引入受限表达式生成入口。
use super::generate_expression;

// 实现有序 computed 派生值的静态局部绑定。
impl WidgetExpander {
    // 按源码顺序生成全部无缓存派生值。
    pub(super) fn emit_computed_bindings(
        // 可变借用组件展开状态。
        &mut self,
        // 接收源码顺序中的派生声明。
        computed: &[WidgetComputed],
        // 接收并扩展当前组件字段绑定表。
        bindings: &mut Bindings,
    ) -> Result<(), Diagnostic> {
        // 依次求值，使每项只能读取 props、state 与先前派生值。
        for (index, derived) in computed.iter().enumerate() {
            // 收集当前项与全部后续项，供前向引用诊断使用。
            let pending = computed[index..]
                // 遍历尚未建立绑定的派生声明。
                .iter()
                // 复制派生名称。
                .map(|item| item.name.clone())
                // 由目标栈字段类型推断为确定性名称集合。
                .collect();
            // 压入本次表达式不可见的派生名称集合。
            self.pending_computed_scope_stack.push(pending);
            // 克隆表达式以保留声明 AST 供重复组件调用展开。
            let mut expanded = derived.expression.clone();
            // computed 只能读取字段和外部符号，不能更新状态。
            let transform_result = self.transform_expression(
                // 改写派生表达式中的已声明绑定。
                &mut expanded,
                // 仅提供当前时刻已建立的字段与派生绑定。
                bindings,
                // 禁止派生值执行 setState。
                false,
                // 普通值位置读取 State 当前值。
                false,
            );
            // 无论改写成功或失败都恢复外层 computed 上下文。
            self.pending_computed_scope_stack.pop();
            // 传播自引用、前向引用或外部依赖诊断。
            transform_result?;
            // 把完成改写的表达式生成 Rust 值令牌。
            let value = generate_expression(&expanded, None)?;
            // 为派生局部值分配卫生名称。
            let value_ident = self.fresh_ident("computed", &derived.name);
            // 每次组件 View 展开都按顺序重新计算派生值。
            self.push_setup(quote! {
                // 建立当前无缓存派生局部变量。
                let #value_ident = #value;
            });
            // 使后续 computed 与组件体能够读取当前派生值。
            bindings.insert(
                // 使用 UIX 派生名称作为查找键。
                derived.name.clone(),
                // 保存普通只读值绑定。
                Binding {
                    // 记录卫生 Rust 局部名称。
                    value_name: value_ident.to_string(),
                    // computed 不是可写 State 句柄。
                    state_name: None,
                    // computed 使用普通值克隆语义。
                    kind: BindingKind::Value,
                    // 派生表达式沿用组件数字规范化规则。
                    authored_numbers: false,
                },
            );
        }
        // 报告全部派生绑定生成成功。
        Ok(())
    }
}
