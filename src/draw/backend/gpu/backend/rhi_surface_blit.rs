//! RHI Picture texture 的有序 sampled blit。

// 引入 blit 几何、错误和 ordered damage 类型。
use crate::core::{Errc, Error, PresentDamage, Rect};
// 引入 FramePlan 的明确 render target 引用。
use crate::draw::backend::frame_plan::RenderTargetRef;
// 引入嵌套 Picture 目标的稳定句柄。
use crate::draw::geometry::types::ImageHandle;
// 引入通用 sampled quad 和薄 RHI 的 pass/load 类型。
use crate::draw::backend::rhi_renderer::RhiSampledQuad;
use crate::native::present::rhi::{
    LoadAction, RenderTargetHandle, RhiColor, RhiScissor, RhiViewport, TextureHandle,
};

// 引入当前 GPU backend owner。
use super::GpuBackend;

// 描述嵌套 Picture blit 允许采用的单一资源路径。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NestedPicturePath {
    // 源与目标都由 RHI texture 持有。
    Rhi,
    // 源与目标都由 legacy offscreen target 持有。
    Legacy,
    // 目标尚未提交，可以在触碰队列前降级到 legacy。
    DowngradeDestination,
}

// 在执行嵌套 Picture blit 前选择不会双写资源的路径。
fn nested_picture_path(
    source_has_rhi: bool,
    destination_has_rhi: bool,
    destination_rhi_committed: bool,
) -> Result<NestedPicturePath, Error> {
    // 同为 RHI 资源时保持 sampled texture 路径。
    if source_has_rhi && destination_has_rhi {
        // 返回唯一的 RHI owner 组合。
        return Ok(NestedPicturePath::Rhi);
    }
    // 同为 legacy 资源时保持原生 offscreen blit 路径。
    if !source_has_rhi && !destination_has_rhi {
        // 返回唯一的 legacy owner 组合。
        return Ok(NestedPicturePath::Legacy);
    }
    // legacy 源只能在目标首次提交前触发目标降级。
    if !source_has_rhi && destination_has_rhi && !destination_rhi_committed {
        // 调用方必须先销毁目标 RHI texture 并透明初始化 legacy target。
        return Ok(NestedPicturePath::DowngradeDestination);
    }
    // 已提交的 RHI 目标不能再切换到 legacy 源。
    if !source_has_rhi {
        // 保持明确的资源所有权错误，不在错误目标上继续绘制。
        return Err(Error::new(
            Errc::NotImplemented,
            "committed RHI Picture target cannot switch to a legacy Picture source",
        ));
    }
    // RHI 源也不能被不认识其 texture 句柄的 legacy 目标采样。
    Err(Error::new(
        Errc::NotImplemented,
        "RHI Picture source cannot be sampled by a legacy Picture target",
    ))
}

// 将 Picture 的逻辑 blur 区域转换为其离屏纹理的像素区域。
pub(super) fn lower_picture_blur_region(region: Rect) -> RhiScissor {
    // 非有限的起点沿用 CPU blur 的空区域语义。
    let x = if region.x.is_finite() {
        // Picture texture 的坐标单位就是创建时的逻辑像素。
        region.x.floor() as i32
    } else {
        // 非有限输入不能进入 RHI NDC 顶点计算。
        0
    };
    // 非有限的垂直起点同样转换为空间安全值。
    let y = if region.y.is_finite() {
        // 保持与现有 blur 入口相同的向下取整规则。
        region.y.floor() as i32
    } else {
        // 交给通用 blur 的边界裁剪完成 no-op。
        0
    };
    // 无效或非有限宽度不能产生可见 blur pass。
    let width = if region.w.is_finite() && region.w > 0.0 {
        // 宽度向上取整，覆盖逻辑区域的最后一个像素。
        region.w.ceil() as i32
    } else {
        // 非正宽度保持空 scissor。
        0
    };
    // 无效或非有限高度不能产生可见 blur pass。
    let height = if region.h.is_finite() && region.h > 0.0 {
        // 高度向上取整，保持 CPU 区域覆盖语义。
        region.h.ceil() as i32
    } else {
        // 非正高度保持空 scissor。
        0
    };
    // 返回仍由通用 blur renderer 按实际离屏 extent 裁剪的区域。
    RhiScissor {
        // 保存已经转换的水平起点。
        x,
        // 保存已经转换的垂直起点。
        y,
        // 保存已经转换的物理宽度；这里不再乘窗口 DPR。
        width,
        // 保存已经转换的物理高度；这里不再乘窗口 DPR。
        height,
    }
}

