// 各项自带原 cfg 门控（uix_gpu_parity_* 或含 test/生产分支的 any 组合），
// 在源文件模块作用域 include! 展开。

impl EglContext {
    // 返回当前真实 Wayland EGL window context 的 EGL 与 GPU 身份。
    #[cfg(uix_gpu_parity_opengl)]
    pub(crate) fn parity_adapter_diagnostic(&self) -> Result<String, Error> {
        use khronos_egl as egl;

        // 诊断先服从正式 owner 门禁并恢复 current context。
        self.make_current_result()?;
        // 从当前 display 查询 EGL 实现身份，不使用 headless 或推断值。
        let egl_vendor = self
            .egl
            .query_string(Some(self.display), egl::VENDOR)
            .map_err(|error| map_egl_surface_error("eglQueryString(EGL_VENDOR)", error))?
            .to_string_lossy();
        let egl_version = self
            .egl
            .query_string(Some(self.display), egl::VERSION)
            .map_err(|error| map_egl_surface_error("eglQueryString(EGL_VERSION)", error))?
            .to_string_lossy();
        // GPU 字符串来自同一个 current window context 的 GLES runtime。
        Ok(format!(
            "EGL vendor={egl_vendor}; version={egl_version}; swap-interval={EGL_PRODUCTION_SWAP_INTERVAL} (explicit production); {}",
            self.pipeline.parity_gpu_diagnostic(),
        ))
    }
}
