// 引入共享纹理区域、传输命令和资源描述。
use crate::platform::presentation::rhi::{
    RhiExtent, RhiTextureRegion, RhiTextureTransfer, TextureCopy, TextureDesc, TextureFormat,
    TextureHandle, TextureMove,
};

// 创建稳定的源纹理身份。
const SOURCE: TextureHandle = TextureHandle::from_raw(1);
// 创建稳定的目标纹理身份。
const DESTINATION: TextureHandle = TextureHandle::from_raw(2);

// 创建指定尺寸和格式的共享纹理描述。
const fn texture(width: u32, height: u32, format: TextureFormat) -> TextureDesc {
    // 返回没有原生 API 状态的资源事实。
    TextureDesc::new(RhiExtent::new(width, height), format)
}

// 创建覆盖源与目标不同偏移的有效传输。
const fn valid_transfer() -> RhiTextureTransfer {
    // 返回非空且落在十六像素纹理内的传输。
    RhiTextureTransfer::from_xy(
        // 从源第二列开始。
        1,
        // 从源第三行开始。
        2,
        // 写入目标第四列。
        3,
        // 写入目标第五行。
        4,
        // 传输六列和七行。
        RhiExtent::new(6, 7),
    )
}

// 创建有效普通复制。
const fn valid_copy() -> TextureCopy {
    // 组合不同资源与唯一传输几何。
    TextureCopy::new(SOURCE, DESTINATION, valid_transfer())
}

// 验证区域门禁统一返回原生矩形与紧密载荷布局。
#[test]
fn region_returns_checked_native_bounds_and_payload_layout() {
    // 创建带偏移的有效上传区域。
    let region = RhiTextureRegion::from_xy(3, 4, RhiExtent::new(6, 7));
    // 使用十六像素纹理验证区域。
    let bounds = region
        // 进入唯一共享边界门禁。
        .validate_within(RhiExtent::new(16, 16))
        // 有效区域必须通过。
        .expect("region should be valid");
    // 无符号矩形必须保持左上原点与 checked 远端边界。
    assert_eq!(bounds.native_rect_u32(), (3, 4, 9, 11));
    // OpenGL 投影必须保持相同原点与尺寸。
    assert_eq!(bounds.native_origin_and_size_i32(), ((3, 4), (6, 7)));
    // 四字节像素必须得到二十四字节行跨度和一百六十八字节载荷。
    assert_eq!(bounds.tight_payload_layout(4), Some((168, 24)));
}

// 验证共享门禁返回两端不可拆分区域。
#[test]
fn copy_returns_checked_typed_regions() {
    // 使用同格式颜色纹理验证复制。
    let bounds = valid_copy()
        // 两端都使用十六像素 BGRA 颜色资源。
        .validate_transfer(
            texture(16, 16, TextureFormat::Bgra8Unorm),
            texture(16, 16, TextureFormat::Bgra8Unorm),
        )
        // 有效区域必须通过。
        .expect("copy should be valid");
    // 源区域必须保持一、二原点和六、七尺寸。
    assert_eq!(bounds.source().native_rect_u32(), (1, 2, 7, 9));
    // 目标区域必须共享同一尺寸并保持三、四原点。
    assert_eq!(bounds.destination().native_rect_u32(), (3, 4, 9, 11));
}

// 验证两个 Adapter 对空区域、跨格式和 R8 使用相同拒绝结果。
#[test]
fn copy_rejects_divergent_format_and_extent_cases() {
    // 创建基础有效 copy。
    let copy = valid_copy();
    // 跨格式复制必须失败。
    assert!(
        copy.validate_transfer(
            // 源使用 BGRA。
            texture(16, 16, TextureFormat::Bgra8Unorm),
            // 目标使用 RGBA。
            texture(16, 16, TextureFormat::Rgba8Unorm),
        )
        // 要求返回失败。
        .is_err()
    );
    // R8 coverage 没有共同 render-target copy 基线。
    assert!(
        copy.validate_transfer(
            // 源使用 R8。
            texture(16, 16, TextureFormat::R8Unorm),
            // 目标也使用 R8。
            texture(16, 16, TextureFormat::R8Unorm),
        )
        // 要求返回失败。
        .is_err()
    );
    // 构造零宽传输。
    let empty = TextureCopy::new(
        // 保持源资源。
        SOURCE,
        // 保持目标资源。
        DESTINATION,
        // 只把唯一尺寸改成零宽。
        RhiTextureTransfer::from_xy(1, 2, 3, 4, RhiExtent::new(0, 7)),
    );
    // 空 copy 必须失败而不是在某个 Adapter 中静默成功。
    assert!(
        empty
            // 使用两端相同颜色描述。
            .validate_transfer(
                texture(16, 16, TextureFormat::Bgra8Unorm),
                texture(16, 16, TextureFormat::Bgra8Unorm),
            )
            // 要求返回失败。
            .is_err()
    );
}