// 把已验证的 Picture blit 几何与当前合成状态组装为通用 sampled quad。
fn lower_picture_sampled_quad(
    // 保存由 Picture slot 持有的 RHI texture。
    texture: TextureHandle,
    // 目标矩形仍使用逻辑 target 空间。
    dst_rect: Rect,
    // 主 surface 才需要应用水平 drawable 比例。
    scale_x: f32,
    // 主 surface 才需要应用垂直 drawable 比例。
    scale_y: f32,
    // 当前目标的有限 opacity 作为 sampled tint。
    opacity: f32,
    // 当前目标的 blend 决定 SrcOver 或 Additive pipeline。
    additive: bool,
    // 保存已裁剪并归一化的 UV 边界。
    uv: [f32; 4],
) -> RhiSampledQuad {
    // 把目标矩形映射到实际 render target 像素空间。
    let physical = Rect::new(
        // 映射左边界。
        dst_rect.x * scale_x,
        // 映射上边界。
        dst_rect.y * scale_y,
        // 映射宽度。
        dst_rect.w * scale_x,
        // 映射高度。
        dst_rect.h * scale_y,
    );
    // 以单个已注释表达式生成物理四角，避免 formatter 把参数注释改成行尾注释。
    let corners = crate::native::present::GpuGlyphBlit::axis_aligned_corners(
        // 使用已完成单次目标缩放的物理矩形。
        physical.x, physical.y, physical.w, physical.h,
    );
    // 返回不再依赖 backend 可变状态的不可变绘制事实。
    RhiSampledQuad {
        // 保存物理左边界。
        x: physical.x,
        // 保存物理上边界。
        y: physical.y,
        // 保存物理宽度。
        w: physical.w,
        // 保存物理高度。
        h: physical.h,
        // 保存与物理矩形完全一致的四角。
        corners,
        // premultiplied texture 的四通道统一应用 opacity。
        rgba: [opacity; 4],
        // 保留调用方已解析的目标 blend 事实。
        additive,
        // 保留 Picture texture 所有权身份。
        texture,
        // 保存左 UV。
        u0: uv[0],
        // 保存上 UV。
        v0: uv[1],
        // 保存右 UV。
        u1: uv[2],
        // 保存下 UV。
        v1: uv[3],
        // Picture 场景裁剪已在调用边界验证，当前 ABI 不重复携带 clip。
        scissor: None,
    }
}

// 验证 Picture blur 的 target-space 不会重复应用主 surface DPR。
#[cfg(test)]
mod tests {
    // 引入当前区域、sampled 合成与嵌套资源路径 helper。
    use super::{
        // 测试 blur 区域转换。
        lower_picture_blur_region,
        // 测试 sampled quad 对当前 opacity/Additive 的保真。
        lower_picture_sampled_quad,
        // 测试嵌套 Picture 的资源所有权选择。
        nested_picture_path,
        // 引入所有权路径枚举用于模式匹配。
        NestedPicturePath,
    };
    // 引入统一逻辑矩形类型。
    use crate::core::Rect;
    // 引入 opaque texture 句柄验证 Picture 身份不被替换。
    use crate::native::present::rhi::TextureHandle;

    // 逻辑离屏纹理的 fractional region 应只做整数覆盖取整。
    #[test]
    fn picture_blur_region_stays_in_logical_target_space() {
        // 构造一个需要向下取起点、向上取尺寸的区域。
        let region = lower_picture_blur_region(Rect::new(2.25, 3.75, 5.5, 7.25));
        // 起点应保持逻辑像素坐标，而不是被窗口 DPR 放大。
        assert_eq!((region.x, region.y), (2, 3));
        // 尺寸应覆盖原始 fractional 区域的末端。
        assert_eq!((region.width, region.height), (6, 8));
    }

