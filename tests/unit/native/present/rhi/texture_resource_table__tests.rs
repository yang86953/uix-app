// 引入共享尺寸、格式和句柄类型。
use super::{RhiTextureResource, RhiTextureResourceTable};
// 引入稳定错误分类。
use crate::core::Errc;
// 引入测试资源描述所需的共享 RHI 值对象。
use crate::platform::presentation::rhi::{
    RhiExtent, RhiTextureTransfer, TextureCopy, TextureDesc, TextureFormat, TextureHandle,
    TextureMove,
};

// 保存测试资源的共享描述事实。
struct TestTexture {
    // 保存不可变 texture 创建描述。
    desc: TextureDesc,
}

// 让测试资源满足共享 texture 资源表契约。
impl RhiTextureResource for TestTexture {
    // 返回资源创建时冻结的描述。
    fn desc(&self) -> TextureDesc {
        // 只读复制描述值，不暴露资源内部状态。
        self.desc
    }
}

// 验证两种颜色纹理可以提升而 coverage 纹理被共享门禁拒绝。
#[test]
fn resolves_only_renderable_texture_formats() {
    // 创建空的测试 texture 资源表。
    let mut table = RhiTextureResourceTable::new();
    // 登记共享颜色目标描述。
    let color = table.insert(TestTexture {
        // 使用两个 Adapter 都支持的 BGRA 目标格式。
        desc: TextureDesc::new(RhiExtent::new(4, 4), TextureFormat::Bgra8Unorm),
    });
    // 登记另一种共享颜色目标描述。
    let rgba_color = table.insert(TestTexture {
        // RGBA 必须与 BGRA 经过同一共享能力门禁。
        desc: TextureDesc::new(RhiExtent::new(4, 4), TextureFormat::Rgba8Unorm),
    });
    // 登记 sampled-only coverage 描述。
    let coverage = table.insert(TestTexture {
        // 使用不能作为 render target 的 R8 格式。
        desc: TextureDesc::new(RhiExtent::new(4, 4), TextureFormat::R8Unorm),
    });
    // 颜色纹理必须成功提升为 render target。
    assert!(table.resolve_render_target(color).is_ok());
    // RGBA 颜色纹理也必须成功提升为 render target。
    assert!(table.resolve_render_target(rgba_color).is_ok());
    // coverage 纹理必须在共享层返回参数错误。
    let error = table
        .resolve_render_target(coverage)
        .expect_err("coverage texture must not become a render target");
    // 错误分类不得依赖任一 Adapter。
    assert_eq!(error.code(), Errc::InvalidArgument);
}

// 验证陈旧 texture 身份在能力提升前被资源表拒绝。
#[test]
fn rejects_stale_texture_before_target_resolution() {
    // 创建没有资源的测试表。
    let table = RhiTextureResourceTable::<TestTexture>::new();
    // 不存在的句柄不能进入目标构造器。
    let error = table
        .resolve_render_target(TextureHandle::from_raw(1))
        .expect_err("stale texture must fail");
    // 共享资源表保持统一参数错误分类。
    assert_eq!(error.code(), Errc::InvalidArgument);
}

// 验证资源表在 copy/move 前统一解析描述并执行传输契约。
#[test]
fn validates_copy_and_move_against_shared_texture_descriptions() {
    // 创建空的测试 texture 资源表。
    let mut table = RhiTextureResourceTable::new();
    // 登记普通 BGRA 源纹理。
    let source = table.insert(TestTexture {
        // 使用四乘四的共享颜色描述。
        desc: TextureDesc::new(RhiExtent::new(4, 4), TextureFormat::Bgra8Unorm),
    });
    // 登记同格式目标纹理。
    let destination = table.insert(TestTexture {
        // 使用相同格式证明合法 copy。
        desc: TextureDesc::new(RhiExtent::new(4, 4), TextureFormat::Bgra8Unorm),
    });
    // 登记格式不同的目标纹理。
    let mismatched = table.insert(TestTexture {
        // 使用 RGBA 格式触发共享格式拒绝。
        desc: TextureDesc::new(RhiExtent::new(4, 4), TextureFormat::Rgba8Unorm),
    });
    // 构造四乘四的完整传输区域。
    let transfer = RhiTextureTransfer::from_xy(0, 0, 0, 0, RhiExtent::new(4, 4));
    // 不同同格式纹理的 copy 必须通过。
    assert!(
        table
            .validate_copy(TextureCopy::new(source, destination, transfer))
            .is_ok()
    );
    // 同一纹理的 move 必须通过共享移动语义。
    assert!(
        table
            .validate_move(TextureMove::new(source, source, transfer))
            .is_ok()
    );
    // 普通 copy 不得接受同一资源。
    assert!(
        table
            .validate_copy(TextureCopy::new(source, source, transfer))
            .is_err()
    );
    // 不同格式的 copy 必须在共享表中拒绝。
    assert!(
        table
            .validate_copy(TextureCopy::new(source, mismatched, transfer))
            .is_err()
    );
    // 陈旧源句柄必须在传输契约前被拒绝。
    assert!(
        table
            .validate_copy(TextureCopy::new(
                TextureHandle::from_raw(99),
                destination,
                transfer,
            ))
            .is_err()
    );
}
