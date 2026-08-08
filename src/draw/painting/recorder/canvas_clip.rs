//! FrameRecordingCanvas 路径裁剪辅助。

// 引入不可变路径载荷。
use crate::draw::geometry::path::Path;

// 复用录制画布及其 deferred error 状态。
use super::canvas::FrameRecordingCanvas;
// 引入目标相关混合枚举。
use crate::draw::geometry::types::BlendMode;

// 为录制画布限定可证明等价的路径 coverage clip 边界。
impl FrameRecordingCanvas {
    // Additive source scratch 可完整保留路径 mask；其余入口继续保持 typed failure。
    pub(super) fn record_path_clip(&mut self, path: &Path) {
        // 已有错误时禁止继续改变裁剪栈。
        if self.deferred_error.is_some() {
            // 保留首个失败事实。
            return;
        }
        // 只有所有后续原语都能被强制送入 source scratch 时才接受路径 mask。
        if self.blend_mode != BlendMode::Additive {
            // 维持旧的不可等价 lowering 边界。
            self.unsupported_state("path clip");
            // 禁止建立只对部分命令生效的裁剪。
            return;
        }
        // 共享软件执行器完成仿射 tessellation、coverage mask 与嵌套相交。
        if let Err(error) = self.scratch.try_push_clip_path(path) {
            // 把 OOM 或拓扑失败延迟到帧边界报告。
            self.remember_error(error);
        }
    }
}
