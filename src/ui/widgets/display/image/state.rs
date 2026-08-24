// 保存 Image 私有的加载生命周期状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ImageLoadState {
    // 没有可加载来源。
    Empty,
    // 延迟加载尚未进入可见路径。
    Deferred,
    // 已登记来源但尚未开始加载。
    Pending,
    // 当前正在加载。
    Loading,
    // 图片已经可绘制。
    Ready,
    // 加载失败并保存诊断文本。
    Error(String),
}

// 提供 Image 组件消费的窄状态查询。
impl ImageLoadState {
    // 判断当前状态是否展示占位内容。
    pub(super) fn shows_placeholder(&self) -> bool {
        // 只有未完成且未失败的状态展示占位。
        matches!(
            self,
            Self::Empty | Self::Deferred | Self::Pending | Self::Loading
        )
    }

    // 返回当前失败诊断。
    pub(super) fn error(&self) -> Option<&str> {
        // 只有 Error 状态携带诊断文本。
        match self {
            // 借用已保存的错误文本。
            Self::Error(error) => Some(error),
            // 其他状态没有错误。
            _ => None,
        }
    }
}