// 验证普通 copy 拒绝同资源，而 move 明确允许并正确拆分 scratch。
#[test]
fn same_resource_move_preserves_transfer_through_scratch() {
    // 构造同纹理普通 copy。
    let copy = TextureCopy::new(SOURCE, SOURCE, valid_transfer());
    // 普通 copy 不能依赖驱动处理同资源行为。
    assert!(
        copy.validate_transfer(
            // 源描述。
            texture(16, 16, TextureFormat::Bgra8Unorm),
            // 目标描述。
            texture(16, 16, TextureFormat::Bgra8Unorm),
        )
        // 要求返回失败。
        .is_err()
    );
    // 构造同纹理重叠安全 move。
    let movement = TextureMove::new(SOURCE, SOURCE, valid_transfer());
    // move 必须允许同资源，由 Adapter 的 scratch 流程保证重叠安全。
    movement
        // 两端描述相同。
        .validate_transfer(
            texture(16, 16, TextureFormat::Bgra8Unorm),
            texture(16, 16, TextureFormat::Bgra8Unorm),
        )
        // 要求验证成功。
        .expect("same-resource move should be valid");
    // 使用独立临时纹理拆分移动。
    let (to_scratch, from_scratch) = movement.through_scratch(DESTINATION);
    // 第一段必须保留原始源区域并写到 scratch 零点。
    assert_eq!(
        to_scratch.transfer().source().origin(),
        valid_transfer().source().origin()
    );
    // 第二段必须从 scratch 零点恢复到原始目标区域。
    assert_eq!(
        from_scratch.transfer().destination().origin(),
        valid_transfer().destination().origin()
    );
    // 两段必须共享完全相同的唯一尺寸。
    assert_eq!(
        (
            to_scratch.transfer().extent(),
            from_scratch.transfer().extent()
        ),
        (RhiExtent::new(6, 7), RhiExtent::new(6, 7))
    );
}

// 验证任何坐标回绕或越界都在原生 API 前失败。
#[test]
fn transfer_rejects_overflow_and_out_of_bounds() {
    // 构造源横坐标会回绕的 copy。
    let overflow = TextureCopy::new(
        // 使用独立源资源。
        SOURCE,
        // 使用独立目标资源。
        DESTINATION,
        // 最大横坐标加正宽度必然溢出。
        RhiTextureTransfer::from_xy(u32::MAX, 2, 3, 4, RhiExtent::new(2, 7)),
    );
    // 溢出必须被共享门禁拒绝。
    assert!(
        overflow
            // 使用有效纹理尺寸证明失败来自坐标加法而不是资源描述。
            .validate_transfer(
                texture(16, 16, TextureFormat::Bgra8Unorm),
                texture(16, 16, TextureFormat::Bgra8Unorm),
            )
            // 要求返回失败。
            .is_err()
    );
    // 构造目标区域越过纹理底边的 copy。
    let outside = TextureCopy::new(
        // 使用独立源资源。
        SOURCE,
        // 使用独立目标资源。
        DESTINATION,
        // 从目标第十二行开始且高度七会越界。
        RhiTextureTransfer::from_xy(1, 2, 3, 12, RhiExtent::new(6, 7)),
    );
    // 越界必须被共享门禁拒绝。
    assert!(
        outside
            // 使用两端相同颜色描述。
            .validate_transfer(
                texture(16, 16, TextureFormat::Bgra8Unorm),
                texture(16, 16, TextureFormat::Bgra8Unorm),
            )
            // 要求返回失败。
            .is_err()
    );
}
