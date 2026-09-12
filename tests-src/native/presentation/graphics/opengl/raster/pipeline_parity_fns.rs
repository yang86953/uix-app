// 各项自带原 cfg 门控，在源文件模块作用域 include! 展开。

impl OpenGlRasterPipeline {
    // 为显式 parity 返回当前 window context 的真实 GPU 身份，不进入生产接口。
    #[cfg(uix_gpu_parity_opengl)]
    pub(crate) fn parity_gpu_diagnostic(&self) -> String {
        // 诊断调用方保证当前 owner-thread GLES context 已经 current。
        let gl = self.runtime.context();
        // SAFETY: 三个字符串查询只读取当前有效 GLES context 的驱动常量。
        let (vendor, renderer, version) = unsafe {
            (
                gl.get_parameter_string(glow::VENDOR),
                gl.get_parameter_string(glow::RENDERER),
                gl.get_parameter_string(glow::VERSION),
            )
        };
        // 保留驱动原始身份，供真机验收区分软件或错误设备。
        format!("GLES vendor={vendor}; renderer={renderer}; version={version}")
    }
}