    // 非有限或非正区域必须变成 blur renderer 可安全 no-op 的空尺寸。
    #[test]
    fn picture_blur_region_rejects_invalid_extent() {
        // 使用 NaN 起点和负尺寸覆盖异常输入边界。
        let region = lower_picture_blur_region(Rect::new(f32::NAN, 1.0, -2.0, f32::INFINITY));
        // 起点回到有限安全值。
        assert_eq!((region.x, region.y), (0, 1));
        // 空尺寸确保后续 blur 不会创建 scratch texture。
        assert_eq!((region.width, region.height), (0, 0));
    }

    // Picture blur 后的 sampled 合成必须保留当前目标的 opacity 与 Additive。
    #[test]
    fn picture_sampled_quad_preserves_additive_and_target_scale() {
        // 使用稳定 Picture texture 身份。
        let texture = TextureHandle::from_raw(41);
        // 组装主 surface 上的 Additive sampled quad。
        let quad = lower_picture_sampled_quad(
            // 指定 Picture texture。
            texture,
            // 使用逻辑目标矩形。
            Rect::new(2.0, 3.0, 4.0, 5.0),
            // 模拟水平二倍 drawable 比例。
            2.0,
            // 模拟垂直三倍 drawable 比例。
            3.0,
            // 使用有限半透明度。
            0.25,
            // 选择当前目标的 Additive blend。
            true,
            // 使用已归一化的裁剪 UV。
            [0.1, 0.2, 0.7, 0.9],
        );
        // 几何只应用一次目标比例。
        assert_eq!((quad.x, quad.y, quad.w, quad.h), (4.0, 9.0, 8.0, 15.0));
        // tint 四通道必须共同保存 opacity。
        assert_eq!(quad.rgba, [0.25; 4]);
        // lowering 必须选择 Additive sampled pipeline 事实。
        assert!(quad.additive);
        // texture 身份必须仍属于原 Picture。
        assert_eq!(quad.texture, texture);
        // UV 必须原样保留，避免 blur 后二次裁剪漂移。
        assert_eq!((quad.u0, quad.v0, quad.u1, quad.v1), (0.1, 0.2, 0.7, 0.9));
    }

    // 嵌套 Picture 必须在提交前收敛到单一资源所有者。
    #[test]
    fn nested_picture_path_is_atomic_across_rhi_and_legacy_owners() {
        // 两端都有 RHI texture 时保持 sampled 路径。
        assert!(matches!(
            nested_picture_path(true, true, false),
            Ok(NestedPicturePath::Rhi)
        ));
        // 两端都没有 RHI texture 时保持 legacy 路径。
        assert!(matches!(
            nested_picture_path(false, false, false),
            Ok(NestedPicturePath::Legacy)
        ));
        // 未提交的 RHI 目标允许在队列执行前降级。
        assert!(matches!(
            nested_picture_path(false, true, false),
            Ok(NestedPicturePath::DowngradeDestination)
        ));
        // 已提交 RHI 内容后不能再接入 legacy 源。
        let committed_target = nested_picture_path(false, true, true);
        // 失败必须保持可恢复层识别的 NotImplemented 分类。
        assert!(matches!(
            committed_target,
            Err(error) if error.code() == crate::core::Errc::NotImplemented
        ));
        // legacy 目标不能直接采样 RHI source handle。
        let legacy_target = nested_picture_path(true, false, false);
        // 反向所有权不兼容同样必须在触碰目标前失败。
        assert!(matches!(
            legacy_target,
            Err(error) if error.code() == crate::core::Errc::NotImplemented
        ));
    }
}

