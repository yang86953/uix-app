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

// 角部 tile 的二维合成值。
//
// 9-patch 边带 tile 只能承载单一一维剖面，而方向性场沿边带长度变化，
// 因此角部取「双带剖面的加权混合」：与上下边带的接缝（sy = 0）严格取
// 水平带值保证无缝，向对角平滑过渡到垂直带剖面；从接缝线起 4px 内
// 完成过渡，残余失配收敛在角点数像素内。负的 sx/sy 表示按剖面公式向
// 窗口矩形内侧的平滑延续，供圆角缺口补画取值。
pub(crate) fn corner_alpha(horizontal: f32, vertical: f32, sx: f32, sy: f32, h_extent: f32) -> f32 {
    let taper = (1.0 - sx / h_extent).clamp(0.0, 2.0);
    let t = (sy / 4.0).clamp(0.0, 1.0);
    let smooth = t * t * (3.0 - 2.0 * t);
    let seam_value = side_profile(0.0);
    let blended = horizontal + (vertical - seam_value) * taper * smooth;
    // 缺口延续评估（sx/sy 为负）时禁止超过两条边带的边缘峰值。
    let edge_peak = seam_value.max(vertical);
    let limit = if sx < 0.0 || sy < 0.0 {
        edge_peak
    } else {
        f32::MAX
    };
    blended.min(limit)
}

// 圆角缺口补画的逐角事实 (alpha) 与公共衰减距离（scale=1 逻辑距离）。
//
// 缺口位于窗口矩形内、圆角轮廓外；补画值取合成场在圆弧对角中点处的
// 取值，使缺口阴影与外圈 tile 在圆弧两侧连续。corner_radius 为当前
// 物理圆角半径；返回 alpha 按颜色通道的 [左上, 右上, 左下, 右下] 排列。
pub(crate) fn notch_fill(corner_radius: i32, scale: i32) -> ([f32; 4], i32) {
    let scale = scale.max(1);
    // 圆角缺口最大深度出现在角点对角方向：r·(√2-1)。
    let depth = corner_radius.max(1) as f32 / scale as f32 * (SQRT_2 - 1.0);
    let h_extent = side_extent() as f32;
    let top = corner_alpha(
        side_profile(-depth),
        top_profile(-depth),
        -depth,
        -depth,
        h_extent,
    );
    let bottom = corner_alpha(
        side_profile(-depth),
        bottom_profile(-depth),
        -depth,
        -depth,
        h_extent,
    );
    ([top, top, bottom, bottom], mean_extent())
}
