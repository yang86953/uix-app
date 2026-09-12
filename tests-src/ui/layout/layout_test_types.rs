// 各项自带 cfg(test) 门控，在源文件模块作用域内 include! 展开。

/// flex 布局输出。
#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) struct FlexOutput {
    pub child_rects: Vec<Rect>,
}

/// grid 布局输出。
#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) struct GridOutput {
    pub child_rects: Vec<Rect>,
    pub total_size: Size,
}
