//! 跨图形 Adapter 共享的目标尺寸、viewport 与 scissor 几何契约。

// 定义 GPU 资源尺寸，避免把平台 API 的 extent 类型泄漏到通用层。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct RhiExtent {
    // 保存资源宽度，零值只允许在构造前的临时描述中出现。
    pub(crate) width: u32,
    // 保存资源高度，零值只允许在构造前的临时描述中出现。
    pub(crate) height: u32,
}

// 为资源尺寸提供共同原生值域与 checked 投影。
impl RhiExtent {
    // 创建资源尺寸值。
    pub(crate) const fn new(width: u32, height: u32) -> Self {
        // 返回调用方提供的尺寸，不在 RHI 值层偷偷修正无效输入。
        Self { width, height }
    }

    // 把一个正尺寸轴投影到现有原生 API 共用的有符号值域。
    const fn axis_i32(value: u32) -> Option<i32> {
        // 零尺寸和超过 i32 上限的尺寸都不能进入共同 ABI。
        if value == 0 || value > i32::MAX as u32 {
            // 使用空值表达共享几何契约拒绝。
            return None;
        }
        // 已验证值允许无损转换。
        Some(value as i32)
    }

    // 返回 OpenGL、D3D11 RECT 与现有 surface host 都能无损接收的尺寸。
    pub(crate) const fn native_size_i32(self) -> Option<(i32, i32)> {
        // 宽度必须先落入共同有符号值域。
        let Some(width) = Self::axis_i32(self.width) else {
            // 无效宽度拒绝整个尺寸。
            return None;
        };
        // 高度必须服从相同规则。
        let Some(height) = Self::axis_i32(self.height) else {
            // 无效高度拒绝整个尺寸。
            return None;
        };
        // 返回可机械编码的原生尺寸。
        Some((width, height))
    }

    // 判断尺寸是否可以进入资源、surface 或 pass 生命周期。
    pub(crate) const fn is_valid(self) -> bool {
        // 两个轴都必须属于所有现有 Adapter 的共同值域。
        self.native_size_i32().is_some()
    }
}

// 定义 viewport 的物理尺寸。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RhiViewport {
    // 保存 viewport 宽度。
    pub(crate) width: f32,
    // 保存 viewport 高度。
    pub(crate) height: f32,
}

// 为 viewport 提供跨 Adapter 一致的整像素投影。
impl RhiViewport {
    // 把一个浮点物理轴投影为共同有符号整像素。
    fn axis_i32(value: f32) -> Option<i32> {
        // 零、负数、NaN、无穷和小数都不能进入共同原生 viewport。
        if !value.is_finite()
            // 物理尺寸必须严格为正。
            || value <= 0.0
            // 两个 Adapter 不得选择不同的小数量化规则。
            || value.fract() != 0.0
            // OpenGL GLsizei 与 D3D11 精确整像素共同受 i32 上限约束。
            || value as f64 > i32::MAX as f64
        {
            // 使用空值表达共享几何契约拒绝。
            return None;
        }
        // 已验证值允许无损转换。
        Some(value as i32)
    }

    // 返回 OpenGL 可直接编码且 D3D11 必须精确保持的整像素尺寸。
    pub(crate) fn native_size_i32(self) -> Option<(i32, i32)> {
        // 宽度必须先落入共同值域。
        let width = Self::axis_i32(self.width)?;
        // 高度必须服从相同规则。
        let height = Self::axis_i32(self.height)?;
        // 返回唯一原生整像素投影。
        Some((width, height))
    }

    // 判断 viewport 尺寸是否为两套原生 API 都能精确表达的正整像素。
    pub(crate) fn is_valid(self) -> bool {
        // 统一委托给唯一原生投影规则。
        self.native_size_i32().is_some()
    }

