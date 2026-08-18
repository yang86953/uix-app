//! 图形 API 无关的 RHI render-pass 状态机。
//!
//! Device Adapter 只负责把这里已经验证的目标、几何与绑定事实机械编码到
//! 原生 API；pass 生命周期、反馈环和命令位置不得由各 Adapter 重复解释。

// 引入统一错误码、错误值和结果类型。
use crate::core::error::{Errc, Error, Result};

// 引入共享的 pass 输入值和不透明资源句柄。
use super::{
    LoadAction, RenderTargetHandle, RhiColor, RhiExtent, RhiScissor, RhiViewport, SamplerHandle,
    TextureHandle,
};

// 原子保存共享 shader ABI 唯一开放的采样纹理与 sampler 绑定。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SampledTextureBinding {
    // 保存被采样纹理的不透明身份。
    texture: TextureHandle,
    // 保存与纹理同时生效的 sampler 身份。
    sampler: SamplerHandle,
}

// 为 FramePlan、Device 和 Adapter 提供不暴露槽位的绑定构造与投影。
impl SampledTextureBinding {
    // 创建当前固定 t0/s0 ABI 的完整采样绑定。
    pub(crate) const fn new(texture: TextureHandle, sampler: SamplerHandle) -> Self {
        // 两个资源身份必须作为一个事实共同传递。
        Self { texture, sampler }
    }

    // 返回供 Adapter 资源表解析的纹理身份。
    pub(crate) const fn texture(self) -> TextureHandle {
        // 复制轻量句柄，不拆开保存状态。
        self.texture
    }

    // 返回供 Adapter 资源表解析的 sampler 身份。
    pub(crate) const fn sampler(self) -> SamplerHandle {
        // 复制轻量句柄，不拆开保存状态。
        self.sampler
    }
}

// 保存一个已经开始且尚未结束的 render pass 事实。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ActiveRhiPass {
    // 保存当前 render target 的 API 无关身份。
    target: RenderTargetHandle,
    // 保存当前目标的物理像素范围。
    extent: RhiExtent,
    // 保存当前左上原点 scissor；None 表示完整目标。
    scissor: Option<RhiScissor>,
    // 原子保存最近一次成功绑定的采样纹理与 sampler。
    sampled_binding: Option<SampledTextureBinding>,
}

// 保存单一 Device 当前唯一 render-pass 生命周期。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RhiPassState {
    // None 表示当前允许 pass 外命令和 submit。
    active: Option<ActiveRhiPass>,
}

