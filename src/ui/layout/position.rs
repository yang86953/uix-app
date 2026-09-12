// 引入保留单位的长度契约。
use crate::ui::theme::style::StyleLength;

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

// 定义四边保留单位的 inset；`StyleLength::Auto` 对应 UIX auto。
//
// px、百分比与有限 calc 都在定位求解时按包含块解析：absolute 参照最近
// 定位祖先 border-box，fixed 参照根视口，relative 参照直接父节点 frame，
// sticky 参照最近滚动视口；水平边参照宽度，垂直边参照高度。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PositionInsets {
    // 顶边 inset。
    top: StyleLength,
    // 右边 inset。
    right: StyleLength,
    // 底边 inset。
    bottom: StyleLength,
    // 左边 inset。
    left: StyleLength,
}

impl PositionInsets {
    // 创建四边均为 auto 的定位值。
    pub const fn new() -> Self {
        // 返回稳定零覆盖值。
        Self {
            // 顶边保持 auto。
            top: StyleLength::Auto,
            // 右边保持 auto。
            right: StyleLength::Auto,
            // 底边保持 auto。
            bottom: StyleLength::Auto,
            // 左边保持 auto。
            left: StyleLength::Auto,
        }
    }

    // 设置顶边 inset；接受有限逻辑像素或保留单位的 [`StyleLength`]。
    pub fn top(mut self, value: impl Into<StyleLength>) -> Self {
        // 保存经过有限性验证的显式值。
        self.top = finite_inset(value.into(), "top");
        // 返回可继续链式声明的值。
        self
    }

    // 设置右边 inset；接受有限逻辑像素或保留单位的 [`StyleLength`]。
    pub fn right(mut self, value: impl Into<StyleLength>) -> Self {
        // 保存经过有限性验证的显式值。
        self.right = finite_inset(value.into(), "right");
        // 返回可继续链式声明的值。
        self
    }

    // 设置底边 inset；接受有限逻辑像素或保留单位的 [`StyleLength`]。
    pub fn bottom(mut self, value: impl Into<StyleLength>) -> Self {
        // 保存经过有限性验证的显式值。
        self.bottom = finite_inset(value.into(), "bottom");
        // 返回可继续链式声明的值。
        self
    }

    // 设置左边 inset；接受有限逻辑像素或保留单位的 [`StyleLength`]。
    pub fn left(mut self, value: impl Into<StyleLength>) -> Self {
        // 保存经过有限性验证的显式值。
        self.left = finite_inset(value.into(), "left");
        // 返回可继续链式声明的值。
        self
    }

    // 读取顶边显式像素值；百分比或 calc 声明请改用 [`Self::top_length`]。
    pub const fn top_value(self) -> Option<f32> {
        // 只有纯像素声明可以脱离包含块读取。
        px_only(self.top)
    }

    // 读取右边显式像素值；百分比或 calc 声明请改用 [`Self::right_length`]。
    pub const fn right_value(self) -> Option<f32> {
        // 只有纯像素声明可以脱离包含块读取。
        px_only(self.right)
    }

    // 读取底边显式像素值；百分比或 calc 声明请改用 [`Self::bottom_length`]。
    pub const fn bottom_value(self) -> Option<f32> {
        // 只有纯像素声明可以脱离包含块读取。
        px_only(self.bottom)
    }

    // 读取左边显式像素值；百分比或 calc 声明请改用 [`Self::left_length`]。
    pub const fn left_value(self) -> Option<f32> {
        // 只有纯像素声明可以脱离包含块读取。
        px_only(self.left)
    }

    // 读取顶边保留单位的声明；`Auto` 表示未声明。
    pub const fn top_length(self) -> StyleLength {
        self.top
    }

    // 读取右边保留单位的声明；`Auto` 表示未声明。
    pub const fn right_length(self) -> StyleLength {
        self.right
    }

    // 读取底边保留单位的声明；`Auto` 表示未声明。
    pub const fn bottom_length(self) -> StyleLength {
        self.bottom
    }

    // 读取左边保留单位的声明；`Auto` 表示未声明。
    pub const fn left_length(self) -> StyleLength {
        self.left
    }

    // 把顶边恢复为 auto，供差异样式只清除单一声明。
    pub(crate) fn auto_top(mut self) -> Self {
        // 清除顶边显式值。
        self.top = StyleLength::Auto;
        // 返回保留其他三边的值。
        self
    }

    // 把右边恢复为 auto，供差异样式只清除单一声明。
    pub(crate) fn auto_right(mut self) -> Self {
        // 清除右边显式值。
        self.right = StyleLength::Auto;
        // 返回保留其他三边的值。
        self
    }

    // 把底边恢复为 auto，供差异样式只清除单一声明。
    pub(crate) fn auto_bottom(mut self) -> Self {
        // 清除底边显式值。
        self.bottom = StyleLength::Auto;
        // 返回保留其他三边的值。
        self
    }

    // 把左边恢复为 auto，供差异样式只清除单一声明。
    pub(crate) fn auto_left(mut self) -> Self {
        // 清除左边显式值。
        self.left = StyleLength::Auto;
        // 返回保留其他三边的值。
        self
    }

    // 按包含块尺寸解析四边；水平边参照宽度，垂直边参照高度。
    pub(crate) fn resolve(self, block_width: f32, block_height: f32) -> ResolvedInsets {
        ResolvedInsets {
            top: self.top.resolve(Some(block_height)),
            right: self.right.resolve(Some(block_width)),
            bottom: self.bottom.resolve(Some(block_height)),
            left: self.left.resolve(Some(block_width)),
        }
    }
}

// 已按包含块解析的四边逻辑像素值；`None` 表示 auto。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedInsets {
    pub(crate) top: Option<f32>,
    pub(crate) right: Option<f32>,
    pub(crate) bottom: Option<f32>,
    pub(crate) left: Option<f32>,
}

// 拒绝无法形成稳定布局几何的数值分量。
fn finite_inset(value: StyleLength, side: &str) -> StyleLength {
    let finite = match value {
        StyleLength::Auto => true,
        StyleLength::Px(px) | StyleLength::Percent(px) => px.is_finite(),
        StyleLength::Calc { px, percent } => px.is_finite() && percent.is_finite(),
    };
    assert!(finite, "position {side} requires a finite value");
    value
}

// 只有纯像素声明可以脱离包含块读取。
const fn px_only(value: StyleLength) -> Option<f32> {
    match value {
        StyleLength::Px(px) => Some(px),
        _ => None,
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
