//! 跨图形 Adapter 共享的 Surface resize 前置与后置契约。

// 引入统一错误分类和结果类型。
use crate::core::error::{Errc, Error, Result};

// 引入 Surface token 与共同 extent 值对象。
use super::{RhiExtent, SurfaceToken};

// 保存已经通过共享原生值域验证的 Surface resize 事务。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RhiSurfaceResizeTransaction {
    // 保存 resize 开始前的完整 Surface 身份。
    previous: SurfaceToken,
    // 保存调用方请求的目标物理尺寸。
    requested: RhiExtent,
    // 保存所有现有 Adapter 都能无损接收的原生宽度。
    native_width: i32,
    // 保存所有现有 Adapter 都能无损接收的原生高度。
    native_height: i32,
}

// 为 resize 事务提供唯一构造、只读投影与成功后门禁。
impl RhiSurfaceResizeTransaction {
    // 验证请求尺寸并冻结 resize 开始前的 Surface token。
    pub(crate) fn validate(requested: RhiExtent, previous: SurfaceToken) -> Result<Self> {
        // 只允许两个 Adapter 都能无损编码的正尺寸进入原生重建。
        let Some((native_width, native_height)) = requested.native_size_i32() else {
            // 所有平台对同一非法请求返回相同分类和诊断。
            return Err(Error::new(
                // 尺寸由调用方提供，因此属于无效参数。
                Errc::InvalidArgument,
                // 诊断不得携带 D3D11、EGL 或 WGL 名称。
                "RHI surface resize extent is invalid",
            ));
        };
        // 发布字段封闭且已经完成共同值域投影的事务。
        Ok(Self {
            // 保存后置验证使用的旧 token。
            previous,
            // 保存 Adapter 必须精确实现的请求尺寸。
            requested,
            // 保存唯一的原生宽度投影。
            native_width,
            // 保存唯一的原生高度投影。
            native_height,
        })
    }

    // 返回 Adapter 必须实现的目标物理尺寸。
    pub(crate) const fn extent(self) -> RhiExtent {
        // 按值返回不可变请求事实。
        self.requested
    }

    // 返回已经通过共同值域验证的原生尺寸。
    pub(crate) const fn native_size_i32(self) -> (i32, i32) {
        // 两个轴只能成对进入原生 Surface helper。
        (self.native_width, self.native_height)
    }

    // 验证原生重建成功后发布的新 Surface token。
    pub(crate) fn complete(self, current: SurfaceToken) -> Result<SurfaceToken> {
        // Adapter 报告的物理尺寸必须精确等于事务请求。
        if current.extent != self.requested {
            // 原生操作已声称成功，因此尺寸不匹配属于 Adapter 违约。
            return Err(Error::new(
                // 使用平台错误区分调用方参数错误与实现后置条件失败。
                Errc::PlatformError,
                // 保持 API 无关的稳定诊断。
                "RHI surface resize did not produce the requested extent",
            ));
        }
        // Surface generation 在任何成功 resize 后都不得回退。
        if current.generation < self.previous.generation {
            // 回退会让已经失效的旧 FramePlan 重新变成可接受状态。
            return Err(Error::new(
                // 代际回退属于 Adapter 生命周期违约。
                Errc::PlatformError,
                // 诊断只描述共享 Surface 事实。
                "RHI surface resize generation regressed",
            ));
        }
        // 物理 extent 改变时必须推进代际以拒绝旧 acquired frame。
        if self.requested != self.previous.extent
            // 相等代表 Adapter 没有隔离旧代 Surface image。
            && current.generation == self.previous.generation
        {
            // 成功结果缺少必需的代际推进。
            return Err(Error::new(
                // 后置条件失败统一归类为平台实现错误。
                Errc::PlatformError,
                // 保持不泄漏具体 swapchain API 的诊断。
                "RHI surface resize did not advance generation",
            ));
        }
        // 返回已经证明尺寸与代际一致的新 token。
        Ok(current)
    }
}
