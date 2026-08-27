//! 图形 API 无关的 RHI render-pass 状态机。
//!
//! Device Adapter 只负责把这里已经验证的目标与几何事实机械编码到原生 API；
//! pass 生命周期、反馈环和命令位置不得由各 Adapter 重复解释。

// 引入统一错误码、错误值和结果类型。
use crate::core::error::{Errc, Error, Result};

// 引入共享的 pass 输入值和不透明资源句柄。
use super::{
    DrawRasterState, LoadAction, RenderTargetHandle, RhiColor, RhiExtent, RhiScissor, TextureHandle,
};

// 保存一个已经开始且尚未结束的 render pass 事实。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ActiveRhiPass {
    // 保存当前 render target 的 API 无关身份。
    target: RenderTargetHandle,
    // 保存当前目标的物理像素范围。
    extent: RhiExtent,
}

// 保存单一 Device 当前唯一 render-pass 生命周期。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RhiPassState {
    // None 表示当前允许 pass 外命令和 submit。
    active: Option<ActiveRhiPass>,
}

// 为两个 Adapter 提供唯一的 pass 状态转换和校验实现。
impl RhiPassState {
    // 创建没有活动 pass 或目标的初始状态。
    pub(crate) const fn new() -> Self {
        // 返回可嵌入任意原生 Device owner 的空状态。
        Self { active: None }
    }

    // 判断当前 Device 是否拥有一个尚未结束的 pass。
    pub(crate) const fn is_open(&self) -> bool {
        // 活动事实存在就是唯一的打开状态来源。
        self.active.is_some()
    }

    // 开始一个已经由 Adapter 解析出物理范围的 pass。
    pub(crate) fn begin(
        // 独占借用状态机以建立唯一活动事实。
        &mut self,
        // 接收 surface 或离屏纹理的通用目标身份。
        target: RenderTargetHandle,
        // 接收 Adapter 从真实目标读取的物理范围。
        extent: RhiExtent,
        // 接收用于验证清屏颜色的共享加载动作。
        load: LoadAction,
    ) -> Result<()> {
        // 已打开 pass 时拒绝嵌套，不允许 Adapter 隐式结束旧 pass。
        if self.active.is_some() {
            // 返回与原生 API 无关的状态错误。
            return Err(invalid_state("RHI render pass is already open"));
        }
        // 所有目标必须具有可由现有原生 API 无损表达的正物理尺寸。
        if !extent.is_valid() {
            // 返回稳定的参数错误。
            return Err(invalid_argument("RHI render target extent is invalid"));
        }
        // Clear 必须携带满足统一预乘 alpha 契约的颜色。
        if let LoadAction::Clear(color) = load {
            // 非有限、越界或非预乘颜色不能进入任何原生 API。
            if !color.is_valid() {
                // 返回共享颜色输入错误。
                return Err(invalid_argument("RHI render pass clear color is invalid"));
            }
        }
        // 只有全部共享前置条件通过后才建立活动状态。
        self.active = Some(ActiveRhiPass {
            // 冻结本 pass 的目标身份。
            target,
            // 冻结本 pass 的目标范围。
            extent,
        });
        // 返回状态建立成功。
        Ok(())
    }

    // 要求当前存在活动 pass。
    pub(crate) fn require_open(&self) -> Result<()> {
        // 没有活动事实时拒绝 pass 内命令。
        if self.active.is_none() {
            // 使用 InvalidState 区分顺序错误与几何输入错误。
            return Err(invalid_state("RHI render pass is not open"));
        }
        // 返回顺序检查成功。
        Ok(())
    }

    // 要求当前没有活动 pass。
    pub(crate) fn require_closed(&self) -> Result<()> {
        // 打开状态时拒绝 copy、move 或 submit 等 pass 外命令。
        if self.active.is_some() {
            // 使用统一状态错误阻止 Adapter 隐式收尾。
            return Err(invalid_state("RHI render pass is still open"));
        }
        // 返回顺序检查成功。
        Ok(())
    }

    // 返回当前 pass 的通用目标身份。
    pub(crate) fn target(&self) -> Result<RenderTargetHandle> {
        // 先取得活动事实，关闭状态必须显式失败。
        let active = self
            // 只读借用不改变任何生命周期事实。
            .active
            // 把缺失状态映射为统一顺序错误。
            .ok_or_else(|| invalid_state("RHI render pass has no active target"))?;
        // 返回复制的不透明目标身份。
        Ok(active.target)
    }

