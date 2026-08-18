//! OpenGL ES 薄 RHI 的纹理复制与重叠安全区域移动。

// 复用同级资源表、错误转换和 OpenGL 类型。
use super::*;

// 为 OpenGL RHI 设备提供 pass 外的纹理区域移动。
impl OpenGlRhiDevice {
    // 在 OpenGL ES 上执行同纹理重叠安全的区域移动。
    pub(crate) fn move_texture_region(
        &mut self,
        gl: &glow::Context,
        movement: TextureMove,
    ) -> Result<()> {
        // 移动必须发生在显式 pass 之外。
        self.pass.require_closed()?;
        // 先复制源纹理描述，避免后续 scratch 操作持有资源表借用。
        let (source_extent, source_format) = {
            // 读取源纹理尺寸和格式。
            let source = self.texture(movement.source)?;
            (source.extent, source.format)
        };
        // 读取目标纹理描述。
        let (destination_extent, destination_format) = {
            // 读取目标纹理尺寸和格式。
            let destination = self.texture(movement.destination)?;
            (destination.extent, destination.format)
        };
        // 格式、非空、范围和溢出统一委托共享 move 契约。
        movement.validate_transfer(
            // 构造源纹理的 API 无关描述。
            TextureDesc {
                // 保存源物理尺寸。
                extent: source_extent,
                // 保存源格式。
                format: source_format,
            },
            // 构造目标纹理的 API 无关描述。
            TextureDesc {
                // 保存目标物理尺寸。
                extent: destination_extent,
                // 保存目标格式。
                format: destination_format,
            },
        )?;
        // 不同纹理不需要额外 scratch，直接复用已验证 copy。
        if movement.source != movement.destination {
            // 转换为普通 texture copy。
            return self.copy_texture(
                gl,
                TextureCopy {
                    source: movement.source,
                    destination: movement.destination,
                    source_x: movement.source_x,
                    source_y: movement.source_y,
                    destination_x: movement.destination_x,
                    destination_y: movement.destination_y,
                    width: movement.width,
                    height: movement.height,
                },
            );
        }
        // 同纹理重叠移动先写入 scratch，避免依赖驱动对反馈 copy 的定义。
        let scratch = self.create_texture(
            gl,
            TextureDesc {
                extent: RhiExtent::new(movement.width, movement.height),
                format: source_format,
            },
        )?;
        // 先把源区域保存到 scratch。
        let operation = self
            .copy_texture(
                gl,
                TextureCopy {
                    source: movement.source,
                    destination: scratch,
                    source_x: movement.source_x,
                    source_y: movement.source_y,
                    destination_x: 0,
                    destination_y: 0,
                    width: movement.width,
                    height: movement.height,
                },
            )
            .and_then(|()| {
                // 再把 scratch 写回目标区域。
                self.copy_texture(
                    gl,
                    TextureCopy {
                        source: scratch,
                        destination: movement.destination,
                        source_x: 0,
                        source_y: 0,
                        destination_x: movement.destination_x,
                        destination_y: movement.destination_y,
                        width: movement.width,
                        height: movement.height,
                    },
                )
            });
        // OpenGL 临时复制源必须活到所属离屏计划完成原生 submit。
        self.texture_move_scratch_after_submit.push(scratch);
        // 复制失败仍保留 typed error；临时资源由后续 submit 或设备 teardown 回收。
        operation
    }
}