// 为 GpuBackend 提供 RHI Picture texture 到当前 target 的保序合成。
impl GpuBackend {
    // 把尚未提交过 RHI 内容的 Picture slot 降级为唯一的 legacy target。
    pub(super) fn downgrade_offscreen_rhi_texture(
        &mut self,
        handle: ImageHandle,
    ) -> Result<(), Error> {
        // 先从 slot 取出句柄，避免 owner-thread destroy 借用跨过 slot 修改。
        let texture = self
            .offscreens
            .get_mut(handle.0 as usize)
            .and_then(Option::as_mut)
            .ok_or_else(|| {
                // 丢失 slot 时返回稳定的状态错误。
                Error::new(
                    Errc::InvalidState,
                    "Picture target disappeared before RHI fallback downgrade",
                )
            })?
            .rhi_texture
            .take();
        // 没有 RHI 纹理时已经是 legacy-only slot，降级操作是幂等的。
        let Some(texture) = texture else {
            // 保持已经完成的降级状态。
            return Ok(());
        };
        // RHI 纹理必须由创建它的 owner-thread context 检查式释放。
        let Some(context) = self.gpu_ctx.rhi_context() else {
            // context 丢失时恢复句柄，禁止把未释放资源伪装成降级成功。
            if let Some(Some(offscreen)) = self.offscreens.get_mut(handle.0 as usize) {
                // 恢复原有 RHI owner 供 shutdown 重试。
                offscreen.rhi_texture = Some(texture);
            }
            // 返回 typed owner-state 失败。
            return Err(Error::new(
                Errc::InvalidState,
                "RHI Picture texture lost its owner context before fallback downgrade",
            ));
        };
        // 销毁失败同样恢复 slot，保证后续 shutdown 仍能重试资源回收。
        if let Err(error) = context.destroy_texture(texture) {
            // 尽量恢复尚未释放的资源句柄。
            if let Some(Some(offscreen)) = self.offscreens.get_mut(handle.0 as usize) {
                // 保留原 owner 身份。
                offscreen.rhi_texture = Some(texture);
            }
            // 传播 adapter 的真实资源错误。
            return Err(error);
        }
        // 成功后 slot 只保留已经准备好的 legacy target。
        Ok(())
    }

    // 在提交嵌套 Picture 目标队列前统一 source/destination 的资源所有权。
    pub(super) fn prepare_nested_picture_blit(
        &mut self,
        source_has_rhi: bool,
        destination: ImageHandle,
    ) -> Result<(), Error> {
        // 读取目标的 legacy handle 与当前 RHI owner 状态。
        let (legacy_target, destination_has_rhi) = self
            .offscreens
            .get(destination.0 as usize)
            .and_then(Option::as_ref)
            .map(|offscreen| (offscreen.target, offscreen.rhi_texture.is_some()))
            .ok_or_else(|| {
                // 目标在 painter-order 边界消失时保持 typed state error。
                Error::new(
                    Errc::InvalidState,
                    "nested Picture destination disappeared before resource selection",
                )
            })?;
        // 在任何 submit/bind 前计算原子的单一 owner 路径。
        let path = nested_picture_path(
            source_has_rhi,
            destination_has_rhi,
            self.offscreen_rhi_initialized,
        )?;
        // 已经同属一种资源模型时不改变目标状态。
        if matches!(path, NestedPicturePath::Rhi | NestedPicturePath::Legacy) {
            // 让调用方继续提交目标队列。
            return Ok(());
        }
        // 首次提交前销毁目标 RHI texture，使后续 blit 只读取 legacy source。
        self.downgrade_offscreen_rhi_texture(destination)?;
        // begin 阶段因原有 RHI texture 未绑定 legacy target，此处补齐 owner 切换。
        self.gpu_ctx.bind_offscreen_target(legacy_target)?;
        // legacy target 可能保留旧缓存内容，必须以透明替换语义初始化。
        if let Err(error) = self.gpu_ctx.clear_render_target(0.0, 0.0, 0.0, 0.0) {
            // 清理失败后优先恢复主 target，保留原始失败作为 source。
            return match self.gpu_ctx.bind_swapchain_target() {
                // 主 target 恢复成功时返回原始清理失败。
                Ok(()) => Err(error),
                // 恢复也失败时用恢复错误包裹原始失败。
                Err(restore_error) => Err(restore_error.with_source(error)),
            };
        }
        // 目标已透明初始化，后续 legacy flush 可以安全使用 Load 语义。
        Ok(())
    }

