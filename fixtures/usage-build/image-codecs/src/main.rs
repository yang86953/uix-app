// 导入保留于基础绘制契约的图片服务公开入口。
use uix::prelude::ImageService;

// 提供可执行 fixture 的稳定入口。
fn main() {
    // 创建独立图片服务以触发公开字节解码路径的编译。
    let service = ImageService::new();
    // 使用黑盒输入避免 release 优化提前消除解码调用。
    let result = service.load_from_bytes(std::hint::black_box(&[]));
    // 显式消费结果，确保能力实现进入使用方 release 产物。
    let _ = std::hint::black_box(result);
    // 结束 fixture 入口。
}
