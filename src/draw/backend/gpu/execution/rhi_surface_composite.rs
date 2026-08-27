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
    // 只有完全位于 drawable 内的受控 partial 集合可以窄绘制。
    if let PresentDamage::Partial(rects) = &damage
        && !rects.is_empty()
        && rects.len() <= MAX_SURFACE_COMPOSITE_RECTS
        && rects
            .iter()
            // 每个矩形必须能无溢出地落在当前物理 extent 内。
            .all(|&(x, y, width, height)| rect_fits_extent(x, y, width, height, extent))
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
    // 把 damage 矩形封闭为共享左上原点 scissor 值对象。
    RhiScissor {
        // 保留调用方水平起点。
        x,
        // 保留调用方垂直起点。
        y,
        // 保留调用方宽度。
        width,
        // 保留调用方高度。
        height,
    }
    // 统一委托共享 checked 远端边界和目标值域门禁。
    .fits_within(extent)
}
