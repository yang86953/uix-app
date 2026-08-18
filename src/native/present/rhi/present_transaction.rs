//! 图形 API 无关的 Surface acquire 到 present 事务契约。
//!
//! Frame、Device submit 身份和最终 damage 必须作为一个不可拆值进入 Surface；
//! 共享门禁统一拒绝旧代际、错误目标和迟到提交，Adapter 只执行原生呈现。

// 引入最终呈现损伤值。
use crate::core::PresentDamage;
// 引入统一错误码、错误值和结果类型。
use crate::core::error::{Errc, Error, Result};

// 引入共享目标、提交、Surface 代际和提交序列类型。
use super::{RenderTargetHandle, RhiSubmissionSequence, SubmissionHandle, SurfaceToken};

// 描述一次 acquire 得到的 Surface image。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SurfaceFrame {
    // 保存 acquire 时的 Surface token。
    token: SurfaceToken,
    // 保存 Adapter 绑定的 Surface render target。
    target: RenderTargetHandle,
}

// 为 Surface frame 提供封闭构造和只读事实。
impl SurfaceFrame {
    // 创建一个带代际和目标句柄的 acquired frame。
    pub(crate) const fn new(token: SurfaceToken, target: RenderTargetHandle) -> Self {
        // 返回不可变的 acquired image 身份。
        Self { token, target }
    }

    // 返回 acquire 时冻结的 Surface token。
    pub(crate) const fn token(self) -> SurfaceToken {
        // token 按值安全复制。
        self.token
    }

    // 返回 acquire 时冻结的 Surface target。
    pub(crate) const fn target(self) -> RenderTargetHandle {
        // 不透明目标身份按值安全复制。
        self.target
    }

