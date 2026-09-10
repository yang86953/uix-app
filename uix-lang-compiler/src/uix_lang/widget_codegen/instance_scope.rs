// 引入过程宏卫生标识符。
use proc_macro2::Ident;
// 引入确定性令牌拼接宏。
use quote::quote;

// 引入父模块拥有的组件展开器。
use super::WidgetExpander;
// 引入 UIX 元素语法树。
use super::super::Element;

// 实现 Widget 静态声明与动态实例作用域投影。
impl WidgetExpander {
    // 按调用位置与最近 For 路径准备可复用的组件状态作用域。
    pub(super) fn prepare_widget_scope(
        // 可变借用当前文档展开状态。
        &mut self,
        // 接收被调用组件的声明名称。
        widget_name: &str,
        // 接收保存静态调用位置的元素。
        element: &Element,
        // 标记调用是否属于 For 实际实例。
        inside_for: bool,
        // 标记组件是否拥有私有状态或状态样式。
        required: bool,
    ) -> Option<Ident> {
        // 无状态组件不申请运行时作用域。
        if !required {
            // 保持纯组合组件的零状态开销。
            return None;
        }
        // 为当前实际调用生成卫生的作用域局部变量名称。
        let scope_ident = self.fresh_ident("widget_scope", widget_name);
        // 使用组件调用标签与源码跨度形成稳定声明身份材料。
        let declaration_source = format!(
            // 保留组件标签与完整调用跨度，区分同类型的相邻静态调用。
            "{}:{}:{}",
            // 写入组件调用标签。
            element.name,
            // 写入调用开始偏移。
            element.span.start,
            // 写入调用结束偏移。
            element.span.end,
        );
        // 把声明身份材料压缩为运行时 API 约定的稳定无符号编号。
        let declaration_id = Self::stable_widget_id(&declaration_source);
        // For 内的组件先取得不依赖当前项的静态声明基座。
        if inside_for {
            // 为静态基座分配不会被实际实例覆盖的卫生名称。
            let base_scope_ident = self.fresh_ident("widget_scope_base", widget_name);
            // 静态基座必须在循环外只申请一次，避免出现序号随行顺序漂移。
            self.setup.push(quote! {
                // 为当前组件声明取得跨 reconcile 稳定的静态作用域基座。
                let #base_scope_ident = ::uix_app::ui::__private::uix_widget_scope(
                    // 由 Rust 宏调用点区分同一 UIX 文档的不同根工厂。
                    concat!(module_path!(), ":", file!(), ":", line!(), ":", column!()),
                    // 由 UIX 静态调用位置区分不同组件声明。
                    #declaration_id,
                );
            });
            // 最近 For 路径已经包含全部外层 key 或位置身份。
            let instance_path = self
                // 借用当前最内层实际实例路径。
                .for_path_stack
                // 组件调用位于 For 时路径栈必须非空。
                .last()
                // 克隆卫生标识符供逐迭代语句使用。
                .cloned()
                // 展开状态与路径栈不同步属于内部错误。
                .expect("For 内组件必须拥有实际实例路径");
            // 在所属 For 的每次迭代中从完整路径派生实际组件实例。
            self.push_setup(quote! {
                // key 或位置路径变化时建立新的组件私有状态身份。
                let #scope_ident = ::uix_app::ui::__private::uix_widget_child_scope(
                    // 继承静态调用点、声明与捕获命名空间。
                    &#base_scope_ident,
                    // 继续保留组件调用声明维度。
                    #declaration_id,
                    // 复制当前完整嵌套 For 实例路径。
                    (#instance_path).clone(),
                );
            });
        } else {
            // 静态组件在最终 View 之前只申请一次完整作用域。
            self.push_setup(quote! {
                // 为当前静态组件调用取得可跨 reconcile 复用的私有状态作用域。
                let #scope_ident = ::uix_app::ui::__private::uix_widget_scope(
                    // 由 Rust 宏调用点区分同一 UIX 文档的不同根工厂。
                    concat!(module_path!(), ":", file!(), ":", line!(), ":", column!()),
                    // 由 UIX 静态调用位置区分同一组件的多个实例。
                    #declaration_id,
                );
            });
        }
        // 返回后续 state 初始化和 View 标记共用的作用域局部变量。
        Some(scope_ident)
    }
}
