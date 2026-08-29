//! retained texture 到 swapchain 的 damage 与 scissor 一致性边界。

// 引入最终呈现 damage。
use crate::core::PresentDamage;
// 引入 sampled quad 与薄 RHI 的 surface 绘制值。
use crate::draw::backend::rhi_renderer::RhiSampledQuad;
// 引入 load action、extent 与整数 scissor。
use crate::platform::presentation::rhi::{LoadAction, RhiColor, RhiExtent, RhiScissor};

// 与 core damage 归一化和共享 adapter 上限保持一致。
const MAX_SURFACE_COMPOSITE_RECTS: usize = 64;

// 保存一次最终合成已经统一验证的 draw/present 事实。
pub(super) struct SurfaceCompositePlan {
    // 保存传给 native present 的最终 damage。
    pub(super) damage: PresentDamage,
    // 保存进入 swapchain pass 的加载动作。
    pub(super) load: LoadAction,
    // 保存逐 damage rect 裁剪的 sampled quad。
    pub(super) quads: Vec<RhiSampledQuad>,
}

// 将 damage 同时 lower 为 swapchain draw scissor 与最终 present 参数。
pub(super) fn plan_surface_composite(
    damage: PresentDamage,
    extent: RhiExtent,
    full_quad: RhiSampledQuad,
) -> SurfaceCompositePlan {
    // 缺口阴影补画只能整体重写缺口像素；局部 Load + SrcOver 会让补画 alpha
    // 逐帧叠加到不透明，因此触及圆角缺口的 partial 集合必须升级为完整合成。
    let shadow_fill_active = full_quad.surface_shadow_fill_range > 0.0
        && full_quad
            .surface_shadow_fill
            .iter()
            .any(|&alpha| alpha > 0.0)
        && full_quad.surface_corner_radius > 0.0;
    // 只有完全位于 drawable 内的受控 partial 集合可以窄绘制。
    if let PresentDamage::Partial(rects) = &damage
        && !rects.is_empty()
        && rects.len() <= MAX_SURFACE_COMPOSITE_RECTS
        && rects
            .iter()
            // 每个矩形必须能无溢出地落在当前物理 extent 内。
            .all(|&(x, y, width, height)| rect_fits_extent(x, y, width, height, extent))
        && !(shadow_fill_active
            && rects
                .iter()
                .any(|&rect| rect_touches_corner_notch(rect, extent, &full_quad)))
    {
        // 为每个 disjoint 或相邻 damage rect 生成一次同纹理 sampled draw。
        let quads = rects
            .iter()
            // 复制全幅几何，仅以 scissor 限制实际写入像素。
            .map(|&(x, y, width, height)| RhiSampledQuad {
                // 只允许写入本项 damage rect。
                scissor: Some(RhiScissor {
                    // 保持物理左上横坐标。
                    x,
                    // 保持物理左上纵坐标。
                    y,
                    // 保持物理宽度。
                    width,
                    // 保持物理高度。
                    height,
                }),
                // 其它 sampled ABI 与全幅纹理完全一致。
                ..full_quad
            })
            // 物化为 renderer 可按 painter order 消费的队列。
            .collect();
        // partial pass 必须保留 damage 外的当前 back-buffer 像素。
        return SurfaceCompositePlan {
            // Present1 消费与 draw scissor 完全相同的矩形集合。
            damage,
            // 禁止全幅 clear 写坏 dirty rect 外区域。
            load: LoadAction::Load,
            // 返回逐矩形合成操作。
            quads,
        };
    }

    // Full 或非法 partial 都保守降级为完整绘制和完整 present。
    SurfaceCompositePlan {
        // native adapter 以 DirtyRectsCount=0 编码完整提交。
        damage: PresentDamage::Full,
        // 完整帧可以安全清理整个当前 back buffer。
        load: LoadAction::Clear(RhiColor::transparent()),
        // 单个无 scissor sampled quad 更新全部像素。
        quads: vec![RhiSampledQuad {
            // 全帧绘制显式移除调用方可能携带的旧 scissor。
            scissor: None,
            // 保留全幅 sampled ABI。
            ..full_quad
        }],
    }
}

// 验证一个物理矩形可以完整落在当前 drawable 内。
fn rect_fits_extent(x: i32, y: i32, width: i32, height: i32, extent: RhiExtent) -> bool {
    // 把尺寸矩形封闭为共享左上原点 scissor 值对象。
    RhiScissor {
        // 保留调用方水平起点。
        x,
        // 保留调用方垂直起点。
        y,
        // 保留物理宽度。
        width,
        // 保留物理高度。
        height,
    }
    // 统一委托共享 checked 远端边界和目标值域门禁。
    .fits_within(extent)
}

// 判断一个物理 damage rect 是否触及任意圆角缺口方块。
fn rect_touches_corner_notch(
    (x, y, width, height): (i32, i32, i32, i32),
    extent: RhiExtent,
    quad: &RhiSampledQuad,
) -> bool {
    // 缺口方块边长与窗口圆角半径一致，圆角出现在四个物理角点。
    let radius = quad.surface_corner_radius as i32;
    // 半径无效时不存在需要整体重写的缺口区域。
    if radius <= 0 {
        return false;
    }
    // 逐角生成缺口方块并与 damage rect 做闭区间相交测试。
    let right = extent.width as i32 - radius;
    let bottom = extent.height as i32 - radius;
    // 四个角点方块共享同一边长，只需要枚举原点。
    [(0, 0), (right, 0), (0, bottom), (right, bottom)]
        .into_iter()
        // 闭区间相交：分离当且仅当一方完全位于另一方的远端之外。
        .any(|(notch_x, notch_y)| {
            x < notch_x + radius
                && notch_x < x + width
                && y < notch_y + radius
                && notch_y < y + height
        })
}
