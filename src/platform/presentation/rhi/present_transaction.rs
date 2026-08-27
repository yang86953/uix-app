//! 图形 API 无关的 Surface acquire 到 present 事务契约。
//!
//! Frame、Device submit 身份和最终 damage 必须作为一个不可拆值进入 Surface；
//! 共享门禁统一拒绝旧代际和迟到提交，Surface 目标由封闭类型保证，
//! Adapter 只执行原生呈现。

// 引入 Surface 保留能力与最终呈现损伤值。
use crate::core::{PresentCoherency, PresentDamage};
// 引入统一错误码、错误值和结果类型。
use crate::core::error::{Errc, Error, Result};

// 引入共享目标、提交、Surface 代际和提交序列类型。
use super::{RenderTargetHandle, RhiSubmissionSequence, SubmissionHandle, SurfaceToken};

// 描述一次 acquire 得到的 Surface image。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SurfaceFrame {
    // 保存 acquire 时的 Surface token。
    token: SurfaceToken,
}

// 为 Surface frame 提供封闭构造和只读事实。
impl SurfaceFrame {
    // 创建一个只能代表当前 Surface 的 acquired frame。
    pub(crate) const fn new(token: SurfaceToken) -> Self {
        // 返回不可变的 acquired image 身份。
        Self { token }
    }

    // 返回 acquire 时冻结的 Surface token。
    pub(crate) const fn token(self) -> SurfaceToken {
        // token 按值安全复制。
        self.token
    }

    // 返回 acquire 时冻结的 Surface target。
    pub(crate) const fn target(self) -> RenderTargetHandle {
        // acquire frame 只能代表原生 Surface，错误目标在类型层不可表示。
        RenderTargetHandle::surface()
    }

    // 验证 frame 仍属于当前 Surface。
    fn validate_current(
        // 按值消费不可变 frame 身份。
        self,
        // 接收 Adapter 当前 Surface token。
        current_token: SurfaceToken,
    ) -> Result<()> {
        // 代际或 extent 任一变化都使旧 frame 失效。
        if self.token != current_token {
            // 统一映射为可恢复的 Surface lost。
            return Err(Error::new(
                // 上层恢复状态机只依赖稳定错误分类。
                Errc::GraphicsSurfaceLost,
                // 诊断文本不携带具体图形 API 名称。
                "RHI surface frame generation is stale",
            ));
        }
        // frame 身份与当前 Surface 完全一致。
        Ok(())
    }
}

// 绑定 acquired frame、Device submit 身份和最终 damage 的呈现命令。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RhiPresentTransaction {
    // 保存必须与当前 Surface 匹配的 acquired frame。
    frame: SurfaceFrame,
    // 保存必须来自同一组合 context 最新 submit 的身份。
    submission: SubmissionHandle,
    // 保存尚需按 Surface 能力与范围规范化的最终 damage。
    damage: PresentDamage,
}

// 为呈现事务提供封闭构造和唯一共享门禁。
impl RhiPresentTransaction {
    // 创建一次不可拆的 Device 到 Surface 呈现事务。
    pub(crate) const fn new(
        // 接收 acquire 返回的完整 frame。
        frame: SurfaceFrame,
        // 接收 Device submit 返回的身份。
        submission: SubmissionHandle,
        // 接收 FramePlan 决定的最终 damage。
        damage: PresentDamage,
    ) -> Self {
        // 只保存类型化事实，不在构造阶段读取动态 Surface 状态。
        Self {
            // 绑定 acquired image 身份。
            frame,
            // 绑定提交身份。
            submission,
            // 绑定呈现损伤。
            damage,
        }
    }

    // 验证 frame 与最新提交后生成 Adapter 可消费的已验证事务。
    pub(crate) fn validate(
        // 消费未验证事务，禁止验证后替换任一组成部分。
        self,
        // 接收 Adapter 当前 Surface token。
        current_token: SurfaceToken,
        // 接收当前 Surface 实际承诺的像素保留语义。
        present_coherency: PresentCoherency,
        // 借用同一组合 context 的 Device 提交序列。
        submissions: &RhiSubmissionSequence,
    ) -> Result<ValidatedRhiPresent> {
        // 先验证 Surface image 代际与目标身份。
        self.frame
            // 所有 Adapter 都必须调用同一 frame 门禁。
            .validate_current(current_token)?;
        // 再验证 Device submit 与 Surface present 的最新值关联。
        submissions.validate(self.submission)?;
        // 按 Surface 保留能力与当前物理范围生成唯一最终 damage。
        let damage = self.damage.normalize_for_surface(
            // 使用 Surface capability 提供的一致性事实。
            present_coherency,
            // 使用 acquire 同代 token 中的物理宽度。
            current_token.extent.width,
            // 使用 acquire 同代 token 中的物理高度。
            current_token.extent.height,
        );
        // 只有全部门禁通过后才发布规范化 damage 给 Adapter。
        Ok(ValidatedRhiPresent {
            // 已验证事务不再暴露 frame 或 submission 的拆分入口。
            damage,
        })
    }
}

// 保存已经通过共享 Surface 与提交门禁的原生呈现输入。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedRhiPresent {
    // 保存可以机械交给原生呈现入口的 damage。
    damage: PresentDamage,
}

// 只向 Adapter 暴露验证后的 damage 投影。
impl ValidatedRhiPresent {
    // 借用最终 damage，供不取得所有权的原生 API 使用。
    pub(crate) const fn damage(&self) -> &PresentDamage {
        // 返回验证后不可替换的 damage。
        &self.damage
    }

    // 取得最终 damage，供拥有式原生交换入口使用。
    pub(crate) fn into_damage(self) -> PresentDamage {
        // 消费已验证事务并移出 damage。
        self.damage
    }
}
