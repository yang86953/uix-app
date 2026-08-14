//! ScenePipeline 的 API-neutral recorder surface 生命周期。

// 引入场景流水线私有状态与渲染契约。
use super::*;

// 集中维护 recorder surface，避免 backdrop 与普通帧复制尺寸规则。
impl ScenePipeline {
    // 从真实 canvas 或无 canvas 测试场景取得稳定参考尺寸。
    pub(super) fn reference_extent<S: ScenePaint>(
        // 借用当前绘制目标。
        engine: &mut dyn RenderTarget,
        // 借用当前场景事实。
        scene: &S,
    ) -> (i32, i32) {
        // 限定 canvas 可变借用范围。
        let (canvas_w, canvas_h) = {
            // 读取当前 canvas。
            let canvas = engine.canvas_2d();
            // 复制物理录制尺寸。
            (canvas.width(), canvas.height())
        };
        // 真实 surface 尺寸优先。
        if canvas_w > 0 && canvas_h > 0 {
            // 返回 backend 提供的有效 extent。
            return (canvas_w, canvas_h);
        }

        // 无真实 canvas 的测试 backend 使用根节点形成确定录制尺寸。
        scene
            // 读取可选根节点。
            .root_id()
            // 把根布局尺寸规范为至少一个像素。
            .map(|id| {
                // 读取当前根布局矩形。
                let frame = scene.node_frame(id);
                // 返回向上取整后的正尺寸。
                (
                    // 水平方向至少一个像素。
                    frame.w.ceil().max(1.0) as i32,
                    // 垂直方向至少一个像素。
                    frame.h.ceil().max(1.0) as i32,
                )
            })
            // 空场景保留最小合法 surface。
            .unwrap_or((1, 1))
    }

    // 初始化或调整唯一 recorder surface。
    pub(super) fn ensure_recording_surface(
        // 借用 ScenePipeline owner。
        &mut self,
        // 接收请求宽度。
        width: i32,
        // 接收请求高度。
        height: i32,
    ) -> Result<(), Error> {
        // 统一把非法或空尺寸夹到最小 extent。
        let extent = (width.max(1), height.max(1));
        // 按当前 recorder 生命周期选择无动作、调整或初始化。
        match self.recording_extent {
            // 相同尺寸保持现有资源。
            Some(current) if current == extent => Ok(()),
            // 已初始化但尺寸变化时调整一次。
            Some(_) => {
                // recorder 拥有具体 resize 事务。
                self.recorder.resize(extent.0, extent.1)?;
                // 只在成功后更新登记尺寸。
                self.recording_extent = Some(extent);
                // 报告调整完成。
                Ok(())
            }
            // 首次使用时初始化唯一 recorder。
            None => {
                // 建立 API-neutral 录制 surface。
                self.recorder.initialize(extent.0, extent.1)?;
                // 只在成功后登记尺寸。
                self.recording_extent = Some(extent);
                // 报告初始化完成。
                Ok(())
            }
        }
    }
}
