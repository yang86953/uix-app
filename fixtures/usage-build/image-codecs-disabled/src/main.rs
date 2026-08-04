// 导入保留于基础绘制契约的图片服务公开入口。
use uix::prelude::ImageService;

// 提供 compile-fail fixture 的稳定入口。
fn main() {
    // 创建基础图片服务，证明类型本身不随编解码能力裁剪。
    let service = ImageService::new();
    // 故意调用未启用 capability 的方法，门禁要求此链无法编译。
    let result = service.load_from_bytes(&[]);
    // 若方法意外泄漏，消费结果会使门禁检测到编译成功并失败。
    drop(result);
    // 结束 fixture 入口。
}
