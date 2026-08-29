//! 跨平台共享的窗口外圈阴影外观剖面。
//!
//! 单一外观事实来源：客户端窗口阴影在所有平台按同一有向投影场生成。
//! 模型按 Windows 11 DWM 活动窗口阴影校准（100% DPI、1200×800 参考窗实测）：
//! 把窗口矩形水平不动、垂直下移后的源矩形做 σ 高斯模糊的可分离场
//! `alpha = PEAK · Gx(x) · Gy(y)`，对参考图四边剖面的拟合残差 ≤1.2 alpha 点。
//! 距离参数一律为 scale=1 物理像素，调用方按输出缩放换算；alpha 值与缩放无关。

// 高斯模糊标准差（物理像素）。
pub(crate) const SIGMA: f32 = 18.5;
// 源矩形振幅（源内部的峰值不透明度）；按参考图四边边缘峰值联合校准。
pub(crate) const PEAK: f32 = 0.315;
// 源矩形上边相对窗口上边的下移（阴影在顶部更短更弱）。
pub(crate) const SOURCE_TOP: f32 = 9.5;
// 源矩形下边相对窗口下边的下移（阴影在底部更长更强）。
pub(crate) const SOURCE_BOTTOM: f32 = 26.5;
// 拟合得到的源水平边相对窗口左右边的微小内收，保留以贴近实测剖面。
const SOURCE_SIDE: f32 = 0.24;

// alpha 衰减到该值以下视为不可见，用于计算各边外扩距离。
const EXTENT_ALPHA: f32 = 0.0035;

// 角部 tile 内接缝修正的过渡带宽（物理像素）。
const SEAM_TRANSITION: f32 = 4.0;

const SQRT_2: f32 = 1.414_213_5;

// Abramowitz-Stegun 7.1.26 误差函数近似（最大误差 1.5e-7）。
fn erf(x: f32) -> f32 {
    let sign = x < 0.0;
    let x = x.abs();
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let polynomial = t
        * (0.254_829_592
            + t * (-0.284_496_736
                + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    let y = 1.0 - polynomial * (-x * x).exp();
    if sign { -y } else { y }
}

// 标准正态分布累积函数。
fn phi(z: f32) -> f32 {
    0.5 * (1.0 + erf(z / SQRT_2))
}

// 上边带剖面：s 为窗口上边向上的距离。
pub(crate) fn top_profile(s: f32) -> f32 {
    PEAK * phi((-s - SOURCE_TOP) / SIGMA)
}

// 侧边带剖面：s 为窗口左右边向外的距离。
pub(crate) fn side_profile(s: f32) -> f32 {
    PEAK * (1.0 - phi((s - SOURCE_SIDE) / SIGMA))
}

// 下边带剖面：s 为窗口下边向下的距离。
pub(crate) fn bottom_profile(s: f32) -> f32 {
    PEAK * (1.0 - phi((s - SOURCE_BOTTOM) / SIGMA))
}

// 按不可见阈值计算单向剖面外扩距离（scale=1 物理像素，向上取整）。
fn extent_of(profile: impl Fn(f32) -> f32) -> i32 {
    let mut extent = 1;
    while profile(extent as f32) > EXTENT_ALPHA {
        extent += 1;
    }
    extent
}

// 顶部外扩距离。
pub(crate) fn top_extent() -> i32 {
    extent_of(top_profile)
}

// 侧边外扩距离。
pub(crate) fn side_extent() -> i32 {
    extent_of(side_profile)
}

// 底部外扩距离。
pub(crate) fn bottom_extent() -> i32 {
    extent_of(bottom_profile)
}

// 三边外扩的名义平均距离，供圆角缺口补画衰减长度使用。
pub(crate) fn mean_extent() -> i32 {
    (top_extent() + side_extent() + bottom_extent()) / 3
}

// 角部 tile 的二维合成值（sx/sy 为相对窗口角点向外的物理距离）。
//
// 取三者的最大：模型场（两条轴尾部相乘，角部沿对角单调衰减、淡尾长
// 度与 Windows 一致）、水平带剖面按 sy 接缝接近度的延续、垂直带剖面
// 按 sx 接缝接近度的延续。内部由模型场主导（无两带平均的隆起），接
// 缝 4px 内由对应边带项钉住（接缝处偏差 ≤1.5 alpha 点）。负的 sx/sy
// 表示按模型场向窗口矩形内侧的平滑延续，供圆角缺口补画取值（延续值
// 钳到两侧边带边缘峰值之内）。
pub(crate) fn corner_alpha(horizontal: f32, vertical: f32, sx: f32, sy: f32, bottom: bool) -> f32 {
    let model = PEAK * model_gx(sx) * model_gy(sy, bottom);
    // 缺口延续评估（sx/sy 为负）：纯模型场，钳到两侧边带边缘峰值之内。
    if sx < 0.0 || sy < 0.0 {
        return model.min(side_profile(0.0).max(vertical));
    }
    // 接缝接近度：1 在接缝线上，δ=4px 内平滑回落到 0。
    let near_sy = 1.0 - smooth_step(sy / SEAM_TRANSITION);
    let near_sx = 1.0 - smooth_step(sx / SEAM_TRANSITION);
    let combined = model.max(horizontal * near_sy).max(vertical * near_sx);
    combined.clamp(0.0, 1.0)
}

// 垂直带在窗口边缘处的峰值（上带或下带）。
fn edge_peak(bottom: bool) -> f32 {
    if bottom {
        bottom_profile(0.0)
    } else {
        top_profile(0.0)
    }
}

// 平滑 0→1 过渡。
fn smooth_step(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// 模型场水平轴因子：s 为向外距离。
fn model_gx(s: f32) -> f32 {
    1.0 - phi((s - SOURCE_SIDE) / SIGMA)
}

// 模型场垂直轴因子：s 为向外距离，按上/下边带取对应源偏移。
fn model_gy(s: f32, bottom: bool) -> f32 {
    if bottom {
        1.0 - phi((s - SOURCE_BOTTOM) / SIGMA)
    } else {
        phi((-s - SOURCE_TOP) / SIGMA)
    }
}

// 圆角缺口补画的逐角事实 (alpha) 与公共衰减距离（scale=1 逻辑距离）。
//
// 缺口位于窗口矩形内、圆角轮廓外；补画值取模型场在圆弧对角中点处的
// 取值，使缺口阴影与外圈 tile 在圆弧两侧连续。corner_radius 为当前
// 物理圆角半径；返回 alpha 按颜色通道的 [左上, 右上, 左下, 右下] 排列。
pub(crate) fn notch_fill(corner_radius: i32, scale: i32) -> ([f32; 4], i32) {
    let scale = scale.max(1);
    // 圆角缺口最大深度出现在角点对角方向：r·(√2-1)。
    let depth = corner_radius.max(1) as f32 / scale as f32 * (SQRT_2 - 1.0);
    let top = corner_alpha(
        side_profile(-depth),
        top_profile(-depth),
        -depth,
        -depth,
        false,
    );
    let bottom = corner_alpha(
        side_profile(-depth),
        bottom_profile(-depth),
        -depth,
        -depth,
        true,
    );
    ([top, top, bottom, bottom], mean_extent())
}