    // 验证 frame 仍属于当前 Surface 和 acquire 目标。
    fn validate_current(
        // 按值消费不可变 frame 身份。
        self,
        // 接收 Adapter 当前 Surface token。
        current_token: SurfaceToken,
        // 接收 Adapter 唯一保留的 Surface target。
        expected_target: RenderTargetHandle,
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
        // 离屏或其它 Surface target 不得进入最终呈现。
        if self.target != expected_target {
            // 错误目标属于调用契约违例。
            return Err(Error::new(
                // 两个 Adapter 共用同一参数错误分类。
                Errc::InvalidArgument,
                // 保持跨后端稳定的诊断文本。
                "RHI present requires the acquired surface target",
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
    // 保存原样交给原生呈现入口的最终 damage。
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
        // 接收 Adapter 保留的唯一 Surface target。
        expected_target: RenderTargetHandle,
        // 借用同一组合 context 的 Device 提交序列。
        submissions: &RhiSubmissionSequence,
    ) -> Result<ValidatedRhiPresent> {
        // 先验证 Surface image 代际与目标身份。
        self.frame
            // 所有 Adapter 都必须调用同一 frame 门禁。
            .validate_current(current_token, expected_target)?;
        // 再验证 Device submit 与 Surface present 的最新值关联。
        submissions.validate(self.submission)?;
        // 只有全部门禁通过后才发布 damage 给 Adapter。
        Ok(ValidatedRhiPresent {
            // 已验证事务不再暴露 frame 或 submission 的拆分入口。
            damage: self.damage,
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

// 仅验证跨后端共享的呈现门禁和错误映射。
#[cfg(test)]
mod tests {
    // 引入最终 damage 和错误分类。
    use crate::core::{Errc, PresentDamage};

    // 引入被测事务及其共享身份类型。
    use super::{
        RenderTargetHandle, RhiPresentTransaction, RhiSubmissionSequence, SubmissionHandle,
        SurfaceFrame, SurfaceToken,
    };
    // 引入测试 Surface 尺寸。
    use crate::native::present::rhi::RhiExtent;

    // 固定测试 Surface token。
    const TOKEN: SurfaceToken = SurfaceToken::new(4, RhiExtent::new(80, 50));
    // 固定测试 Surface target。
    const TARGET: RenderTargetHandle = RenderTargetHandle::from_raw(9);

    // 创建一次使用最新提交身份的有效事务。
    fn valid_transaction(
        // 借用可签发提交的共享序列。
        submissions: &mut RhiSubmissionSequence,
    ) -> RhiPresentTransaction {
        // 签发唯一最新提交身份。
        let submission = submissions.issue().expect("submission should be issued");
        // 绑定同代际 frame、最新提交和完整呈现。
        RhiPresentTransaction::new(
            // 使用当前 Surface frame。
            SurfaceFrame::new(TOKEN, TARGET),
            // 使用共享序列刚签发的身份。
            submission,
            // 使用不需要平台解释的完整 damage。
            PresentDamage::Full,
        )
    }

    // 验证完整匹配的事务才会发布 Adapter damage。
    #[test]
    fn present_transaction_accepts_current_frame_and_latest_submission() {
        // 创建当前 context 的提交序列。
        let mut submissions = RhiSubmissionSequence::new();
        // 创建有效呈现事务。
        let transaction = valid_transaction(&mut submissions);
        // 通过共享门禁取得已验证事务。
        let validated = transaction
            // 提供当前 Surface 的唯一事实。
            .validate(TOKEN, TARGET, &submissions)
            // 有效事务必须通过。
            .expect("current present transaction should validate");
        // 已验证事务必须原样保留完整 damage。
        assert!(validated.damage().is_full());
    }

    // 验证旧代际 frame 在两个 Adapter 之前统一返回 Surface lost。
    #[test]
    fn present_transaction_rejects_stale_surface_frame() {
        // 创建当前 context 的提交序列。
        let mut submissions = RhiSubmissionSequence::new();
        // 创建使用第四代 frame 的事务。
        let transaction = valid_transaction(&mut submissions);
        // 使用第五代当前 Surface 触发旧 frame 门禁。
        let error = transaction
            // extent 不变也必须由 generation 区分重建。
            .validate(SurfaceToken::new(5, TOKEN.extent), TARGET, &submissions)
            // 旧 frame 必须失败。
            .expect_err("stale frame must be rejected");
        // 错误分类必须驱动统一 Surface 恢复状态机。
        assert_eq!(error.code(), Errc::GraphicsSurfaceLost);
    }

    // 验证离屏 target 不能误走最终 Surface present。
    #[test]
    fn present_transaction_rejects_non_surface_target() {
        // 创建并签发当前提交。
        let mut submissions = RhiSubmissionSequence::new();
        // 创建 frame target 与 Adapter 期望不同的事务。
        let transaction = RhiPresentTransaction::new(
            // 使用非 Surface 目标身份。
            SurfaceFrame::new(TOKEN, RenderTargetHandle::from_raw(10)),
            // 签发仍然有效的提交身份。
            submissions.issue().expect("submission should be issued"),
            // damage 不影响目标门禁。
            PresentDamage::Full,
        );
        // 验证错误目标。
        let error = transaction
            // 传入真实 Surface target。
            .validate(TOKEN, TARGET, &submissions)
            // 错误目标必须失败。
            .expect_err("non-surface target must be rejected");
        // 错误目标属于稳定参数错误。
        assert_eq!(error.code(), Errc::InvalidArgument);
    }

    // 验证下一次 submit 会让未呈现的旧事务失效。
    #[test]
    fn present_transaction_rejects_stale_submission() {
        // 创建当前 context 的提交序列。
        let mut submissions = RhiSubmissionSequence::new();
        // 创建绑定第一份提交的事务。
        let transaction = valid_transaction(&mut submissions);
        // 签发更新提交，使事务中的身份变成迟到值。
        let _latest = submissions
            .issue()
            .expect("latest submission should be issued");
        // 通过同代际与同目标进入提交门禁。
        let error = transaction
            // 提交序列必须拒绝旧身份。
            .validate(TOKEN, TARGET, &submissions)
            // 迟到提交必须失败。
            .expect_err("stale submission must be rejected");
        // 迟到身份属于稳定参数错误。
        assert_eq!(error.code(), Errc::InvalidArgument);
    }

    // 验证零提交身份不能绕过尚未签发的序列。
    #[test]
    fn present_transaction_rejects_zero_submission() {
        // 创建尚未签发提交的序列。
        let submissions = RhiSubmissionSequence::new();
        // 显式构造零提交身份事务。
        let transaction = RhiPresentTransaction::new(
            // frame 仍然保持有效。
            SurfaceFrame::new(TOKEN, TARGET),
            // 零值不属于任何成功 Device submit。
            SubmissionHandle::from_raw(0),
            // damage 不影响提交门禁。
            PresentDamage::Full,
        );
        // 执行共享门禁。
        let error = transaction
            // Surface 身份完全匹配，确保失败只来自提交。
            .validate(TOKEN, TARGET, &submissions)
            // 零提交必须失败。
            .expect_err("zero submission must be rejected");
        // 零提交同样属于调用参数错误。
        assert_eq!(error.code(), Errc::InvalidArgument);
    }
}
