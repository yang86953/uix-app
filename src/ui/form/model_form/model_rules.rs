// 引入父模块的类型化表单构建器。
use super::ModelFormBuilder;
// 引入统一字段错误类型以保持既有提交返回契约。
use crate::ui::form::form_validation::FieldError;

// 保存一个读取完整候选模型的类型化规则。
pub(super) type ModelRule<M> = Box<dyn Fn(&M) -> Result<(), String>>;

// 为类型化表单构建器登记全模型规则。
impl<M: Clone + Send + Sync + 'static> ModelFormBuilder<M> {
    /// 追加一个在字段校验与候选模型组装后执行的全模型规则。
    pub fn model_rule<F>(mut self, validator: F) -> Self
    where
        // 规则只读取候选模型并返回用户可见错误。
        F: Fn(&M) -> Result<(), String> + 'static,
    {
        // 保留源码或调用链声明顺序。
        self.model_rules.push(Box::new(validator));
        // 返回可继续组装的构建器。
        self
    }
}

// 按声明顺序执行全部全模型规则并收集失败。
pub(super) fn validate_model_rules<M>(
    // 接收构建期登记的规则序列。
    rules: &[ModelRule<M>],
    // 接收字段值组装后的候选模型。
    model: &M,
) -> Result<(), Vec<FieldError>> {
    // 收集全部规则错误以保持字段校验的多错误返回形态。
    let errors = rules
        // 按声明顺序遍历规则。
        .iter()
        // 只保留失败规则并转换成统一错误。
        .filter_map(|rule| rule(model).err())
        // 使用稳定虚拟字段名区分整表错误。
        .map(|message| FieldError::new("$form", message))
        // 物化有序错误集合。
        .collect::<Vec<_>>();
    // 没有错误时允许提交继续。
    if errors.is_empty() {
        // 返回成功状态。
        Ok(())
    } else {
        // 返回全部全模型规则错误。
        Err(errors)
    }
}
