// 定义所有 View 节点共享的布局定位模式。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PositionMode {
    // static 参与正常布局流并忽略四边 inset。
    #[default]
    Static,
    // relative 保留正常流占位，只移动视觉与命中位置。
    Relative,
    // absolute 脱离正常流并相对最近定位祖先排列。
    Absolute,
    // fixed 脱离正常流并相对根视口排列。
    Fixed,
    // sticky 保留正常流占位并受最近滚动视口约束。
    Sticky,
}

impl PositionMode {
    // 判断当前模式是否不参与父级 flex/grid 槽位计算。
    pub(crate) const fn is_out_of_flow(self) -> bool {
        // absolute 与 fixed 都由树级定位求解器独立排列。
        matches!(self, Self::Absolute | Self::Fixed)
    }
}

// 定义四边可选逻辑像素 inset；None 对应 UIX auto。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PositionInsets {
    // 顶边 inset。
    top: Option<f32>,
    // 右边 inset。
    right: Option<f32>,
    // 底边 inset。
    bottom: Option<f32>,
    // 左边 inset。
    left: Option<f32>,
}

impl PositionInsets {
    // 创建四边均为 auto 的定位值。
    pub const fn new() -> Self {
        // 返回稳定零覆盖值。
        Self {
            // 顶边保持 auto。
            top: None,
            // 右边保持 auto。
            right: None,
            // 底边保持 auto。
            bottom: None,
            // 左边保持 auto。
            left: None,
        }
    }

    // 设置有限逻辑像素顶边 inset。
    pub fn top(mut self, value: f32) -> Self {
        // 拒绝无法形成稳定布局几何的数值。
        assert!(value.is_finite(), "position top requires a finite value");
        // 保存显式顶边值。
        self.top = Some(value);
        // 返回可继续链式声明的值。
        self
    }

    // 设置有限逻辑像素右边 inset。
    pub fn right(mut self, value: f32) -> Self {
        // 拒绝无法形成稳定布局几何的数值。
        assert!(value.is_finite(), "position right requires a finite value");
        // 保存显式右边值。
        self.right = Some(value);
        // 返回可继续链式声明的值。
        self
    }

    // 设置有限逻辑像素底边 inset。
    pub fn bottom(mut self, value: f32) -> Self {
        // 拒绝无法形成稳定布局几何的数值。
        assert!(value.is_finite(), "position bottom requires a finite value");
        // 保存显式底边值。
        self.bottom = Some(value);
        // 返回可继续链式声明的值。
        self
    }

    // 设置有限逻辑像素左边 inset。
    pub fn left(mut self, value: f32) -> Self {
        // 拒绝无法形成稳定布局几何的数值。
        assert!(value.is_finite(), "position left requires a finite value");
        // 保存显式左边值。
        self.left = Some(value);
        // 返回可继续链式声明的值。
        self
    }

    // 读取顶边显式值。
    pub const fn top_value(self) -> Option<f32> {
        // 返回复制值避免暴露内部可变状态。
        self.top
    }

    // 读取右边显式值。
    pub const fn right_value(self) -> Option<f32> {
        // 返回复制值避免暴露内部可变状态。
        self.right
    }

    // 读取底边显式值。
    pub const fn bottom_value(self) -> Option<f32> {
        // 返回复制值避免暴露内部可变状态。
        self.bottom
    }

    // 读取左边显式值。
    pub const fn left_value(self) -> Option<f32> {
        // 返回复制值避免暴露内部可变状态。
        self.left
    }

    // 把顶边恢复为 auto，供差异样式只清除单一声明。
    pub(crate) fn auto_top(mut self) -> Self {
        // 清除顶边显式像素值。
        self.top = None;
        // 返回保留其他三边的值。
        self
    }

    // 把右边恢复为 auto，供差异样式只清除单一声明。
    pub(crate) fn auto_right(mut self) -> Self {
        // 清除右边显式像素值。
        self.right = None;
        // 返回保留其他三边的值。
        self
    }

    // 把底边恢复为 auto，供差异样式只清除单一声明。
    pub(crate) fn auto_bottom(mut self) -> Self {
        // 清除底边显式像素值。
        self.bottom = None;
        // 返回保留其他三边的值。
        self
    }

    // 把左边恢复为 auto，供差异样式只清除单一声明。
    pub(crate) fn auto_left(mut self) -> Self {
        // 清除左边显式像素值。
        self.left = None;
        // 返回保留其他三边的值。
        self
    }
}

// 保存声明节点已经归一化的完整定位元数据。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct PositionedLayout {
    // 保存布局参与模式。
    pub(crate) mode: PositionMode,
    // 保存四边可选 inset。
    pub(crate) insets: PositionInsets,
}
