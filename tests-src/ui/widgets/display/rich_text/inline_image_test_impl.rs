// 整 impl 自源文件移入，在源模块作用域 include! 展开。

// 暴露测试所需的资源事实，不进入公开 API。
#[cfg(all(test, feature = "image-codecs"))]
impl InlineImageState {
    // 构造带固有尺寸的测试状态。
    pub(crate) fn with_intrinsic_size(size: Size) -> Self {
        // 只设置布局关心的事实。
        Self {
            // 测试状态不绑定实际路径。
            src: String::new(),
            // 测试不需要位图句柄。
            handle: None,
            // 保存指定固有尺寸。
            intrinsic_size: Some(size),
            // 测试状态没有错误。
            error: None,
            // 测试状态不在加载中。
            loading: false,
            // 使用标准一倍设备比例。
            device_scale: 1.0,
        }
    }
}
