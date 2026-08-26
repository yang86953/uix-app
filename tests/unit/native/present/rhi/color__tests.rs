// 引入当前模块与父 RHI 的私有值对象。
use super::*;
// 引入 Device capability，测试颜色路径进入正确角色的准入条件。
use crate::platform::presentation::rhi::GraphicsDeviceCapabilities;
// 引入构造 surface token 所需的通用 extent。
use crate::platform::presentation::rhi::RhiExtent;

// 锁定全部颜色目标共享一个编码与混合域。
#[test]
fn color_textures_surface_and_capabilities_share_one_contract() {
    // BGRA 颜色纹理必须采用统一颜色路径。
    assert_eq!(
        // 读取 BGRA 的完整颜色解释。
        TextureFormat::Bgra8Unorm.color_contract(),
        // 颜色纹理必须报告唯一契约。
        Some(UIX_COLOR_CONTRACT)
    );
    // RGBA 颜色纹理不得建立第二套颜色解释。
    assert_eq!(
        // 读取 RGBA 的完整颜色解释。
        TextureFormat::Rgba8Unorm.color_contract(),
        // 通道顺序不同不改变颜色语义。
        Some(UIX_COLOR_CONTRACT)
    );
    // 覆盖率纹理必须保持非颜色资源身份。
    assert_eq!(TextureFormat::R8Unorm.color_contract(), None);
    // 构造一个不依赖原生 API 的 surface 身份。
    let surface = SurfaceToken::new(1, RhiExtent::new(16, 16));
    // Surface 必须与离屏颜色纹理共享颜色路径。
    assert_eq!(surface.color_contract(), UIX_COLOR_CONTRACT);
    // 通用 GPU 基线也必须把同一颜色事实交给 Adapter。
    assert_eq!(
        // 读取基线能力中的颜色契约。
        GraphicsDeviceCapabilities::full_gpu_baseline().color_contract,
        // 能力快照不得另行解释颜色。
        UIX_COLOR_CONTRACT
    );
    // 统一颜色契约必须进入设备基线的运行时准入条件。
    assert!(GraphicsDeviceCapabilities::full_gpu_baseline().has_gpu_baseline());
    // 整目标与局部清理必须共享同一输出状态常量。
    assert!(UIX_COLOR_CLEAR_CONTRACT.is_uix_contract());
    // 清理必须完整写入全部颜色通道。
    assert_eq!(
        UIX_COLOR_CLEAR_CONTRACT.write_mask,
        PipelineColorWriteMask::All
    );
    // 清理必须关闭会改变八位颜色最低位的抖动。
    assert_eq!(
        UIX_COLOR_CLEAR_CONTRACT.dither,
        PipelineDitherState::Disabled
    );
}