    // 判断 viewport 是否完整落在当前 render target 物理范围内。
    pub(crate) fn fits_within(self, extent: RhiExtent) -> bool {
        // 读取共享 viewport 整像素投影。
        let Some((width, height)) = self.native_size_i32() else {
            // 无效 viewport 不得进入目标边界比较。
            return false;
        };
        // 活动目标也必须属于共同原生值域。
        if !extent.is_valid() {
            // 无效目标不能证明 viewport 安全。
            return false;
        }
        // viewport 当前固定从左上角零点开始，宽度不得越过目标。
        width as u32 <= extent.width
            // viewport 高度同样不得越过目标。
            && height as u32 <= extent.height
    }
}

// 定义整数 scissor，坐标仍保持左上角原点约定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RhiScissor {
    // 保存裁剪矩形左侧。
    pub(crate) x: i32,
    // 保存裁剪矩形顶部。
    pub(crate) y: i32,
    // 保存裁剪矩形宽度。
    pub(crate) width: i32,
    // 保存裁剪矩形高度。
    pub(crate) height: i32,
}

// 为 scissor 提供跨 Adapter 一致的输入检查与边界投影。
impl RhiScissor {
    // 判断 scissor 的 extent 是否为正数且坐标没有负值。
    pub(crate) const fn is_valid(self) -> bool {
        // RHI 使用物理 surface 坐标，禁止负尺寸和负起点。
        self.x >= 0 && self.y >= 0 && self.width > 0 && self.height > 0
    }

    // 计算可被两套原生矩形 ABI 精确表达的右侧与下侧边界。
    pub(crate) const fn far_edges(self) -> Option<(i32, i32)> {
        // 基础输入无效时不允许进入边界加法。
        if !self.is_valid() {
            // 使用空值表达共享几何契约拒绝。
            return None;
        }
        // 水平边界必须保持在原生 API 共用的有符号整数值域内。
        let Some(right) = self.x.checked_add(self.width) else {
            // 溢出不能由 Adapter 饱和处理。
            return None;
        };
        // 垂直边界必须保持在同一个有符号整数值域内。
        let Some(bottom) = self.y.checked_add(self.height) else {
            // 溢出不能由 Adapter 饱和处理。
            return None;
        };
        // 返回 Adapter 可机械编码的中立边界。
        Some((right, bottom))
    }

    // 返回 D3D11 RECT 与局部 clear 共用的完整左上原点矩形。
    pub(crate) const fn native_rect(self) -> Option<(i32, i32, i32, i32)> {
        // 远端边界必须先通过唯一 checked 规则。
        let Some((right, bottom)) = self.far_edges() else {
            // 无效矩形不允许部分投影。
            return None;
        };
        // 返回左、上、右、下四条边。
        Some((self.x, self.y, right, bottom))
    }

    // 把左上原点矩形机械换算为底部原点 API 使用的 Y 坐标。
    pub(crate) const fn bottom_origin_y(self, extent: RhiExtent) -> Option<i32> {
        // 目标高度必须属于共同有符号值域。
        let Some((_, extent_height)) = extent.native_size_i32() else {
            // 无效目标不能参与坐标翻转。
            return None;
        };
        // scissor 必须完整落在目标内。
        if !self.fits_within(extent) {
            // 越界矩形不得在 Adapter 内补偿。
            return None;
        }
        // 读取从顶部到矩形底边的 checked 距离。
        let Some((_, bottom)) = self.far_edges() else {
            // fits_within 已证明该分支理论不可达。
            return None;
        };
        // 从目标高度减去顶部坐标系的底边，得到底部原点 Y。
        extent_height.checked_sub(bottom)
    }

    // 判断左上原点裁剪矩形是否完整落在当前 render target 内。
    pub(crate) const fn fits_within(self, extent: RhiExtent) -> bool {
        // 活动目标必须先属于共同原生值域。
        if !extent.is_valid() {
            // 无效目标不能证明 scissor 安全。
            return false;
        }
        // 只接受可以被所有已支持原生矩形 ABI 精确表达的边界。
        let Some((right, bottom)) = self.far_edges() else {
            // 负值、非正尺寸或有符号边界溢出都统一拒绝。
            return false;
        };
        // 右边界允许恰好等于目标宽度。
        right as u32 <= extent.width
            // 下边界允许恰好等于目标高度。
            && bottom as u32 <= extent.height
    }
}
