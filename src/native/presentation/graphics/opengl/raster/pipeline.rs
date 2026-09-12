// 复用 retained RHI owner、target 与错误类型。
use super::*;

// 构造和维护 OpenGL retained RHI owner。
impl OpenGlRasterPipeline {
    // 在已经 current 的原生 context 中创建固定 RHI 资源。
    pub(crate) fn new(
        runtime: NativeOpenGlRuntime,
        logical_width: i32,
        logical_height: i32,
        drawable_width: i32,
        drawable_height: i32,
    ) -> Result<Self> {
        // 借用 runtime 的 glow context 完成固定 probe 资源构造。
        let gl = runtime.context();
        // 建立默认 framebuffer 与物理 drawable 状态。
        let swapchain = TargetState::swapchain(
            logical_width,
            logical_height,
            drawable_width,
            drawable_height,
        );
        // 在同一 current GLES context 中初始化通用 RHI 的共享 VAO。
        // surface 与 texture 的 Y 方向由每个 render pass 的目标身份决定。
        let rhi = OpenGlRhiDevice::new(gl)?;
        // 返回不再持有逐 UI shader 或 atlas 的薄 owner。
        let pipeline = Self {
            // runtime 与 RHI 资源共享同一 owner thread。
            runtime,
            // 保存默认 swapchain target。
            swapchain,
            // 初始绘制目标也是默认 swapchain。
            current: swapchain,
            // 新 owner 尚未释放。
            released: false,
            // 保存固定 RHI 资源表。
            rhi,
        };
        // 使初始 viewport 与物理 drawable 一致。
        pipeline.restore_full_viewport();
        // 返回已完成固定资源构造的 owner。
        Ok(pipeline)
    }

    // 借用 owner-thread glow context。
    pub(super) fn gl(&self) -> &glow::Context {
        // runtime 是 OpenGL context 生命周期的唯一 owner。
        self.runtime.context()
    }


    // 更新 swapchain target 的尺寸与默认 viewport。
    pub(crate) fn resize_swapchain(
        &mut self,
        logical_width: i32,
        logical_height: i32,
        drawable_width: i32,
        drawable_height: i32,
    ) {
        // 重建只含物理范围的默认 target 状态。
        self.swapchain = TargetState::swapchain(
            logical_width,
            logical_height,
            drawable_width,
            drawable_height,
        );
        // 只有当前位于默认 framebuffer 时才同步恢复 viewport。
        if self.current.framebuffer.is_none() {
            // 更新当前 target 快照。
            self.current = self.swapchain;
            // 将驱动 viewport 与新物理范围同步。
            self.restore_full_viewport();
        }
    }
}

// GPU 验证专用实现位于 tests-src（模块级 include! 保持原作用域与 cfg），
// 仅 cargo test（含 RUSTFLAGS parity 入口）构建读取，发布包不携带。
#[cfg(test)]
include!("../../../../../../tests-src/native/presentation/graphics/opengl/raster/pipeline_parity_fns.rs");
