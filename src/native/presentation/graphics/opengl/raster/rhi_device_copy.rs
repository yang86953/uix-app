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
        let source_desc = {
            // 读取源纹理的完整共享描述。
            let source = self.texture(movement.source())?;
            source.desc
        };
        // 读取目标纹理描述。
        let destination_desc = {
            // 读取目标纹理的完整共享描述。
            let destination = self.texture(movement.destination())?;
            destination.desc
        };
        // 格式、非空、范围和溢出统一委托共享 move 契约。
        movement.validate_transfer(source_desc, destination_desc)?;
        // 不同纹理不需要额外 scratch，直接复用已验证 copy。
        if movement.source() != movement.destination() {
            // 将完整类型化传输无损转换为普通 texture copy。
            return self.copy_texture(gl, movement.into_copy());
        }
        // 同纹理重叠移动先写入 scratch，避免依赖驱动对反馈 copy 的定义。
        let scratch = self.create_texture(
            gl,
            TextureDesc::new(
                // scratch 精确采用传输 Component 的唯一尺寸。
                movement.transfer().extent(),
                // scratch 继承源纹理的共享格式事实。
                source_desc.format(),
            ),
        )?;
        // 由共享传输 Component 唯一拆分保存与恢复两段 copy。
        let (to_scratch, from_scratch) = movement.through_scratch(scratch);
        // 先把源区域保存到 scratch。
        let operation = self.copy_texture(gl, to_scratch).and_then(|()| {
            // 再把 scratch 写回目标区域。
            self.copy_texture(gl, from_scratch)
        });
        // OpenGL 临时复制源必须活到所属离屏计划完成原生 submit。
        self.texture_move_scratch_after_submit.push(scratch);
        // 复制失败仍保留 typed error；临时资源由后续 submit 或设备 teardown 回收。
        operation
    }
}