    // 返回当前 pass 的物理目标范围。
    pub(crate) fn extent(&self) -> Result<RhiExtent> {
        // 先取得活动事实，禁止用陈旧 extent 执行命令。
        let active = self
            // 只读借用当前唯一状态。
            .active
            // 把缺失状态映射为统一顺序错误。
            .ok_or_else(|| invalid_state("RHI render pass has no active extent"))?;
        // 返回复制的物理范围值。
        Ok(active.extent)
    }

    // 验证当前 Draw 独占的动态栅格状态完整落在活动目标内。
    pub(crate) fn validate_draw_raster(&self, raster: DrawRasterState) -> Result<()> {
        // 当前目标范围同时证明命令位于活动 pass 内。
        let extent = self.extent()?;
        // 复用 DrawRasterState 的唯一共同边界算法。
        if !raster.fits_within(extent) {
            // 返回不依赖原生 API 的参数错误。
            return Err(invalid_argument(
                "RHI draw raster state is invalid or outside target",
            ));
        }
        // 返回当前 packet 的完整栅格状态合法。
        Ok(())
    }

    // 验证一次局部清理的颜色和目标区域。
    pub(crate) fn validate_clear(&self, color: RhiColor, scissor: RhiScissor) -> Result<()> {
        // 当前目标范围同时证明清理位于活动 pass 内。
        let extent = self.extent()?;
        // 清理颜色必须满足统一预乘 alpha 契约。
        if !color.is_valid() {
            // 返回共享颜色参数错误。
            return Err(invalid_argument("RHI clear color is invalid"));
        }
        // 清理区域必须使用共享左上原点并完整落在目标内。
        if !scissor.fits_within(extent) {
            // 返回共享几何参数错误。
            return Err(invalid_argument(
                "RHI clear rect is invalid or outside target",
            ));
        }
        // 返回局部清理输入合法。
        Ok(())
    }

    // 验证当前 Draw 的采样纹理不会与活动输出目标形成反馈环。
    pub(crate) fn validate_sampled_texture(&self, texture: TextureHandle) -> Result<()> {
        // 当前目标查询同时证明 Draw 位于活动 pass 内。
        let target = self.target()?;
        // 禁止同一离屏纹理同时作为当前输出与采样输入。
        if target.texture() == Some(texture) {
            // 返回共享反馈环错误，避免依赖驱动隐式解绑行为。
            return Err(invalid_argument("RHI texture feedback loop is invalid"));
        }
        // 当前 Draw 的采样输入与输出目标没有资源冲突。
        Ok(())
    }

    // 验证指定纹理可以从 Device 资源表中销毁。
    pub(crate) fn validate_texture_destroy(&self, texture: TextureHandle) -> Result<()> {
        // 两个 Adapter 都不得销毁正在承载当前 pass 输出的资源。
        if self
            // 只读取共享活动 pass 事实。
            .active
            // 使用封闭 target 投影比较类型化 texture 身份。
            .is_some_and(|active| active.target.texture() == Some(texture))
        {
            // 生命周期违例统一使用 API 无关的 InvalidState。
            return Err(invalid_state(
                // 诊断不携带 D3D11 或 OpenGL 平台名称。
                "RHI cannot destroy the active render target",
            ));
        }
        // 关闭 pass、Surface target 或其它 texture 都允许继续检查式销毁。
        Ok(())
    }

    // 结束当前 pass 并原子丢弃全部 pass 级状态。
    pub(crate) fn end(&mut self) -> Result<()> {
        // 没有活动事实时拒绝重复结束。
        self.require_open()?;
        // 一次清除目标与 extent。
        self.active = None;
        // 返回生命周期转换成功。
        Ok(())
    }

    // 在 Device 释放或不可恢复重建时无条件清空状态。
    pub(crate) fn reset(&mut self) {
        // 释放路径不伪造一次正常 end，只切断所有陈旧引用。
        self.active = None;
    }
}

// 构造 API 无关的调用顺序错误。
fn invalid_state(message: &'static str) -> Error {
    // 使用统一 InvalidState 分类表达生命周期违例。
    Error::new(Errc::InvalidState, message)
}

// 构造 API 无关的输入参数错误。
fn invalid_argument(message: &'static str) -> Error {
    // 使用统一 InvalidArgument 分类表达几何、颜色或资源冲突违例。
    Error::new(Errc::InvalidArgument, message)
}
