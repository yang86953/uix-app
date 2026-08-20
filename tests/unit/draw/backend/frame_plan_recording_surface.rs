// 记录一个不会触碰真实窗口的 mock surface。
struct RecordingSurface {
    // 保存当前 surface token。
    token: SurfaceToken,
    // 保存最终 present 次数。
    present_count: usize,
    // 保存 acquire 次数以验证 Surface 预检边界。
    acquire_count: usize,
    // 保存是否强制最终 present 失败。
    fail_present: bool,
    // 保存是否强制 Surface acquire 失败。
    fail_acquire: bool,
}

// 为记录型 surface 实现 acquire/resize/present。
impl GraphicsSurface for RecordingSurface {
    // 返回当前代际。
    fn token(&self) -> SurfaceToken {
        // 返回 surface token。
        self.token
    }

    // 返回当前 acquired image。
    fn acquire(&mut self) -> Result<SurfaceFrame> {
        // 记录每一次真正取得 Surface image 的副作用。
        self.acquire_count += 1;
        // 在已经记录 acquire 尝试后模拟 Surface 环境失败。
        if self.fail_acquire {
            // 返回稳定 Surface 生命周期错误。
            return Err(Error::new(
                Errc::GraphicsSurfaceLost,
                "recording acquire failed",
            ));
        }
        // 构造与当前 token 匹配的 frame。
        Ok(SurfaceFrame::new(self.token))
    }

    // 推进代际并更新 extent。
    fn resize(&mut self, extent: RhiExtent) -> Result<SurfaceToken> {
        // 递增代际，模拟真实 surface 重建。
        self.token = SurfaceToken::new(self.token.generation + 1, extent);
        // 返回新 token。
        Ok(self.token)
    }

    // 记录最终 present，并在失败模式下返回 surface 错误。
    fn present(&mut self, _transaction: RhiPresentTransaction) -> Result<()> {
        // 记录到达最终 present 边界。
        self.present_count += 1;
        // 在失败模式下返回 surface lost。
        if self.fail_present {
            // 返回 typed surface error，调用方不会得到 FrameCommit。
            return Err(Error::new(
                Errc::GraphicsSurfaceLost,
                "recording present failed",
            ));
        }
        // 返回成功。
        Ok(())
    }
}
