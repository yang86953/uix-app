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
        if self.pass_open {
            // 返回稳定的状态错误。
            return Err(rhi_invalid(
                "OpenGL RHI texture move is inside a render pass",
            ));
        }
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
        // 只有同格式颜色纹理可以安全移动。
        if source_format != destination_format {
            // 返回稳定的参数错误。
            return Err(rhi_invalid("OpenGL RHI texture move formats differ"));
        }
        // 零尺寸移动没有可定义的 copy 区域。
        if movement.width == 0 || movement.height == 0 {
            // 返回稳定的参数错误。
            return Err(rhi_invalid("OpenGL RHI texture move extent is empty"));
        }
        // 使用 checked_add 防止异常坐标回绕。
        let source_right = movement
            .source_x
            .checked_add(movement.width)
            .ok_or_else(|| rhi_invalid("OpenGL RHI texture move source x overflows"))?;
        // 检查源区域底边。
        let source_bottom = movement
            .source_y
            .checked_add(movement.height)
            .ok_or_else(|| rhi_invalid("OpenGL RHI texture move source y overflows"))?;
        // 检查目标区域右边。
        let destination_right = movement
            .destination_x
            .checked_add(movement.width)
            .ok_or_else(|| rhi_invalid("OpenGL RHI texture move destination x overflows"))?;
        // 检查目标区域底边。
        let destination_bottom = movement
            .destination_y
            .checked_add(movement.height)
            .ok_or_else(|| rhi_invalid("OpenGL RHI texture move destination y overflows"))?;
        // 同时检查源、目标范围。
        if source_right > source_extent.width
            || source_bottom > source_extent.height
            || destination_right > destination_extent.width
            || destination_bottom > destination_extent.height
        {
            // 返回稳定的参数错误。
            return Err(rhi_invalid("OpenGL RHI texture move is out of range"));
        }
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
        // 无论移动结果如何都回收 scratch 资源。
        let cleanup = self.destroy_texture(gl, scratch);
        // 优先返回移动错误，再报告临时资源清理错误。
        match (operation, cleanup) {
            // 移动失败时保留原始错误。
            (Err(error), _) => Err(error),
            // 清理失败不能被忽略。
            (Ok(()), Err(error)) => Err(error),
            // 移动和清理都成功。
            (Ok(()), Ok(())) => Ok(()),
        }
    }
}
