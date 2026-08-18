//! 图形 API 无关的 RHI render-pass 状态机。
//!
//! Device Adapter 只负责把这里已经验证的目标与几何事实机械编码到原生 API；
//! pass 生命周期、反馈环和命令位置不得由各 Adapter 重复解释。

// 引入统一错误码、错误值和结果类型。
use crate::core::error::{Errc, Error, Result};

// 引入共享的 pass 输入值和不透明资源句柄。
use super::{
    LoadAction, RenderTargetHandle, RhiColor, RhiExtent, RhiScissor, RhiViewport, TextureHandle,
};

// 保存一个已经开始且尚未结束的 render pass 事实。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ActiveRhiPass {
    // 保存当前 render target 的 API 无关身份。
    target: RenderTargetHandle,
    // 保存当前目标的物理像素范围。
    extent: RhiExtent,
    // 保存当前左上原点 scissor；None 表示完整目标。
    scissor: Option<RhiScissor>,
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
            // 每个 pass 都从完整目标开始。
            scissor: None,
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

    // 验证 viewport 是当前目标内的正整数物理范围。
    pub(crate) fn validate_viewport(&self, viewport: RhiViewport) -> Result<()> {
        // 当前目标范围同时证明命令位于活动 pass 内。
        let extent = self.extent()?;
        // 复用共享几何值对象的唯一边界算法。
        if !viewport.fits_within(extent) {
            // 返回不依赖原生 API 的参数错误。
            return Err(invalid_argument(
                "RHI viewport is invalid or outside target",
            ));
        }
        // 返回 viewport 合法。
        Ok(())
    }

    // 验证并记录当前 pass 的左上原点 scissor。
    pub(crate) fn set_scissor(&mut self, scissor: Option<RhiScissor>) -> Result<()> {
        // 取得可变活动事实，关闭状态不能保存悬空裁剪。
        let active = self
            // 独占借用当前 pass。
            .active
            // 把缺失状态映射为统一顺序错误。
            .as_mut()
            // 延迟构造错误，避免成功路径分配。
            .ok_or_else(|| invalid_state("RHI scissor has no active target"))?;
        // 显式 scissor 必须完整落在当前目标内。
        if scissor.is_some_and(|value| !value.fits_within(active.extent)) {
            // 返回统一几何参数错误。
            return Err(invalid_argument("RHI scissor is invalid or outside target"));
        }
        // 只有校验成功后才更新可恢复的状态镜像。
        active.scissor = scissor;
        // 返回 scissor 状态更新成功。
        Ok(())
    }

    // 返回当前 pass 已验证的 scissor 镜像。
    pub(crate) fn scissor(&self) -> Option<RhiScissor> {
        // 关闭状态与完整目标都使用 None，调用者必须在 pass 内使用该方法。
        self.active.and_then(|active| active.scissor)
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
        // 一次清除目标、extent 和 scissor。
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

// 仅验证两个 Adapter 必须共享的 pass 状态转换。
#[cfg(test)]
mod tests {
    // 引入统一错误分类。
    use crate::core::Errc;
    // 引入被测共享状态机。
    use super::RhiPassState;
    // 引入构造 pass 输入与资源身份所需的共享值。
    use crate::native::present::rhi::{
        LoadAction, RenderTargetHandle, RhiColor, RhiExtent, RhiScissor, RhiViewport, TextureHandle,
    };

    // 创建稳定的测试目标身份。
    const TARGET: RenderTargetHandle = RenderTargetHandle::for_test(TextureHandle::from_raw(7));
    // 创建与目标不同的采样纹理身份。
    const TEXTURE: TextureHandle = TextureHandle::from_raw(8);

    // 验证非法 begin 不会留下半打开状态。
    #[test]
    fn begin_rejects_invalid_target_facts_without_mutation() {
        // 创建空状态机。
        let mut state = RhiPassState::new();
        // 零宽目标必须被共享层拒绝。
        assert!(
            state
                // 尝试使用无效物理范围开始 pass。
                .begin(TARGET, RhiExtent::new(0, 20), LoadAction::Load)
                // 要求返回失败。
                .is_err()
        );
        // 失败后状态必须仍然关闭。
        assert!(!state.is_open());
        // 非预乘清屏颜色也必须被共享层拒绝。
        assert!(
            state
                // 红通道大于 alpha，违反预乘不变量。
                .begin(
                    TARGET,
                    RhiExtent::new(20, 20),
                    LoadAction::Clear(RhiColor::from_premultiplied_rgba([1.0, 0.0, 0.0, 0.5])),
                )
                // 要求返回失败。
                .is_err()
        );
        // 第二次失败后也不能留下活动目标。
        assert!(!state.is_open());
    }

    // 验证目标范围统一约束 viewport、scissor 和局部清理。
    #[test]
    fn active_target_owns_all_physical_geometry_validation() {
        // 创建空状态机。
        let mut state = RhiPassState::new();
        // 建立二十乘十的活动目标。
        state
            // 使用保留载荷开始 pass。
            .begin(TARGET, RhiExtent::new(20, 10), LoadAction::Load)
            // 测试设置必须成功。
            .expect("pass should begin");
        // 完整 viewport 必须合法。
        state
            // 验证与目标完全一致的物理范围。
            .validate_viewport(RhiViewport {
                // 使用完整目标宽度。
                width: 20.0,
                // 使用完整目标高度。
                height: 10.0,
            })
            // 共享几何门禁必须接收。
            .expect("viewport should fit");
        // 越过目标的 viewport 必须失败。
        assert!(
            state
                // 验证超出一个像素的范围。
                .validate_viewport(RhiViewport {
                    // 宽度越过目标。
                    width: 21.0,
                    // 高度保持合法。
                    height: 10.0,
                })
                // 要求返回失败。
                .is_err()
        );
        // 合法 scissor 必须写入唯一状态镜像。
        let scissor = RhiScissor {
            // 从第二个像素开始。
            x: 1,
            // 从第三个像素行开始。
            y: 2,
            // 覆盖四个像素宽度。
            width: 4,
            // 覆盖五个像素高度。
            height: 5,
        };
        // 设置共享 scissor。
        state
            // 传入显式区域。
            .set_scissor(Some(scissor))
            // 合法区域必须被接受。
            .expect("scissor should fit");
        // 状态机必须原样保存左上原点区域。
        assert_eq!(state.scissor(), Some(scissor));
        // 同一范围也必须通过局部清理门禁。
        state
            // 使用规范透明色验证区域。
            .validate_clear(RhiColor::transparent(), scissor)
            // 合法清理必须成功。
            .expect("clear should fit");
    }

    // 验证 pass 只核对当前 Draw 的采样输入与活动输出关系。
    #[test]
    fn sampled_texture_rejects_feedback_without_storing_binding() {
        // 创建并开始一个测试 pass。
        let mut state = RhiPassState::new();
        // 建立稳定目标范围。
        state
            // 使用目标七开始 pass。
            .begin(TARGET, RhiExtent::new(20, 10), LoadAction::Load)
            // 测试设置必须成功。
            .expect("pass should begin");
        // 从封闭目标中取得同一纹理身份以模拟反馈环。
        let target_texture = TARGET.texture().expect("target should be a texture");
        // 当前 render target 不能同时作为 sampled source。
        assert!(state.validate_sampled_texture(target_texture).is_err());
        // 与输出不同的 packet 采样纹理必须通过共享关系门禁。
        state
            // 只交付当前 Draw 自身携带的纹理身份。
            .validate_sampled_texture(TEXTURE)
            // 不同资源不得被 pass-local 历史状态影响。
            .expect("distinct sampled texture should pass");
    }

    // 验证活动目标销毁在两个 Adapter 之前共享同一生命周期错误。
    #[test]
    fn texture_destroy_rejects_only_the_active_target() {
        // 创建并开始一个离屏 texture pass。
        let mut state = RhiPassState::new();
        // 使用稳定物理范围建立活动目标。
        state
            // 使用共享封闭 texture target。
            .begin(TARGET, RhiExtent::new(20, 10), LoadAction::Load)
            // 测试设置必须成功。
            .expect("pass should begin");
        // 取得当前 target 携带的同一 texture 身份。
        let target_texture = TARGET.texture().expect("target should be a texture");
        // 在 pass 仍打开时销毁输出目标必须失败。
        let error = state
            // 调用两个 Adapter 共用的销毁前门禁。
            .validate_texture_destroy(target_texture)
            // 活动目标不得通过。
            .expect_err("active render target destroy must fail");
        // 该违例属于调用顺序错误，不是资源参数错误。
        assert_eq!(error.code(), Errc::InvalidState);
        // 同一 pass 中未作为输出的 texture 允许进入资源表销毁。
        state
            // 使用与目标不同的类型化 texture 身份。
            .validate_texture_destroy(TEXTURE)
            // 共享门禁必须放行。
            .expect("non-target texture destroy should pass");
        // 结束 pass 后旧目标不再被活动状态引用。
        state.end().expect("pass should end");
        // 已结束目标应允许正常销毁。
        state
            // 重新验证原 target texture。
            .validate_texture_destroy(target_texture)
            // 关闭状态不得拒绝。
            .expect("closed-pass target destroy should pass");
    }

    // 验证 end 原子清除所有 pass 级目标与几何事实。
    #[test]
    fn end_clears_target_and_geometry() {
        // 创建并开始一个测试 pass。
        let mut state = RhiPassState::new();
        // 建立稳定目标范围。
        state
            // 使用保留载荷开始 pass。
            .begin(TARGET, RhiExtent::new(20, 10), LoadAction::Load)
            // 测试设置必须成功。
            .expect("pass should begin");
        // 结束活动 pass。
        state.end().expect("pass should end");
        // end 后必须允许 pass 外命令。
        state
            // 检查共享关闭状态。
            .require_closed()
            // 状态机必须报告成功。
            .expect("state should be closed");
        // end 后不能再读取陈旧目标。
        assert!(state.target().is_err());
        // 重复 end 必须作为顺序错误被拒绝。
        assert!(state.end().is_err());
    }
}
