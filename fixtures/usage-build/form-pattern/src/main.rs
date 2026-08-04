// 导入基础表单公开入口。
use uix::prelude::Form;

// 提供可执行 fixture 的稳定入口。
fn main() {
    // 构造使用正则 pattern 规则的表单模型。
    let form = Form::new()
        .field("code", "编码")
        .initial("UIX-42")
        .validate_pattern(r"^UIX-[0-9]+$", "编码格式错误")
        .build();
    // 显式消费模型，确保公开 builder 方法进入编译。
    drop(form);
}
