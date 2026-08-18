//! 图形 API 无关的 RHI 纹理传输契约。
//!
//! 复制与移动统一使用左上原点、非空整数区域和同格式可渲染颜色纹理；
//! Adapter 只把已经验证的边界机械编码到原生 copy 命令。

// 引入统一错误码、错误值和结果类型。
use crate::core::error::{Errc, Error, Result};

// 引入共享纹理描述、传输命令和格式。
use super::{TextureCopy, TextureDesc, TextureFormat, TextureMove};

// 保存一次已经验证且不会溢出的纹理传输远端边界。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RhiTextureTransferBounds {
    // 保存源区域不包含的右边界。
    source_right: u32,
    // 保存源区域不包含的底边界。
    source_bottom: u32,
    // 保存目标区域不包含的右边界。
    destination_right: u32,
    // 保存目标区域不包含的底边界。
    destination_bottom: u32,
}

// 为原生 Adapter 提供只读的已验证边界。
impl RhiTextureTransferBounds {
    // 返回源区域不包含的右边界。
    pub(crate) const fn source_right(self) -> u32 {
        // 返回共享 checked_add 得到的结果。
        self.source_right
    }

    // 返回源区域不包含的底边界。
    pub(crate) const fn source_bottom(self) -> u32 {
        // 返回共享 checked_add 得到的结果。
        self.source_bottom
    }

    // 返回目标区域不包含的右边界。
    pub(crate) const fn destination_right(self) -> u32 {
        // 返回共享 checked_add 得到的结果。
        self.destination_right
    }

    // 返回目标区域不包含的底边界。
    pub(crate) const fn destination_bottom(self) -> u32 {
        // 返回共享 checked_add 得到的结果。
        self.destination_bottom
    }
}

// 保存 copy 与 move 共用的坐标和范围输入。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RhiTextureTransferInput {
    // 保存源区域左边界。
    source_x: u32,
    // 保存源区域顶边界。
    source_y: u32,
    // 保存目标区域左边界。
    destination_x: u32,
    // 保存目标区域顶边界。
    destination_y: u32,
    // 保存传输宽度。
    width: u32,
    // 保存传输高度。
    height: u32,
}

// 为共享输入提供唯一边界算法。
impl RhiTextureTransferInput {
    // 验证格式、非空范围和两端边界。
    fn validate(
        // 只读消费不可变传输输入。
        self,
        // 接收源纹理的共享资源事实。
        source: TextureDesc,
        // 接收目标纹理的共享资源事实。
        destination: TextureDesc,
    ) -> Result<RhiTextureTransferBounds> {
        // 两端必须使用相同像素格式，禁止驱动隐式转换。
        if source.format != destination.format {
            // 返回稳定格式错误。
            return Err(invalid_transfer("RHI texture transfer formats differ"));
        }
        // 当前跨 Adapter 基线只承诺可渲染颜色纹理传输。
        if !is_transferable_color(source.format) {
            // R8 覆盖率纹理没有 OpenGL framebuffer，不能伪造跨后端支持。
            return Err(invalid_transfer(
                "RHI texture transfer requires a renderable color format",
            ));
        }
        // 空区域在不同原生 API 中有 no-op 与错误两种行为，统一显式拒绝。
        if self.width == 0 || self.height == 0 {
            // 返回稳定非空范围错误。
            return Err(invalid_transfer("RHI texture transfer extent is empty"));
        }
        // 使用 checked_add 计算源右边界，禁止饱和运算隐藏溢出。
        let source_right = self
            // 从共享左上原点横坐标开始。
            .source_x
            // 加上传输宽度。
            .checked_add(self.width)
            // 把整数溢出转换成参数错误。
            .ok_or_else(|| invalid_transfer("RHI texture transfer source x overflows"))?;
        // 使用 checked_add 计算源底边界。
        let source_bottom = self
            // 从共享左上原点纵坐标开始。
            .source_y
            // 加上传输高度。
            .checked_add(self.height)
            // 把整数溢出转换成参数错误。
            .ok_or_else(|| invalid_transfer("RHI texture transfer source y overflows"))?;
        // 使用 checked_add 计算目标右边界。
        let destination_right = self
            // 从共享目标横坐标开始。
            .destination_x
            // 加上传输宽度。
            .checked_add(self.width)
            // 把整数溢出转换成参数错误。
            .ok_or_else(|| invalid_transfer("RHI texture transfer destination x overflows"))?;
        // 使用 checked_add 计算目标底边界。
        let destination_bottom = self
            // 从共享目标纵坐标开始。
            .destination_y
            // 加上传输高度。
            .checked_add(self.height)
            // 把整数溢出转换成参数错误。
            .ok_or_else(|| invalid_transfer("RHI texture transfer destination y overflows"))?;
        // 源区域必须完整落在源纹理内。
        if source_right > source.extent.width || source_bottom > source.extent.height {
            // 返回稳定源范围错误。
            return Err(invalid_transfer(
                "RHI texture transfer source is outside extent",
            ));
        }
        // 目标区域必须完整落在目标纹理内。
        if destination_right > destination.extent.width
            // 同时检查纵向底边界。
            || destination_bottom > destination.extent.height
        {
            // 返回稳定目标范围错误。
            return Err(invalid_transfer(
                "RHI texture transfer destination is outside extent",
            ));
        }
        // 返回两个 Adapter 直接消费的已验证边界。
        Ok(RhiTextureTransferBounds {
            // 保存源右边界。
            source_right,
            // 保存源底边界。
            source_bottom,
            // 保存目标右边界。
            destination_right,
            // 保存目标底边界。
            destination_bottom,
        })
    }
}

