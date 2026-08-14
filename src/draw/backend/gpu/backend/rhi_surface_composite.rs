//! retained texture 到 swapchain 的 damage 与 scissor 一致性边界。

// 引入最终呈现 damage。
use crate::core::PresentDamage;
// 引入 sampled quad 与薄 RHI 的 surface 绘制值。
use crate::draw::backend::rhi_renderer::RhiSampledQuad;
// 引入 load action、extent 与整数 scissor。
use crate::native::present::rhi::{LoadAction, RhiColor, RhiExtent, RhiScissor};

// 与 core damage 归一化和 D3D11 adapter 上限保持一致。
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
        load: LoadAction::Clear(RhiColor([0.0, 0.0, 0.0, 0.0])),
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
    // 使用 checked_add 同时拒绝负值、空矩形、溢出和越界。
    x >= 0
        && y >= 0
        && width > 0
        && height > 0
        && x.checked_add(width)
            // 右边界必须落在 drawable 宽度内。
            .is_some_and(|right| right <= extent.width.min(i32::MAX as u32) as i32)
        && y.checked_add(height)
            // 下边界必须落在 drawable 高度内。
            .is_some_and(|bottom| bottom <= extent.height.min(i32::MAX as u32) as i32)
}

// 验证最终合成严格保持 draw/present damage 一致。
#[cfg(test)]
mod tests {
    // 引入被测规划函数。
    use super::plan_surface_composite;
    // 引入 damage、quad 和 RHI 测试值。
    use crate::core::PresentDamage;
    // 引入 sampled quad。
    use crate::draw::backend::rhi_renderer::RhiSampledQuad;
    // 引入 RHI 句柄、load action 与 extent。
    use crate::native::present::rhi::{LoadAction, RhiExtent, TextureHandle};

    // 构造覆盖整个测试 surface 的 sampled quad。
    fn full_quad() -> RhiSampledQuad {
        // 返回稳定且有效的 sampled ABI。
        RhiSampledQuad {
            // 从左边界开始。
            x: 0.0,
            // 从上边界开始。
            y: 0.0,
            // 覆盖 100 像素宽度。
            w: 100.0,
            // 覆盖 80 像素高度。
            h: 80.0,
            // 使用轴对齐四角。
            corners: [[0.0, 0.0], [100.0, 0.0], [100.0, 80.0], [0.0, 80.0]],
            // 不改变 retained texture 颜色。
            rgba: [1.0; 4],
            // 使用 SrcOver。
            additive: false,
            // 提供非零测试纹理句柄。
            texture: TextureHandle::from_raw(7),
            // 采样完整横向 UV 起点。
            u0: 0.0,
            // 采样完整纵向 UV 起点。
            v0: 0.0,
            // 采样完整横向 UV 终点。
            u1: 1.0,
            // 采样完整纵向 UV 终点。
            v1: 1.0,
            // 初始 quad 不携带裁剪。
            scissor: None,
        }
    }

    // 验证 partial damage 只产生对应 scissor 且不执行全幅 clear。
    #[test]
    fn partial_damage_draws_and_presents_the_same_rectangles() {
        // 准备两个物理 damage rect。
        let damage = PresentDamage::Partial(vec![(2, 3, 10, 20), (40, 30, 15, 12)]);
        // 规划 100×80 surface 的最终合成。
        let plan = plan_surface_composite(damage.clone(), RhiExtent::new(100, 80), full_quad());
        // 最终 present damage 必须原样保留。
        assert_eq!(plan.damage, damage);
        // partial 路径必须保留 damage 外像素。
        assert!(matches!(plan.load, LoadAction::Load));
        // 每个 damage rect 对应一个 sampled draw。
        assert_eq!(plan.quads.len(), 2);
        // 第一个 scissor 与第一个 present rect 完全一致。
        assert_eq!(
            plan.quads[0]
                .scissor
                .map(|rect| (rect.x, rect.y, rect.width, rect.height)),
            Some((2, 3, 10, 20))
        );
        // 第二个 scissor 与第二个 present rect 完全一致。
        assert_eq!(
            plan.quads[1]
                .scissor
                .map(|rect| (rect.x, rect.y, rect.width, rect.height)),
            Some((40, 30, 15, 12))
        );
    }

    // 验证越界 partial 同时降级为完整绘制和完整提交。
    #[test]
    fn invalid_partial_damage_falls_back_to_full_draw_and_present() {
        // 构造越过右边界的 damage。
        let damage = PresentDamage::Partial(vec![(90, 0, 20, 10)]);
        // 规划最终合成。
        let plan = plan_surface_composite(damage, RhiExtent::new(100, 80), full_quad());
        // native present 必须改为 Full。
        assert_eq!(plan.damage, PresentDamage::Full);
        // 完整路径允许执行透明全幅 clear。
        assert!(matches!(plan.load, LoadAction::Clear(_)));
        // 完整路径只需一个 sampled quad。
        assert_eq!(plan.quads.len(), 1);
        // 完整 quad 不得保留局部 scissor。
        assert!(plan.quads[0].scissor.is_none());
    }
}
