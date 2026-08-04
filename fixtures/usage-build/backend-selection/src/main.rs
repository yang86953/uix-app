// 导入应用 builder 与受 backend feature 门控的公开选择枚举。
use uix::prelude::{App, GraphicsBackend};

// D3D12 正反配置都故意引用同一个公开变体。
#[cfg(any(feature = "d3d12-selection", feature = "d3d12-selection-disabled"))]
// 返回当前配置要求验证的 D3D12 公开选择。
fn selected_backend() -> GraphicsBackend {
    // 正向配置应编译，负向配置应在该变体处失败。
    GraphicsBackend::Direct3D12
}

// Vulkan 正反配置都故意引用同一个公开变体。
#[cfg(any(feature = "vulkan-selection", feature = "vulkan-selection-disabled"))]
// 返回当前配置要求验证的 Vulkan 公开选择。
fn selected_backend() -> GraphicsBackend {
    // 正向配置应编译并解析 ash，负向配置应在该变体处失败。
    GraphicsBackend::Vulkan
}

// Metal 正反配置都故意引用同一个公开变体。
#[cfg(any(feature = "metal-selection", feature = "metal-selection-disabled"))]
// 返回当前配置要求验证的 Metal 公开选择。
fn selected_backend() -> GraphicsBackend {
    // 正向配置应编译且不新增 package，负向配置应在该变体处失败。
    GraphicsBackend::Metal
}

// 提供六个互斥配置共用的稳定可执行入口。
fn main() {
    // 把公开 backend 选择传给真实应用 builder，覆盖使用方调用形态。
    let app = App::new().graphics_backend(selected_backend());
    // 显式消费应用值，避免未使用警告干扰构建证据。
    drop(app);
}
