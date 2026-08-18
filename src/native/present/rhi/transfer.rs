//! 图形 API 无关的 RHI 纹理区域与传输契约。
//!
//! 上传、复制与移动统一使用左上原点、非空整数区域和共同原生值域；
//! Adapter 只把已经验证的类型化边界机械编码到原生命令。

// 引入统一错误码、错误值和结果类型。
use crate::core::error::{Errc, Error, Result};

// 引入共享纹理尺寸、资源描述、格式与句柄。
use super::{RhiExtent, TextureDesc, TextureFormat, TextureHandle};

// 描述纹理左上坐标系中的类型化原点。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RhiTextureOrigin {
    // 保存横向像素坐标。
    x: u32,
    // 保存纵向像素坐标。
    y: u32,
}

// 为纹理原点提供封闭构造与只读访问。
impl RhiTextureOrigin {
    // 保存纹理左上角零点。
    const ZERO: Self = Self::new(0, 0);

    // 创建一个不偷偷裁剪的纹理原点。
    pub(crate) const fn new(x: u32, y: u32) -> Self {
        // 原点是否落入资源范围由区域门禁统一证明。
        Self { x, y }
    }

    // 返回横向像素坐标。
    pub(crate) const fn x(self) -> u32 {
        // 保持原始无符号像素值。
        self.x
    }

    // 返回纵向像素坐标。
    pub(crate) const fn y(self) -> u32 {
        // 保持原始无符号像素值。
        self.y
    }
}

// 描述一个不可拆分的纹理原点与尺寸。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RhiTextureRegion {
    // 保存区域左上原点。
    origin: RhiTextureOrigin,
    // 保存区域物理尺寸。
    extent: RhiExtent,
}

// 为纹理区域提供唯一构造、读取与边界验证。
impl RhiTextureRegion {
    // 从显式原点和尺寸创建类型化区域。
    pub(crate) const fn new(origin: RhiTextureOrigin, extent: RhiExtent) -> Self {
        // 保持原点与尺寸在同一个值对象内。
        Self { origin, extent }
    }

    // 从坐标与尺寸创建类型化区域。
    pub(crate) const fn from_xy(x: u32, y: u32, extent: RhiExtent) -> Self {
        // 坐标先进入类型化原点，再与尺寸组成区域。
        Self::new(RhiTextureOrigin::new(x, y), extent)
    }

    // 创建覆盖完整纹理尺寸的零原点区域。
    pub(crate) const fn full(extent: RhiExtent) -> Self {
        // 完整区域固定从左上角开始。
        Self::new(RhiTextureOrigin::ZERO, extent)
    }

    // 返回区域左上原点。
    pub(crate) const fn origin(self) -> RhiTextureOrigin {
        // 暴露只读值副本，禁止字段被拆散修改。
        self.origin
    }

    // 返回区域物理尺寸。
    pub(crate) const fn extent(self) -> RhiExtent {
        // 暴露只读值副本，保持宽高不可独立改写。
        self.extent
    }

    // 验证区域完整落在指定纹理尺寸内。
    pub(crate) fn validate_within(
        // 按值消费不可变区域事实。
        self,
        // 接收资源登记的完整纹理尺寸。
        available: RhiExtent,
    ) -> Result<RhiTextureRegionBounds> {
        // 资源尺寸本身必须属于两个 Adapter 的共同原生值域。
        if !available.is_valid() {
            // 无效资源不能证明任何子区域安全。
            return Err(invalid_transfer("RHI texture extent is invalid"));
        }
        // 空区域在原生 API 中存在错误与 no-op 分歧，统一显式拒绝。
        if self.extent.width == 0 || self.extent.height == 0 {
            // 保持既有稳定诊断。
            return Err(invalid_transfer("RHI texture transfer extent is empty"));
        }
        // 区域尺寸也必须能被 OpenGL 与 D3D11 共同无损表达。
        if !self.extent.is_valid() {
            // 禁止 Adapter 各自截断超大尺寸。
            return Err(invalid_transfer(
                "RHI texture region exceeds the native value domain",
            ));
        }
        // 横向远端边界必须使用 checked 加法。
        let right = self
            // 从类型化原点横坐标开始。
            .origin
            // 读取封闭原点的横向值。
            .x()
            // 加上同一区域对象中的宽度。
            .checked_add(self.extent.width)
            // 把整数回绕转换成稳定参数错误。
            .ok_or_else(|| invalid_transfer("RHI texture region x overflows"))?;
        // 纵向远端边界服从同一 checked 规则。
        let bottom = self
            // 从类型化原点纵坐标开始。
            .origin
            // 读取封闭原点的纵向值。
            .y()
            // 加上同一区域对象中的高度。
            .checked_add(self.extent.height)
            // 把整数回绕转换成稳定参数错误。
            .ok_or_else(|| invalid_transfer("RHI texture region y overflows"))?;
        // 区域必须完整落在资源物理范围内。
        if right > available.width || bottom > available.height {
            // Adapter 不得自行裁剪越界区域。
            return Err(invalid_transfer("RHI texture region is outside extent"));
        }
        // 返回只有本门禁能够构造的已验证边界。
        Ok(RhiTextureRegionBounds {
            // 保存完整类型化区域。
            region: self,
            // 保存不包含的横向远端边界。
            right,
            // 保存不包含的纵向远端边界。
            bottom,
        })
    }
}