// 为普通 copy 提供唯一共享验证入口。
impl TextureCopy {
    // 验证不同纹理之间的确定性复制。
    pub(crate) fn validate_transfer(
        // 复制命令按值进入共享门禁。
        self,
        // 接收源纹理描述。
        source: TextureDesc,
        // 接收目标纹理描述。
        destination: TextureDesc,
    ) -> Result<RhiTextureTransferBounds> {
        // 同一纹理必须走具有 scratch/memmove 语义的 TextureMove。
        if self.source == self.destination {
            // 禁止依赖原生 API 对同资源 copy 的未定义行为。
            return Err(invalid_transfer(
                "RHI texture copy requires different resources",
            ));
        }
        // 复用 copy 与 move 唯一的范围算法。
        RhiTextureTransferInput {
            // 复制源横坐标。
            source_x: self.source_x,
            // 复制源纵坐标。
            source_y: self.source_y,
            // 复制目标横坐标。
            destination_x: self.destination_x,
            // 复制目标纵坐标。
            destination_y: self.destination_y,
            // 复制宽度。
            width: self.width,
            // 复制高度。
            height: self.height,
        }
        // 验证两个资源事实。
        .validate(source, destination)
    }
}

// 为重叠安全 move 提供唯一共享验证入口。
impl TextureMove {
    // 验证同纹理或不同纹理的确定性区域移动。
    pub(crate) fn validate_transfer(
        // 移动命令按值进入共享门禁。
        self,
        // 接收源纹理描述。
        source: TextureDesc,
        // 接收目标纹理描述。
        destination: TextureDesc,
    ) -> Result<RhiTextureTransferBounds> {
        // move 允许同资源，具体 Adapter 必须用 scratch 保证重叠安全。
        RhiTextureTransferInput {
            // 移动源横坐标。
            source_x: self.source_x,
            // 移动源纵坐标。
            source_y: self.source_y,
            // 移动目标横坐标。
            destination_x: self.destination_x,
            // 移动目标纵坐标。
            destination_y: self.destination_y,
            // 移动宽度。
            width: self.width,
            // 移动高度。
            height: self.height,
        }
        // 验证两个资源事实。
        .validate(source, destination)
    }
}

// 判断格式是否属于两个 Adapter 都能直接 copy 的颜色基线。
const fn is_transferable_color(format: TextureFormat) -> bool {
    // BGRA 与 RGBA 都有 render-target view；R8 coverage 当前只有 sampled view。
    matches!(
        format,
        // retained surface 使用 BGRA。
        TextureFormat::Bgra8Unorm
            // MSDF 和其它颜色纹理使用 RGBA。
            | TextureFormat::Rgba8Unorm
    )
}

// 构造 API 无关的纹理传输参数错误。
fn invalid_transfer(message: &'static str) -> Error {
    // 所有共享格式和范围违例都属于调用参数错误。
    Error::new(Errc::InvalidArgument, message)
}

// 仅验证两个 Adapter 必须共享的纹理传输语义。
#[cfg(test)]
mod tests {
    // 引入共享纹理传输命令和资源描述。
    use crate::native::present::rhi::{
        RhiExtent, TextureCopy, TextureDesc, TextureFormat, TextureHandle, TextureMove,
    };

    // 创建稳定的源纹理身份。
    const SOURCE: TextureHandle = TextureHandle::from_raw(1);
    // 创建稳定的目标纹理身份。
    const DESTINATION: TextureHandle = TextureHandle::from_raw(2);

