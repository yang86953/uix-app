//! RHI Picture texture 的有序 sampled blit。

// 引入 blit 几何与统一错误类型。
use crate::core::{Errc, Error, Rect};
// 引入 FramePlan 的明确 render target 引用。
use crate::draw::backend::frame_plan::RenderTargetRef;
// 引入通用 sampled quad 和薄 RHI 的 pass/load 类型。
use crate::draw::backend::rhi_renderer::RhiSampledQuad;
use crate::platform::presentation::rhi::{
    LoadAction, RhiColor, RhiScissor, RhiViewport, TextureHandle,
};

// 引入当前 GPU backend owner。
use super::GpuBackend;

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
    let corners = super::super::GpuGlyphBlit::axis_aligned_corners(
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
        // Picture 合成不承担原生窗口边界裁剪。
        surface_corner_radius: 0.0,
        // Picture 合成同样不承担客户端阴影环缺口补画。
        surface_shadow_fill: [0.0; 4],
        surface_shadow_fill_range: 0.0,
        // Picture 场景裁剪已在调用边界验证，当前 ABI 不重复携带 clip。
        scissor: None,
    }
}

// 为 GpuBackend 提供 RHI Picture texture 到当前 target 的保序合成。
impl GpuBackend {
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
                // 当前 Picture target 必须仍存在，才能继续使用同一条 RHI 链路。
                let Some(Some(offscreen)) = self.offscreens.get(active as usize) else {
                    // 返回稳定的状态错误。
                    return Err(Error::new(
                        Errc::InvalidState,
                        "active Picture target disappeared before RHI blit",
                    ));
                };
                // Picture slot 直接持有唯一且必需的 RHI target 纹理。
                let target_texture = offscreen.rhi_texture;
                // 将当前 Picture texture 转成通用 render target 身份。
                (
                    RenderTargetRef::Texture(target_texture),
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
                    LoadAction::Clear(RhiColor::transparent())
                } else {
                    // 已有 retained 内容时保留前序 painter-order 结果。
                    LoadAction::Load
                };
                // 返回主 retained target 的几何尺寸和写入标记。
                (target, self.surface.width, self.surface.height, load, true)
            };
        // 已验证两条分支都只能交付显式纹理目标。
        let target = match target {
            // 提取 Device-only Renderer 需要的类型化纹理句柄。
            RenderTargetRef::Texture(target) => target,
            // 防御组合边界重新返回主 Surface sentinel。
            RenderTargetRef::Surface => {
                // 使用稳定状态错误拒绝无 present Surface 写入。
                return Err(Error::new(
                    // 目标角色漂移属于内部状态错误。
                    Errc::InvalidState,
                    // 指明本入口只允许显式纹理。
                    "RHI sampled segment requires an offscreen texture target",
                ));
            }
        };
        // 只有通用 renderer 和组合 RHI context 都存在时才进入无 present segment。
        let Some(renderer) = self.rhi_renderer.as_mut() else {
            // 当前 backend 没有 RHI cache，不能使用 RHI source。
            return Err(Error::new(
                Errc::NotImplemented,
                "RHI offscreen blit requires the RHI renderer cache",
            ));
        };
        // 已验证 owner 保证 sampled blit 只使用组合 RHI。
        let context = self.gpu_ctx.rhi_context()?;
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
                // 冻结当前 drawable 的物理范围。
                context.surface_ref().token().extent,
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
            // 只把资源、命令与 submit 所需的 Device 角色交给 Renderer。
            context.device(),
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