// 保存一个已经验证且可以机械编码的纹理区域。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RhiTextureRegionBounds {
    // 保存原点与尺寸的不可拆区域事实。
    region: RhiTextureRegion,
    // 保存不包含的横向远端边界。
    right: u32,
    // 保存不包含的纵向远端边界。
    bottom: u32,
}

// 为原生 Adapter 提供只读的类型化区域投影。
impl RhiTextureRegionBounds {
    // 返回原始类型化区域。
    pub(crate) const fn region(self) -> RhiTextureRegion {
        // 已验证区域按值安全复制。
        self.region
    }

    // 返回原点与远端边界组成的无符号原生矩形。
    pub(crate) const fn native_rect_u32(self) -> (u32, u32, u32, u32) {
        // D3D11_BOX 可以直接消费这四条边。
        (
            // 返回左边界。
            self.region.origin.x,
            // 返回顶边界。
            self.region.origin.y,
            // 返回不包含的右边界。
            self.right,
            // 返回不包含的底边界。
            self.bottom,
        )
    }

    // 返回 OpenGL 可直接消费的有符号原点与尺寸。
    pub(crate) const fn native_origin_and_size_i32(self) -> ((i32, i32), (i32, i32)) {
        // validate_within 已证明所有值不超过有效资源的 i32 上限。
        (
            // 原点保持左上坐标系。
            (self.region.origin.x as i32, self.region.origin.y as i32),
            // 尺寸保持严格正值。
            (
                // 宽度已通过共同原生值域门禁。
                self.region.extent.width as i32,
                // 高度已通过共同原生值域门禁。
                self.region.extent.height as i32,
            ),
        )
    }

    // 计算紧密排列上传的总字节数与行跨度。
    pub(crate) fn tight_payload_layout(
        // 按值消费已验证区域。
        self,
        // 接收 Adapter 格式映射得到的每像素字节数。
        bytes_per_pixel: usize,
    ) -> Option<(usize, u32)> {
        // 零字节像素格式不属于有效上传布局。
        if bytes_per_pixel == 0 {
            // 使用空值拒绝无意义布局。
            return None;
        }
        // 紧密行跨度必须先检查 usize 乘法。
        let row_pitch = (self.region.extent.width as usize).checked_mul(bytes_per_pixel)?;
        // D3D11 行跨度 ABI 使用 u32，OpenGL 也共享这一上限。
        let row_pitch_u32 = u32::try_from(row_pitch).ok()?;
        // 总长度必须检查行跨度与高度的乘法。
        let byte_len = row_pitch.checked_mul(self.region.extent.height as usize)?;
        // 返回两个 Adapter 共用的紧密布局事实。
        Some((byte_len, row_pitch_u32))
    }
}

// 描述复制与移动共用的源区域、目标原点和唯一尺寸。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RhiTextureTransfer {
    // 保存源纹理中的完整区域。
    source: RhiTextureRegion,
    // 保存目标纹理中的左上原点。
    destination: RhiTextureOrigin,
}

// 为传输几何提供封闭构造与 scratch 拆分。
impl RhiTextureTransfer {
    // 从两个原点与唯一尺寸创建传输。
    pub(crate) const fn new(
        // 接收源左上原点。
        source: RhiTextureOrigin,
        // 接收目标左上原点。
        destination: RhiTextureOrigin,
        // 接收两端必须共享的唯一尺寸。
        extent: RhiExtent,
    ) -> Self {
        // 只在源区域保存尺寸，目标区域由同一事实派生。
        Self {
            // 组合源原点与尺寸。
            source: RhiTextureRegion::new(source, extent),
            // 保存目标原点。
            destination,
        }
    }

