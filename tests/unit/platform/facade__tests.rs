// 引入本模块私有分派组件和公开错误码。
use super::{Errc, GraphicsBackend, enumerate_gpu_adapters};

// 纯 OpenGL ES 构建必须返回稳定的类型化未实现错误。
#[test]
fn opengles_only_adapter_enumeration_is_typed_not_implemented() {
    // 调用无状态分派组件，不创建窗口、设备或 Platform 单例。
    let result = enumerate_gpu_adapters(GraphicsBackend::OpenGlEs);
    // 同时约束失败类型和禁止伪造空成功结果。
    assert!(matches!(result, Err(error) if error.code() == Errc::NotImplemented));
}