    // 将 RHI 离屏 texture 作为已有 sampled source 合成到当前 target。
    pub(super) fn blit_rhi_offscreen_texture(
        &mut self,
        texture: TextureHandle,
        source_width: i32,
        source_height: i32,
        src_rect: Rect,
        dst_rect: Rect,
        opacity: f32,
        additive: bool,
    ) -> Result<(), Error> {
        // source 和 destination 的几何必须能稳定转换为归一化 UV。
        if source_width <= 0
            || source_height <= 0
            || !src_rect.x.is_finite()
            || !src_rect.y.is_finite()
            || !src_rect.w.is_finite()
            || !src_rect.h.is_finite()
            || src_rect.w <= 0.0
            || src_rect.h <= 0.0
            || !dst_rect.x.is_finite()
            || !dst_rect.y.is_finite()
            || !dst_rect.w.is_finite()
            || !dst_rect.h.is_finite()
            || dst_rect.w <= 0.0
            || dst_rect.h <= 0.0
        {
            // 返回稳定的参数错误，不把异常 Picture 几何交给 adapter 猜测。
            return Err(Error::new(
                Errc::InvalidArgument,
                "RHI offscreen blit geometry is invalid",
            ));
        }
        // 把源区域裁到 source texture 范围，保持 legacy blit 的可见子矩形语义。
        let u0 = (src_rect.x / source_width as f32).clamp(0.0, 1.0);
        let v0 = (src_rect.y / source_height as f32).clamp(0.0, 1.0);
        let u1 = ((src_rect.x + src_rect.w) / source_width as f32).clamp(0.0, 1.0);
        let v1 = ((src_rect.y + src_rect.h) / source_height as f32).clamp(0.0, 1.0);
        // 完全在 source 外的 blit 是有序 no-op。
        if u1 <= u0 || v1 <= v0 {
            // 不提交空 quad，也不改变当前 frame 状态。
            return Ok(());
        }
        // 读取当前目标是主 surface 还是另一个 RHI Picture texture。
        let active = self.active_offscreen;
        // 记录本次 blit 是否直接写入主 retained surface，以及对应的 load 语义。
        let (target, target_width, target_height, target_load, writes_main_retained) =
            if let Some(active) = active {
                // 当前 Picture target 必须具备 RHI texture，才能继续使用同一条链路。
                let Some(Some(offscreen)) = self.offscreens.get(active as usize) else {
                    // 返回稳定的状态错误。
                    return Err(Error::new(
                        Errc::InvalidState,
                        "active Picture target disappeared before RHI blit",
                    ));
                };
                let Some(target_texture) = offscreen.rhi_texture else {
                    // 不在两个不同资源模型之间伪造同步。
                    return Err(Error::new(
                        Errc::NotImplemented,
                        "active Picture target lacks an RHI texture",
                    ));
                };
                // 将当前 Picture texture 转成通用 render target 身份。
                (
                    RenderTargetRef::Texture(RenderTargetHandle::from_raw(target_texture.raw())),
                    offscreen.width,
                    offscreen.height,
                    LoadAction::Load,
                    false,
                )
            } else {
                // soft 内容不能与当前 retained sampled pass 无损交错合成。
                if self.surface.canvas.soft_has_content {
                    // 保持明确的未实现边界，禁止丢失前序 CPU 像素。
                    return Err(Error::new(
                        Errc::NotImplemented,
                        "RHI main-surface blit cannot follow soft canvas content",
                    ));
                }
                // 有局部 clear 尚未表达为 RHI pass 时不能绕过它直接写 retained texture。
                if !self.surface.pending_clear_rects.is_empty() {
                    // 返回未实现让上层保留明确的兼容回退边界。
                    return Err(Error::new(
                        Errc::NotImplemented,
                        "RHI main-surface blit requires retained clear-rect lowering",
                    ));
                }
                // 主 surface 必须取得本代际 retained texture，而不是直接使用 swapchain。
                let Some(target) = self.try_rhi_surface_target()? else {
                    // 当前 backend 没有组合 RHI 时不能伪造 sampled blit 成功。
                    return Err(Error::new(
                        Errc::NotImplemented,
                        "RHI main-surface blit requires a retained surface target",
                    ));
                };
                // 校验 surface helper 没有返回易失 surface sentinel。
                if !matches!(target, RenderTargetRef::Texture(_)) {
                    // 保持 target 资源边界显式可检查。
                    return Err(Error::new(
                        Errc::InvalidState,
                        "RHI main-surface blit did not resolve to a retained texture",
                    ));
                }
                // 主 surface 的 pass load action 必须继承当前 retained 代际。
                let load = if self.surface.needs_gpu_clear {
                    // 新 retained target 先透明初始化，再叠加当前 Picture source。
                    LoadAction::Clear(RhiColor([0.0, 0.0, 0.0, 0.0]))
                } else {
                    // 已有 retained 内容时保留前序 painter-order 结果。
                    LoadAction::Load
                };
                // 返回主 retained target 的几何尺寸和写入标记。
                (target, self.surface.width, self.surface.height, load, true)
            };
        // 只有通用 renderer 和组合 RHI context 都存在时才进入无 present segment。
        let Some(renderer) = self.rhi_renderer.as_mut() else {
            // 当前 backend 没有 RHI cache，不能使用 RHI source。
            return Err(Error::new(
                Errc::NotImplemented,
                "RHI offscreen blit requires the RHI renderer cache",
            ));
        };
        let Some(context) = self.gpu_ctx.rhi_context() else {
            // 当前 adapter 未暴露组合 RHI，不能采样 RHI source。
            return Err(Error::new(
                Errc::NotImplemented,
                "RHI offscreen blit requires a composable RHI context",
            ));
        };
        // 主 surface 需要按 drawable extent 缩放，Picture target 使用自身物理尺寸。
        let (viewport, scale_x, scale_y) = if active.is_some() {
            (
                RhiViewport {
                    width: target_width.max(1) as f32,
                    height: target_height.max(1) as f32,
                },
                1.0,
                1.0,
            )
        } else {
            // 复用主 surface native lowering 的 mixed-DPI 几何规则。
            super::super::submit::rhi_physical_geometry(
                context,
                self.surface.width,
                self.surface.height,
            )
        };
        // 组装已有纹理的 sampled quad，保持 opacity、目标比例和 blend mode 事实。
        let quad = lower_picture_sampled_quad(
            // 保留 Picture texture 身份。
            texture,
            // 使用调用方已验证的逻辑目标矩形。
            dst_rect,
            // 主 surface 或 Picture target 对应的水平比例。
            scale_x,
            // 主 surface 或 Picture target 对应的垂直比例。
            scale_y,
            // 当前目标 canvas 的有限 opacity。
            opacity,
            // 当前目标 canvas 的 SrcOver/Additive 事实。
            additive,
            // 保留裁剪后的归一化 UV。
            [u0, v0, u1, v1],
        );
        // 无 present segment 只提交当前 ordered boundary，最终 present 仍由外层负责。
        let result = renderer.execute_sampled_quad_without_present(
            context,
            PresentDamage::Full,
            viewport,
            target_load,
            target,
            quad,
        );
        // 主 retained target 写入成功后等待同一帧的最终 sampled composite。
        if writes_main_retained && result.is_ok() {
            // 当前 pass 已负责新代际的透明初始化或保留旧内容。
            self.surface.needs_gpu_clear = false;
            // 清理记录已经由 pass load action 覆盖。
            self.surface.pending_clear_rects.clear();
            // 记录后续 present 必须从 retained texture 合成到 swapchain。
            self.rhi_surface_frame_pending_present = true;
        }
        // 失败时保留下一帧全清和明确的 pending 状态。
        if writes_main_retained && result.is_err() {
            // 防止失败的 retained texture 内容被当作已交付帧。
            self.surface.needs_gpu_clear = true;
            // 失败帧没有可安全合成的结果。
            self.rhi_surface_frame_pending_present = false;
        }
        // 返回 sampled pass 的真实执行结果。
        result
    }
}