    // 从坐标与唯一尺寸创建传输。
    pub(crate) const fn from_xy(
        // 接收源横坐标。
        source_x: u32,
        // 接收源纵坐标。
        source_y: u32,
        // 接收目标横坐标。
        destination_x: u32,
        // 接收目标纵坐标。
        destination_y: u32,
        // 接收两端共同尺寸。
        extent: RhiExtent,
    ) -> Self {
        // 所有坐标先进入类型化原点。
        Self::new(
            // 创建源原点。
            RhiTextureOrigin::new(source_x, source_y),
            // 创建目标原点。
            RhiTextureOrigin::new(destination_x, destination_y),
            // 保持唯一尺寸。
            extent,
        )
    }

    // 创建两端都覆盖完整纹理的传输。
    pub(crate) const fn full(extent: RhiExtent) -> Self {
        // 全幅复制的两个原点都固定为零。
        Self::new(RhiTextureOrigin::ZERO, RhiTextureOrigin::ZERO, extent)
    }

    // 返回源类型化区域。
    pub(crate) const fn source(self) -> RhiTextureRegion {
        // 源区域已经不可拆分地拥有尺寸。
        self.source
    }

    // 返回由目标原点和同一尺寸派生的目标区域。
    pub(crate) const fn destination(self) -> RhiTextureRegion {
        // 禁止目标端持有可能漂移的第二份尺寸。
        RhiTextureRegion::new(self.destination, self.source.extent)
    }

    // 返回两端共享的唯一传输尺寸。
    pub(crate) const fn extent(self) -> RhiExtent {
        // 尺寸由源区域唯一拥有。
        self.source.extent
    }

    // 把同资源移动拆成经由零原点 scratch 的两段传输。
    const fn through_scratch(self) -> (Self, Self) {
        // 第一段从原始源区域写到 scratch 左上角。
        let to_scratch = Self::new(self.source.origin, RhiTextureOrigin::ZERO, self.extent());
        // 第二段从 scratch 左上角写到原始目标原点。
        let from_scratch = Self::new(RhiTextureOrigin::ZERO, self.destination, self.extent());
        // 返回保持完全相同尺寸的有序传输对。
        (to_scratch, from_scratch)
    }

    // 验证格式、非空范围和两端边界。
    fn validate(
        // 只读消费不可变传输几何。
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
            // R8 覆盖率纹理没有共同 framebuffer copy 基线。
            return Err(invalid_transfer(
                "RHI texture transfer requires a renderable color format",
            ));
        }
        // 源区域使用唯一共享门禁验证。
        let source = self.source().validate_within(source.extent)?;
        // 目标区域从同一传输尺寸派生并使用相同门禁。
        let destination = self.destination().validate_within(destination.extent)?;
        // 返回两个 Adapter 直接消费的类型化边界。
        Ok(RhiTextureTransferBounds {
            // 保存已验证源区域。
            source,
            // 保存已验证目标区域。
            destination,
        })
    }
}

// 保存一次已经验证的源区域与目标区域。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RhiTextureTransferBounds {
    // 保存已验证源区域。
    source: RhiTextureRegionBounds,
    // 保存已验证目标区域。
    destination: RhiTextureRegionBounds,
}

// 为 Adapter 提供两端类型化边界。
impl RhiTextureTransferBounds {
    // 返回已验证源区域。
    pub(crate) const fn source(self) -> RhiTextureRegionBounds {
        // 按值复制不可变边界。
        self.source
    }

    // 返回已验证目标区域。
    pub(crate) const fn destination(self) -> RhiTextureRegionBounds {
        // 按值复制不可变边界。
        self.destination
    }
}

// 描述不同纹理之间的一次有限复制。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TextureCopy {
    // 保存源纹理句柄。
    source: TextureHandle,
    // 保存目标纹理句柄。
    destination: TextureHandle,
    // 保存不可拆分的传输几何。
    transfer: RhiTextureTransfer,
}