    // 创建指定尺寸和格式的共享纹理描述。
    const fn texture(width: u32, height: u32, format: TextureFormat) -> TextureDesc {
        // 返回没有原生 API 状态的资源事实。
        TextureDesc {
            // 保存物理尺寸。
            extent: RhiExtent::new(width, height),
            // 保存共享格式。
            format,
        }
    }

    // 创建覆盖源与目标不同偏移的有效 copy。
    const fn valid_copy() -> TextureCopy {
        // 返回非空且落在十六像素纹理内的复制。
        TextureCopy {
            // 使用独立源资源。
            source: SOURCE,
            // 使用独立目标资源。
            destination: DESTINATION,
            // 从源第二列开始。
            source_x: 1,
            // 从源第三行开始。
            source_y: 2,
            // 写入目标第四列。
            destination_x: 3,
            // 写入目标第五行。
            destination_y: 4,
            // 复制六列。
            width: 6,
            // 复制七行。
            height: 7,
        }
    }

    // 验证共享门禁返回不会溢出的远端边界。
    #[test]
    fn copy_returns_checked_top_left_bounds() {
        // 创建有效 copy。
        let copy = valid_copy();
        // 使用同格式颜色纹理验证复制。
        let bounds = copy
            // 两端都使用十六像素 BGRA 颜色资源。
            .validate_transfer(
                texture(16, 16, TextureFormat::Bgra8Unorm),
                texture(16, 16, TextureFormat::Bgra8Unorm),
            )
            // 有效区域必须通过。
            .expect("copy should be valid");
        // 源右边界必须等于一加六。
        assert_eq!(bounds.source_right(), 7);
        // 源底边界必须等于二加七。
        assert_eq!(bounds.source_bottom(), 9);
        // 目标右边界必须等于三加六。
        assert_eq!(bounds.destination_right(), 9);
        // 目标底边界必须等于四加七。
        assert_eq!(bounds.destination_bottom(), 11);
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
        // 构造零宽 copy。
        let empty = TextureCopy {
            // 只覆盖基础 copy 的宽度。
            width: 0,
            // 复用其它合法字段。
            ..copy
        };
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

    // 验证普通 copy 拒绝同资源，而 move 明确允许同资源。
    #[test]
    fn same_resource_requires_move_semantics() {
        // 构造同纹理普通 copy。
        let copy = TextureCopy {
            // 目标改成源资源。
            destination: SOURCE,
            // 复用其它合法字段。
            ..valid_copy()
        };
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
        let movement = TextureMove {
            // 使用同一源纹理。
            source: SOURCE,
            // 使用同一目标纹理。
            destination: SOURCE,
            // 从源第二列开始。
            source_x: 1,
            // 从源第三行开始。
            source_y: 2,
            // 移动到相邻列。
            destination_x: 2,
            // 移动到相邻行。
            destination_y: 3,
            // 移动六列。
            width: 6,
            // 移动七行。
            height: 7,
        };
        // move 必须允许同资源，由 Adapter 的 scratch 流程保证重叠安全。
        movement
            // 两端描述相同。
            .validate_transfer(
                texture(16, 16, TextureFormat::Bgra8Unorm),
                texture(16, 16, TextureFormat::Bgra8Unorm),
            )
            // 要求验证成功。
            .expect("same-resource move should be valid");
    }

    // 验证任何坐标回绕或越界都在原生 API 前失败。
    #[test]
    fn transfer_rejects_overflow_and_out_of_bounds() {
        // 构造源横坐标会回绕的 copy。
        let overflow = TextureCopy {
            // 使用最大横坐标。
            source_x: u32::MAX,
            // 保持正宽度触发 checked_add 溢出。
            width: 2,
            // 复用其它合法字段。
            ..valid_copy()
        };
        // 溢出必须被共享门禁拒绝。
        assert!(
            overflow
                // 使用最大尺寸也不能绕过整数溢出。
                .validate_transfer(
                    texture(u32::MAX, 16, TextureFormat::Bgra8Unorm),
                    texture(16, 16, TextureFormat::Bgra8Unorm),
                )
                // 要求返回失败。
                .is_err()
        );
        // 构造目标区域越过纹理底边的 copy。
        let outside = TextureCopy {
            // 从目标第十二行开始。
            destination_y: 12,
            // 高度七会越过十六行。
            height: 7,
            // 复用其它合法字段。
            ..valid_copy()
        };
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
}
