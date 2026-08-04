// 导入基础表单公开入口；该类型本身不随 pattern capability 裁剪。
use uix::prelude::Form;

// 提供 compile-fail fixture 的稳定入口。
fn main() {
    // 故意调用未启用 capability 的方法，门禁要求此链无法编译。
    let form = Form::new()
        .field("code", "编码")
        .validate_pattern(r"^UIX-[0-9]+$", "编码格式错误")
        .build();
    // 若方法意外泄漏，消费模型会使门禁检测到编译成功并失败。
    drop(form);
}