// 为普通复制提供封闭构造、只读访问和共享验证。
impl TextureCopy {
    // 创建一次不同资源间的类型化复制命令。
    pub(crate) const fn new(
        // 接收源纹理句柄。
        source: TextureHandle,
        // 接收目标纹理句柄。
        destination: TextureHandle,
        // 接收完整传输几何。
        transfer: RhiTextureTransfer,
    ) -> Self {
        // 命令字段保持私有，禁止调用方拆散修改。
        Self {
            // 保存源资源身份。
            source,
            // 保存目标资源身份。
            destination,
            // 保存类型化传输。
            transfer,
        }
    }

    // 返回源纹理句柄。
    pub(crate) const fn source(self) -> TextureHandle {
        // 句柄按值安全复制。
        self.source
    }

    // 返回目标纹理句柄。
    pub(crate) const fn destination(self) -> TextureHandle {
        // 句柄按值安全复制。
        self.destination
    }

    // 返回不可拆分的传输几何。
    pub(crate) const fn transfer(self) -> RhiTextureTransfer {
        // 几何值按值安全复制。
        self.transfer
    }

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
        // 委托唯一传输几何验证两端资源。
        self.transfer.validate(source, destination)
    }
}

// 描述一次具有重叠安全语义的纹理区域移动。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TextureMove {
    // 保存源纹理句柄。
    source: TextureHandle,
    // 保存目标纹理句柄。
    destination: TextureHandle,
    // 保存不可拆分的传输几何。
    transfer: RhiTextureTransfer,
}

// 为移动提供封闭构造、只读访问与 scratch 降级。
impl TextureMove {
    // 创建一次具有重叠安全语义的移动命令。
    pub(crate) const fn new(
        // 接收源纹理句柄。
        source: TextureHandle,
        // 接收目标纹理句柄。
        destination: TextureHandle,
        // 接收完整传输几何。
        transfer: RhiTextureTransfer,
    ) -> Self {
        // 命令字段保持私有，禁止调用方拆散修改。
        Self {
            // 保存源资源身份。
            source,
            // 保存目标资源身份。
            destination,
            // 保存类型化传输。
            transfer,
        }
    }

    // 返回源纹理句柄。
    pub(crate) const fn source(self) -> TextureHandle {
        // 句柄按值安全复制。
        self.source
    }

    // 返回目标纹理句柄。
    pub(crate) const fn destination(self) -> TextureHandle {
        // 句柄按值安全复制。
        self.destination
    }

    // 返回不可拆分的传输几何。
    pub(crate) const fn transfer(self) -> RhiTextureTransfer {
        // 几何值按值安全复制。
        self.transfer
    }

    // 把不同资源移动无损降低为普通复制。
    pub(crate) const fn into_copy(self) -> TextureCopy {
        // 复用同一资源身份与传输几何。
        TextureCopy::new(self.source, self.destination, self.transfer)
    }

    // 把同资源移动拆成经由指定 scratch 的两次普通复制。
    pub(crate) const fn through_scratch(
        // 按值消费原始移动命令。
        self,
        // 接收 Adapter 创建的临时纹理身份。
        scratch: TextureHandle,
    ) -> (TextureCopy, TextureCopy) {
        // 由传输 Component 唯一拆分两段区域。
        let (to_scratch, from_scratch) = self.transfer.through_scratch();
        // 第一段保存源区域，第二段恢复到目标区域。
        (
            // 从原始源写到 scratch。
            TextureCopy::new(self.source, scratch, to_scratch),
            // 从 scratch 写到原始目标。
            TextureCopy::new(scratch, self.destination, from_scratch),
        )
    }

    // 验证同纹理或不同纹理的确定性区域移动。
    pub(crate) fn validate_transfer(
        // 移动命令按值进入共享门禁。
        self,
        // 接收源纹理描述。
        source: TextureDesc,
        // 接收目标纹理描述。
        destination: TextureDesc,
    ) -> Result<RhiTextureTransferBounds> {
        // move 允许同资源，由 Adapter 的 scratch 流程保证重叠安全。
        self.transfer.validate(source, destination)
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

// 仅验证两个 Adapter 必须共享的纹理区域与传输语义。
#[cfg(test)]
mod tests {
    // 引入共享纹理区域、传输命令和资源描述。
    use crate::native::present::rhi::{
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
        TextureDesc {
            // 保存物理尺寸。
            extent: RhiExtent::new(width, height),
            // 保存共享格式。
            format,
        }
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
}