// 为两个 Adapter 提供唯一的 pass 状态转换和校验实现。
impl RhiPassState {
    // 创建没有活动 pass、目标或采样绑定的初始状态。
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
        // 所有目标必须具有正物理尺寸。
        if !extent.is_positive() {
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
            // 每个 pass 都必须重新建立完整采样绑定。
            sampled_binding: None,
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

    // 验证并记录当前 pass 的唯一 sampled texture/sampler 绑定。
    pub(crate) fn bind_sampled_texture(
        // 独占借用状态机以提交绑定事实。
        &mut self,
        // 接收已经由 Adapter 资源表验证存活的原子绑定。
        binding: SampledTextureBinding,
    ) -> Result<()> {
        // 取得可变活动事实，pass 外不能留下下一帧可见绑定。
        let active = self
            // 独占借用当前 pass。
            .active
            // 把缺失状态映射为统一顺序错误。
            .as_mut()
            // 延迟构造错误。
            .ok_or_else(|| invalid_state("RHI texture binding has no active pass"))?;
        // 禁止同一离屏纹理同时作为当前输出与采样输入。
        if active.target.raw() == binding.texture().raw() {
            // 返回共享反馈环错误，避免依赖驱动隐式解绑行为。
            return Err(invalid_argument("RHI texture feedback loop is invalid"));
        }
        // 只有全部门禁通过后才原子记录纹理与 sampler。
        active.sampled_binding = Some(binding);
        // 返回绑定事实建立成功。
        Ok(())
    }

    // 返回当前 pass 最近一次完整 sampled texture 绑定。
    pub(crate) fn sampled_binding(&self) -> Option<SampledTextureBinding> {
        // 关闭状态不暴露任何陈旧或拆分绑定。
        self.active.and_then(|active| active.sampled_binding)
    }

    // 判断指定纹理是否正作为活动 render target。
    pub(crate) fn references_target(&self, texture: TextureHandle) -> bool {
        // 只比较 API 无关身份，不借用或解释原生 view。
        self.active
            // 取得可能存在的活动目标。
            .is_some_and(|active| active.target.raw() == texture.raw())
    }

    // 清除被销毁纹理留下的采样绑定身份。
    pub(crate) fn unbind_texture(&mut self, texture: TextureHandle) {
        // 只有活动 pass 可能拥有采样绑定。
        if let Some(active) = self.active.as_mut() {
            // 任一成员失效都必须清除整个原子绑定。
            if active
                .sampled_binding
                .is_some_and(|binding| binding.texture() == texture)
            {
                // 让后续 sampled draw 显式报告缺失完整绑定。
                active.sampled_binding = None;
            }
        }
    }

    // 清除被销毁 sampler 留下的绑定身份。
    pub(crate) fn unbind_sampler(&mut self, sampler: SamplerHandle) {
        // 只有活动 pass 可能拥有 sampler 绑定。
        if let Some(active) = self.active.as_mut() {
            // 任一成员失效都必须清除整个原子绑定。
            if active
                .sampled_binding
                .is_some_and(|binding| binding.sampler() == sampler)
            {
                // 让后续 sampled draw 显式报告缺失完整绑定。
                active.sampled_binding = None;
            }
        }
    }

    // 结束当前 pass 并原子丢弃全部 pass 级状态。
    pub(crate) fn end(&mut self) -> Result<()> {
        // 没有活动事实时拒绝重复结束。
        self.require_open()?;
        // 一次清除目标、extent、scissor 和采样绑定。
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
    // 引入被测共享状态机。
    use super::{RhiPassState, SampledTextureBinding};
    // 引入构造 pass 输入与资源身份所需的共享值。
    use crate::native::present::rhi::{
        LoadAction, RenderTargetHandle, RhiColor, RhiExtent, RhiScissor, RhiViewport,
        SamplerHandle, TextureHandle,
    };

    // 创建稳定的测试目标身份。
    const TARGET: RenderTargetHandle = RenderTargetHandle::from_raw(7);
    // 创建与目标不同的采样纹理身份。
    const TEXTURE: TextureHandle = TextureHandle::from_raw(8);
    // 创建稳定的采样器身份。
    const SAMPLER: SamplerHandle = SamplerHandle::from_raw(9);

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

    // 验证 pass 内绑定拒绝目标反馈环且只能保存完整事实。
    #[test]
    fn sampled_binding_rejects_feedback_and_stays_atomic() {
        // 创建并开始一个测试 pass。
        let mut state = RhiPassState::new();
        // 建立稳定目标范围。
        state
            // 使用目标七开始 pass。
            .begin(TARGET, RhiExtent::new(20, 10), LoadAction::Load)
            // 测试设置必须成功。
            .expect("pass should begin");
        // 把目标自身转换成纹理身份以模拟反馈环。
        let target_texture = TextureHandle::from_raw(TARGET.raw());
        // 组装一个输入纹理与当前目标相同的完整绑定。
        let feedback_binding = SampledTextureBinding::new(target_texture, SAMPLER);
        // 当前 render target 不能同时作为 sampled source。
        assert!(state.bind_sampled_texture(feedback_binding).is_err());
        // 失败不能提交半绑定状态。
        assert_eq!(state.sampled_binding(), None);
        // 合法的完整绑定必须成功。
        let binding = SampledTextureBinding::new(TEXTURE, SAMPLER);
        // 把原子绑定交给共享状态机。
        state
            // 绑定与目标不同的纹理和 sampler。
            .bind_sampled_texture(binding)
            // 共享门禁必须接收。
            .expect("binding should succeed");
        // 状态机必须原样返回完整绑定。
        assert_eq!(state.sampled_binding(), Some(binding));
        // 销毁 sampler 时必须同时清除纹理身份。
        state.unbind_sampler(SAMPLER);
        // 不允许留下只有纹理的半绑定状态。
        assert_eq!(state.sampled_binding(), None);
        // 重新建立完整绑定以验证纹理销毁路径。
        state
            // 第二次绑定使用相同稳定身份。
            .bind_sampled_texture(binding)
            // 共享状态机必须允许覆盖空绑定。
            .expect("binding should succeed again");
        // 销毁纹理时必须同时清除 sampler 身份。
        state.unbind_texture(TEXTURE);
        // 不允许留下只有 sampler 的半绑定状态。
        assert_eq!(state.sampled_binding(), None);
    }

    // 验证 end 原子清除所有 pass 级事实。
    #[test]
    fn end_clears_target_geometry_and_bindings() {
        // 创建并开始一个测试 pass。
        let mut state = RhiPassState::new();
        // 建立稳定目标范围。
        state
            // 使用保留载荷开始 pass。
            .begin(TARGET, RhiExtent::new(20, 10), LoadAction::Load)
            // 测试设置必须成功。
            .expect("pass should begin");
        // 写入一个合法 sampled binding。
        state
            // 使用不暴露槽位的原子绑定。
            .bind_sampled_texture(SampledTextureBinding::new(TEXTURE, SAMPLER))
            // 测试绑定必须成功。
            .expect("binding should succeed");
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
        // end 后不能再读取陈旧纹理绑定。
        assert_eq!(state.sampled_binding(), None);
        // 重复 end 必须作为顺序错误被拒绝。
        assert!(state.end().is_err());
    }
}
