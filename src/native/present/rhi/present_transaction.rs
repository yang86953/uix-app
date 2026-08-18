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

// 仅验证跨后端共享的呈现门禁和错误映射。
#[cfg(test)]
mod tests {
    // 引入 Surface 保留语义、最终 damage 和错误分类。
    use crate::core::{Errc, PresentCoherency, PresentDamage};

    // 引入被测事务及其共享身份类型。
    use super::{
        RenderTargetHandle, RhiPresentTransaction, RhiSubmissionSequence, SubmissionHandle,
        SurfaceFrame, SurfaceToken,
    };
    // 引入测试 Surface 尺寸。
    use crate::native::present::rhi::RhiExtent;

    // 固定测试 Surface token。
    const TOKEN: SurfaceToken = SurfaceToken::new(4, RhiExtent::new(80, 50));
    // 固定可以保留窄呈现的测试 Surface 语义。
    const TRACKED: PresentCoherency = PresentCoherency::TrackedSwapchain;
    // 创建一次使用最新提交身份和指定 damage 的事务。
    fn transaction_with_damage(
        // 借用可签发提交的共享序列。
        submissions: &mut RhiSubmissionSequence,
        // 接收本次要验证的最终 damage。
        damage: PresentDamage,
    ) -> RhiPresentTransaction {
        // 签发唯一最新提交身份。
        let submission = submissions.issue().expect("submission should be issued");
        // 绑定同代际 frame、最新提交和调用方 damage。
        RhiPresentTransaction::new(
            // 使用当前 Surface frame。
            SurfaceFrame::new(TOKEN),
            // 使用共享序列刚签发的身份。
            submission,
            // 保留不可拆事务中的 damage 输入。
            damage,
        )
    }
    // 创建一次使用最新提交身份的有效事务。
    fn valid_transaction(
        // 借用可签发提交的共享序列。
        submissions: &mut RhiSubmissionSequence,
    ) -> RhiPresentTransaction {
        // 复用可指定 damage 的唯一测试构造边界。
        transaction_with_damage(submissions, PresentDamage::Full)
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
            .validate(TOKEN, TRACKED, &submissions)
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
            .validate(
                // 使用新代际的同尺寸 Surface。
                SurfaceToken::new(5, TOKEN.extent),
                // coherency 不能改变 frame 代际门禁。
                TRACKED,
                // 使用原提交序列。
                &submissions,
            )
            // 旧 frame 必须失败。
            .expect_err("stale frame must be rejected");
        // 错误分类必须驱动统一 Surface 恢复状态机。
        assert_eq!(error.code(), Errc::GraphicsSurfaceLost);
    }

    // 验证 acquired frame 的目标种类在类型层固定为 Surface。
    #[test]
    fn acquired_frame_always_exposes_surface_target() {
        // 构造唯一公开形状的 acquired frame。
        let frame = SurfaceFrame::new(TOKEN);
        // frame 目标必须是无 texture 身份的 Surface 变体。
        assert_eq!(frame.target(), RenderTargetHandle::surface());
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
            .validate(TOKEN, TRACKED, &submissions)
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
            SurfaceFrame::new(TOKEN),
            // 零值不属于任何成功 Device submit。
            SubmissionHandle::from_raw(0),
            // damage 不影响提交门禁。
            PresentDamage::Full,
        );
        // 执行共享门禁。
        let error = transaction
            // Surface 身份完全匹配，确保失败只来自提交。
            .validate(TOKEN, TRACKED, &submissions)
            // 零提交必须失败。
            .expect_err("zero submission must be rejected");
        // 零提交同样属于调用参数错误。
        assert_eq!(error.code(), Errc::InvalidArgument);
    }

    // 验证可保留 Surface 会发布范围内的规范 partial damage。
    #[test]
    fn tracked_surface_preserves_valid_partial_damage() {
        // 创建当前 context 的提交序列。
        let mut submissions = RhiSubmissionSequence::new();
        // 构造一个完整位于 80×50 drawable 的物理矩形。
        let damage = PresentDamage::Partial(vec![(4, 5, 10, 12)]);
        // 把矩形绑定到最新提交事务。
        let transaction = transaction_with_damage(&mut submissions, damage.clone());
        // 使用 tracked 保留能力执行共享门禁。
        let present = transaction
            // 同时提供当前 token、Surface 能力和提交序列。
            .validate(TOKEN, TRACKED, &submissions)
            // 合法窄呈现必须通过。
            .expect("tracked partial damage should validate");
        // Adapter 只能看到同一规范化矩形集。
        assert_eq!(present.damage(), &damage);
    }

    // 验证 FullOnly Surface 在进入 Adapter 前就将 partial 降级为完整呈现。
    #[test]
    fn full_only_surface_normalizes_partial_damage() {
        // 创建当前 context 的提交序列。
        let mut submissions = RhiSubmissionSequence::new();
        // 构造几何合法但 Surface 能力不承诺的 partial。
        let transaction = transaction_with_damage(
            // 签发并绑定本 context 的最新提交。
            &mut submissions,
            // 矩形本身完整位于 Surface 内。
            PresentDamage::Partial(vec![(4, 5, 10, 12)]),
        );
        // 使用 OpenGL 与 legacy DXGI 共用的 FullOnly 能力执行门禁。
        let present = transaction
            // 不允许 Adapter 自行忽略 partial。
            .validate(TOKEN, PresentCoherency::FullOnly, &submissions)
            // 能力降级应以完整呈现成功，不是失败。
            .expect("full-only damage should normalize");
        // 已验证事务只能发布 Full。
        assert!(present.damage().is_full());
    }

    // 验证空集、越界与超量 partial 共用同一保守回退。
    #[test]
    fn invalid_partial_damage_falls_back_before_adapter() {
        // 创建当前 context 的提交序列。
        let mut submissions = RhiSubmissionSequence::new();
        // 依次覆盖空集、越过右边界和超过共享上限的输入。
        for damage in [
            // 空 partial 不能表示成功却未更新任何像素。
            PresentDamage::Partial(Vec::new()),
            // 宽度超过 80 像素 Surface 右边界。
            PresentDamage::Partial(vec![(70, 0, 20, 10)]),
            // 六十五个不相交矩形超过共享规范上限。
            PresentDamage::Partial((0..65).map(|x| (x, 0, 1, 1)).collect()),
        ] {
            // 为当前输入签发一个新的最新提交事务。
            let transaction = transaction_with_damage(&mut submissions, damage);
            // 使用本可以窄呈现的 tracked Surface 证明失败来自几何。
            let present = transaction
                // 由共享门禁消费所有动态事实。
                .validate(TOKEN, TRACKED, &submissions)
                // 保守回退应继续完整呈现。
                .expect("invalid partial damage should fall back");
            // 原生 Adapter 不得看到任何非法 partial。
            assert!(present.damage().is_full());
        }
    }
}
