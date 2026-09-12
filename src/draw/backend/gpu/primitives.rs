//! graphics backend 私有的 Canvas2D GPU 原语传输对象。

// 引入共享所有权缓冲区，避免原语排队期间复制大块字形、网格与像素数据。
use std::sync::Arc;

// 为纯色矩形提供可复制、可比较的值语义。
#[derive(Debug, Clone, Copy, PartialEq)]
// 描述 GPU-native Canvas2D 的轴对齐纯色矩形。
pub(crate) struct GpuSolidRect {
    /// 记录逻辑坐标左边界。
    pub(crate) x: f32,
    /// 记录逻辑坐标上边界。
    pub(crate) y: f32,
    /// 记录逻辑宽度。
    pub(crate) w: f32,
    /// 记录逻辑高度。
    pub(crate) h: f32,
    /// 记录非预乘直通 RGBA 颜色。
    pub(crate) rgba: [f32; 4],
    /// 记录左上、右上、右下、左下圆角半径。
    pub(crate) radius: [f32; 4],
}

// 为描边矩形提供可复制、可比较的值语义。
#[derive(Debug, Clone, Copy, PartialEq)]
// 描述 GPU-native Canvas2D 的轴对齐描边矩形。
pub(crate) struct GpuStrokeRect {
    /// 记录逻辑坐标左边界。
    pub(crate) x: f32,
    /// 记录逻辑坐标上边界。
    pub(crate) y: f32,
    /// 记录逻辑宽度。
    pub(crate) w: f32,
    /// 记录逻辑高度。
    pub(crate) h: f32,
    /// 记录非预乘直通 RGBA 颜色。
    pub(crate) rgba: [f32; 4],
    /// 记录左上、右上、右下、左下圆角半径。
    pub(crate) radius: [f32; 4],
    /// 记录以矩形边缘为中心的完整逻辑线宽。
    pub(crate) line_width: f32,
}

// 为字形覆盖采样提供可克隆的共享载荷语义。
#[derive(Debug, Clone)]
// 描述 GPU-native 文本的 coverage 或轮廓网格 blit。
pub(crate) struct GpuGlyphBlit {
    /// 记录字形本地左边界。
    pub(crate) x: f32,
    /// 记录字形本地上边界。
    pub(crate) y: f32,
    /// 记录字形本地宽度。
    pub(crate) w: f32,
    /// 记录字形本地高度。
    pub(crate) h: f32,
    /// 记录设备坐标四角，顺序为左上、右上、右下、左下。
    pub(crate) corners: [[f32; 2]; 4],
    /// 记录非预乘直通 RGBA 颜色。
    pub(crate) rgba: [f32; 4],
    /// 共享字体面积 coverage、软回退或 tofu 遮罩。
    pub(crate) coverage: Arc<[u8]>,
    /// 记录 coverage 像素宽度。
    pub(crate) cov_w: u32,
    /// 记录 coverage 像素高度。
    pub(crate) cov_h: u32,
    /// 可选记录 NonZero 本地轮廓边列表，优先进入 MSDF atlas。
    pub(crate) outline_mesh: Option<Arc<[f32]>>,
}

// 为字形四角构造提供 renderer 内部辅助入口。
impl GpuGlyphBlit {
    /// 由轴对齐边界构造左上、右上、右下、左下四角。
    pub(crate) fn axis_aligned_corners(
        // 接收矩形左边界。
        x: f32,
        // 接收矩形上边界。
        y: f32,
        // 接收矩形宽度。
        w: f32,
        // 接收矩形高度。
        h: f32,
    ) -> [[f32; 2]; 4] {
        // 返回与 identity 或纯平移缩放一致的轴对齐四角。
        [[x, y], [x + w, y], [x + w, y + h], [x, y + h]]
    }
}

// 为线性渐变矩形提供可复制、可比较的值语义。
#[derive(Debug, Clone, Copy, PartialEq)]
// 描述 GPU-native Canvas2D 的线性渐变矩形。
pub(crate) struct GpuLinearGradientRect {
    /// 记录渐变变换后 AABB 左边界。
    pub(crate) x: f32,
    /// 记录渐变变换后 AABB 上边界。
    pub(crate) y: f32,
    /// 记录渐变变换后 AABB 宽度。
    pub(crate) w: f32,
    /// 记录渐变变换后 AABB 高度。
    pub(crate) h: f32,
    /// 记录渐变局部矩形（变换前）宽度，供对角插值与 CPU 同源。
    pub(crate) local_w: f32,
    /// 记录渐变局部矩形（变换前）高度，供对角插值与 CPU 同源。
    pub(crate) local_h: f32,
    /// 记录当前仿射变换后的设备坐标四角。
    pub(crate) corners: [[f32; 2]; 4],
    /// 记录渐变起始颜色。
    pub(crate) color_a: [f32; 4],
    /// 记录渐变终止颜色。
    pub(crate) color_b: [f32; 4],
    /// 预计算的圆角掩码 ABI 字段（归一化半径与 quad/单位矩形）；None 关闭掩码。
    pub(crate) mask: Option<([f32; 4], [f32; 6])>,
    /// 记录水平、垂直或两条对角线方向枚举值。
    pub(crate) dir: u32,
    /// 可选多色标载荷；None 保留既有双色方向。
    pub(crate) stops: Option<crate::platform::presentation::rhi::RhiLinearGradientStops>,
}

