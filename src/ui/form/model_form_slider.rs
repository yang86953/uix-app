//! 类型化表单滑块字段适配。

// 引入统一类型化字段契约。
use super::model_form::FormItemSpec;
// 引入声明式滑块字段与底层表单模型。
use crate::ui::form::form_binding::FormSliderItem;
// 引入统一表单字段状态模型。
use crate::ui::form::form_validation::FormModel;
// 引入公开视图构建契约。
use crate::ui::view::{View, ViewNode};
// 引入 f64 字段受控状态。
use crate::ui::State;

// 把声明式滑块字段接入类型化 f64 投影契约。
impl FormItemSpec<f64> for FormSliderItem {
    // 绑定统一表单状态并构建滑块字段 View。
    fn bind_view(&self, form: &FormModel, value: &State<f64>) -> ViewNode {
        // 通过低层适配器建立字段状态、重置与焦点绑定。
        let mut bound = form
            // 把声明范围与类型化字段状态交给表单 Module。
            .slider_item(&self.field, value, self.range.clone())
            // 字段未登记时给出稳定开发者诊断。
            .unwrap_or_else(|| panic!("Form::model 字段未在内部模型登记"));
        // 投影独立 FormItem 标签。
        bound.label = self.label.clone();
        // 投影滑块步进约束。
        bound.step = self.step;
        // 投影控件尺寸。
        bound.size = self.size;
        // 投影错误文本显示策略。
        bound.show_error = self.show_error;
        // 构建绑定后的字段 View。
        bound.build()
    }

    // 返回面向用户的表单标签。
    fn label<'a>(&'a self, field: &'a str) -> &'a str {
        // 独立标签优先，否则沿用稳定字段 key。
        self.label.as_deref().unwrap_or(field)
    }
}