// 为径向渐变提供可复制、可比较的值语义。
#[derive(Debug, Clone, Copy, PartialEq)]
// 描述 GPU-native Canvas2D 的径向渐变圆盘。
pub(crate) struct GpuRadialGradient {
    /// 记录逻辑圆心横坐标。
    pub(crate) cx: f32,
    /// 记录逻辑圆心纵坐标。
    pub(crate) cy: f32,
    /// 记录内半径。
    pub(crate) inner_r: f32,
    /// 记录外半径。
    pub(crate) outer_r: f32,
    /// 记录圆盘包围矩形变换后的设备坐标四角。
    pub(crate) corners: [[f32; 2]; 4],
    /// 记录内圈颜色。
    pub(crate) color_inner: [f32; 4],
    /// 记录外圈颜色。
    pub(crate) color_outer: [f32; 4],
    /// 预计算的圆角掩码 ABI 字段（归一化半径与 quad/单位矩形）；None 关闭掩码。
    pub(crate) mask: Option<([f32; 4], [f32; 6])>,
}

// 为扇形提供可复制、可比较的值语义。
#[derive(Debug, Clone, Copy, PartialEq)]
// 描述 GPU-native Canvas2D 的解析抗锯齿实心扇形。
pub(crate) struct GpuSector {
    /// 记录圆心横坐标。
    pub(crate) cx: f32,
    /// 记录圆心纵坐标。
    pub(crate) cy: f32,
    /// 记录逻辑半径。
    pub(crate) radius: f32,
    /// 记录归一化到零至整圆的起始角。
    pub(crate) start_angle: f32,
    /// 记录顺时针扫过角度。
    pub(crate) sweep_angle: f32,
    /// 记录非预乘直通 RGBA 颜色。
    pub(crate) rgba: [f32; 4],
}

// 为解析抗锯齿线段提供可复制、可比较的值语义。
#[derive(Debug, Clone, Copy, PartialEq)]
// 描述 GPU-native Canvas2D 的设备空间线段中心线与完整宽度。
pub(crate) struct GpuLineSegment {
    /// 记录设备空间起点。
    pub(crate) start: [f32; 2],
    /// 记录设备空间终点。
    pub(crate) end: [f32; 2],
    /// 记录完整设备空间线宽。
    pub(crate) width: f32,
    /// 记录非预乘直通 RGBA 颜色。
    pub(crate) rgba: [f32; 4],
}

// 为纯色三角网格提供可克隆的共享载荷语义。
#[derive(Debug, Clone)]
// 描述 CPU 曲面细分后交给 GPU 的纯色三角列表。
pub(crate) struct GpuSolidMesh {
    /// 共享交错排列的逻辑坐标 xy 顶点。
    pub(crate) vertices: Arc<[f32]>,
    /// 记录非预乘直通 RGBA 颜色。
    pub(crate) rgba: [f32; 4],
}

// 为盒阴影提供可复制、可比较的值语义。
#[derive(Debug, Clone, Copy, PartialEq)]
// 描述轴对齐或仿射映射的 GPU-native 盒阴影。
pub(crate) struct GpuBoxShadow {
    /// 记录阴影主体本地左边界。
    pub(crate) x: f32,
    /// 记录阴影主体本地上边界。
    pub(crate) y: f32,
    /// 记录阴影主体逻辑宽度。
    pub(crate) w: f32,
    /// 记录阴影主体逻辑高度。
    pub(crate) h: f32,
    /// 记录水平偏移。
    pub(crate) offset_x: f32,
    /// 记录垂直偏移。
    pub(crate) offset_y: f32,
    /// 记录水平模糊半径。
    pub(crate) blur_x: f32,
    /// 记录垂直模糊半径。
    pub(crate) blur_y: f32,
    /// 记录非预乘直通 RGBA 颜色。
    pub(crate) rgba: [f32; 4],
    /// 记录阴影主体的四角半径。
    pub(crate) radius: [f32; 4],
    /// 标记是否使用更柔和的环境阴影覆盖曲线。
    pub(crate) ambient: bool,
    /// 记录扩展阴影四边形的设备坐标四角。
    pub(crate) corners: [[f32; 2]; 4],
}

// 为图片 blit 提供可克隆的共享像素载荷语义。
#[derive(Debug, Clone)]
// 描述 GPU-native Canvas2D 的 BGRA 预乘图片 blit。
pub(crate) struct GpuImageBlit {
    /// 记录目标逻辑左边界。
    pub(crate) x: f32,
    /// 记录目标逻辑上边界。
    pub(crate) y: f32,
    /// 记录目标逻辑宽度。
    pub(crate) w: f32,
    /// 记录目标逻辑高度。
    pub(crate) h: f32,
    /// 记录支持旋转与剪切的设备坐标四角。
    pub(crate) corners: [[f32; 2]; 4],
    /// 记录已折叠的画布不透明度。
    pub(crate) opacity: f32,
    /// 标记是否使用通道相加合成。
    pub(crate) additive: bool,
    /// 共享紧裁剪的行主序 BGRA 预乘像素。
    pub(crate) pixels: Arc<Vec<u32>>,
    /// 记录像素缓冲区宽度。
    pub(crate) pixel_w: u32,
    /// 记录像素缓冲区高度。
    pub(crate) pixel_h: u32,
}
