//! 跨图形 Adapter 共享的 Surface 重建状态机。
//!
//! 原生 API 只负责报告 acquisition/presentation 结果并机械执行重建；是否保留
//! 当前帧、推进 generation 或把已呈现的次优状态视为成功，由本组件统一决定。

use crate::core::error::{Errc, Error, Result};

use super::{RhiExtent, SurfaceToken};

// 描述触发一次 Surface 原生重建的 API 无关原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RhiSurfaceRecreateReason {
    // 首次创建可呈现 Surface。
    Initialize,
    // 调用方提交了新的正物理尺寸。
    Resize,
    // acquire 在取得 image 前确认旧 Surface 已失效。
    AcquisitionRejected,
    // present 未能证明当前帧已经呈现。
    PresentationRejected,
    // 当前帧已经呈现，但 Surface 要求后续使用新代际。
    PresentedNeedsRecreate,
}

// 描述成功重建后当前帧应如何收尾。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RhiSurfaceRecreateCommit {
    // 初始化或显式 resize 已完成，调用方可以继续正常生命周期。
    Ready(SurfaceToken),
    // 旧帧未呈现；下一帧必须用新 token 重新生成 FramePlan。
    RetryFrame(SurfaceToken),
    // 当前帧已呈现；重建只影响后续帧，不得把本帧改写为失败。
    Presented(SurfaceToken),
}

impl RhiSurfaceRecreateCommit {
    // 返回重建后已经发布的新 Surface token。
    pub(crate) const fn token(self) -> SurfaceToken {
        match self {
            Self::Ready(token) | Self::RetryFrame(token) | Self::Presented(token) => token,
        }
    }

    // 把原生失效原因链接到稳定的“已重建、重试帧”错误。
    pub(crate) fn complete_frame(self, source: Error) -> Result<()> {
        match self {
            // 初始化、resize 与已呈现后重建都没有未提交帧。
            Self::Ready(_) | Self::Presented(_) => Ok(()),
            // acquire/present 拒绝的旧帧必须保留 dirty，并在新代际重试。
            Self::RetryFrame(_) => Err(Error::new(
                Errc::GraphicsSurfaceChanged,
                "RHI surface was rebuilt before the frame could be presented",
            )
            .with_source(source)),
        }
    }
}

// 保存一次已经进入 Recreating 状态的封闭事务身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RhiSurfaceRecreateTransaction {
    id: u64,
    requested: RhiExtent,
    reason: RhiSurfaceRecreateReason,
    native_width: i32,
    native_height: i32,
}

impl RhiSurfaceRecreateTransaction {
    // 返回 Adapter 应交给原生 swapchain 创建入口的请求尺寸。
    pub(crate) const fn requested(self) -> RhiExtent {
        self.requested
    }

    // 返回已经由共享生命周期证明可进入原生 ABI 的尺寸。
    pub(crate) const fn native_size_i32(self) -> (i32, i32) {
        (self.native_width, self.native_height)
    }

    // 返回 Adapter 机械选择原生重建操作所需的共享原因。
    pub(crate) const fn reason(self) -> RhiSurfaceRecreateReason {
        self.reason
    }
}

// 保存 Surface 当前是否允许 acquire，以及在途重建身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RhiSurfaceLifecycleState {
    Uninitialized,
    Active,
    Recreating(u64),
    Invalidated(RhiSurfaceRecreateReason),
}

// 唯一拥有 Surface generation、extent 与重建事务顺序的共享状态机。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RhiSurfaceLifecycle {
    token: SurfaceToken,
    state: RhiSurfaceLifecycleState,
    next_transaction_id: u64,
}

impl RhiSurfaceLifecycle {
    // 创建尚未拥有原生 swapchain 的状态；首次事务必须是 Initialize。
    pub(crate) const fn uninitialized(requested: RhiExtent) -> Self {
        Self {
            token: SurfaceToken::new(0, requested),
            state: RhiSurfaceLifecycleState::Uninitialized,
            next_transaction_id: 1,
        }
    }

    // 返回最后一次成功发布的 Surface token。
    pub(crate) const fn token(self) -> SurfaceToken {
        self.token
    }

    // 在 Adapter 读取 token 或触碰原生 Surface 前确认当前代际仍可使用。
    pub(crate) fn ensure_active(&self) -> Result<()> {
        if self.state != RhiSurfaceLifecycleState::Active {
            return Err(Error::new(
                Errc::InvalidState,
                "RHI surface lifecycle is not active",
            ));
        }
        Ok(())
    }

    // 在任何原生销毁或创建前原子进入 Recreating。
    pub(crate) fn begin_recreate(
        &mut self,
        requested: RhiExtent,
        reason: RhiSurfaceRecreateReason,
    ) -> Result<RhiSurfaceRecreateTransaction> {
        let Some((native_width, native_height)) = requested.native_size_i32() else {
            return Err(Error::new(
                Errc::InvalidArgument,
                "RHI surface recreate extent is invalid",
            ));
        };
        match (self.state, reason) {
            (RhiSurfaceLifecycleState::Uninitialized, RhiSurfaceRecreateReason::Initialize) => {}
            (RhiSurfaceLifecycleState::Uninitialized, _) => {
                return Err(Error::new(
                    Errc::InvalidState,
                    "RHI surface must be initialized before recreation",
                ));
            }
            (RhiSurfaceLifecycleState::Active, RhiSurfaceRecreateReason::Initialize)
            | (RhiSurfaceLifecycleState::Invalidated(_), RhiSurfaceRecreateReason::Initialize) => {
                return Err(Error::new(
                    Errc::InvalidState,
                    "RHI surface cannot be initialized twice",
                ));
            }
            (RhiSurfaceLifecycleState::Recreating(_), _) => {
                return Err(Error::new(
                    Errc::InvalidState,
                    "RHI surface recreation is already in progress",
                ));
            }
            (RhiSurfaceLifecycleState::Active, _)
            | (RhiSurfaceLifecycleState::Invalidated(_), _) => {}
        }
        let id = self.next_transaction_id;
        self.next_transaction_id = self.next_transaction_id.wrapping_add(1).max(1);
        self.state = RhiSurfaceLifecycleState::Recreating(id);
        Ok(RhiSurfaceRecreateTransaction {
            id,
            requested,
            reason,
            native_width,
            native_height,
        })
    }

    // 原生重建成功后一次发布新 extent、generation 与帧收尾语义。
    pub(crate) fn commit_recreate(
        &mut self,
        transaction: RhiSurfaceRecreateTransaction,
        actual: RhiExtent,
    ) -> Result<RhiSurfaceRecreateCommit> {
        self.validate_transaction(transaction)?;
        if !actual.is_valid() {
            self.state = RhiSurfaceLifecycleState::Invalidated(transaction.reason);
            return Err(Error::new(
                Errc::PlatformError,
                "RHI surface recreate produced an invalid extent",
            ));
        }
        let generation = if transaction.reason == RhiSurfaceRecreateReason::Initialize {
            self.token.generation
        } else {
            self.token.generation.saturating_add(1)
        };
        self.token = SurfaceToken::new(generation, actual);
        self.state = RhiSurfaceLifecycleState::Active;
        Ok(match transaction.reason {
            RhiSurfaceRecreateReason::Initialize | RhiSurfaceRecreateReason::Resize => {
                RhiSurfaceRecreateCommit::Ready(self.token)
            }
            RhiSurfaceRecreateReason::AcquisitionRejected
            | RhiSurfaceRecreateReason::PresentationRejected => {
                RhiSurfaceRecreateCommit::RetryFrame(self.token)
            }
            RhiSurfaceRecreateReason::PresentedNeedsRecreate => {
                RhiSurfaceRecreateCommit::Presented(self.token)
            }
        })
    }

    // 原生重建失败后保留旧 token，但禁止继续把它当成可 acquire 的代际。
    pub(crate) fn abort_recreate(
        &mut self,
        transaction: RhiSurfaceRecreateTransaction,
    ) -> Result<()> {
        self.validate_transaction(transaction)?;
        self.state = RhiSurfaceLifecycleState::Invalidated(transaction.reason);
        Ok(())
    }

    // 验证完成或回滚操作只消费当前唯一在途事务。
    fn validate_transaction(&self, transaction: RhiSurfaceRecreateTransaction) -> Result<()> {
        if self.state != RhiSurfaceLifecycleState::Recreating(transaction.id) {
            return Err(Error::new(
                Errc::InvalidState,
                "RHI surface recreate transaction is stale",
            ));
        }
        Ok(())
    }
}

// 让显式 Vulkan 验证 feature 在不创建窗口或原生对象时执行共享状态机。
#[cfg(feature = "vulkan-parity-test")]
pub(crate) fn run_surface_lifecycle_contract_test() {
    let initial_extent = RhiExtent::new(640, 480);
    let mut lifecycle = RhiSurfaceLifecycle::uninitialized(initial_extent);
    let initial = lifecycle
        .begin_recreate(initial_extent, RhiSurfaceRecreateReason::Initialize)
        .unwrap_or_else(|error| panic!("surface initialize transaction failed: {error}"));
    let ready = lifecycle
        .commit_recreate(initial, initial_extent)
        .unwrap_or_else(|error| panic!("surface initialize commit failed: {error}"));
    assert!(matches!(ready, RhiSurfaceRecreateCommit::Ready(_)));

    let out_of_date = lifecycle
        .begin_recreate(
            initial_extent,
            RhiSurfaceRecreateReason::AcquisitionRejected,
        )
        .unwrap_or_else(|error| panic!("OUT_OF_DATE transaction failed: {error}"));
    let retry = lifecycle
        .commit_recreate(out_of_date, initial_extent)
        .unwrap_or_else(|error| panic!("OUT_OF_DATE commit failed: {error}"));
    let retry_error = retry
        .complete_frame(Error::new(
            Errc::GraphicsSurfaceLost,
            "deterministic OUT_OF_DATE status",
        ))
        .expect_err("OUT_OF_DATE must preserve dirty state and retry");
    assert_eq!(retry_error.code(), Errc::GraphicsSurfaceChanged);

    let suboptimal = lifecycle
        .begin_recreate(
            initial_extent,
            RhiSurfaceRecreateReason::PresentedNeedsRecreate,
        )
        .unwrap_or_else(|error| panic!("SUBOPTIMAL transaction failed: {error}"));
    let presented = lifecycle
        .commit_recreate(suboptimal, initial_extent)
        .unwrap_or_else(|error| panic!("SUBOPTIMAL commit failed: {error}"));
    presented
        .complete_frame(Error::new(
            Errc::GraphicsSurfaceLost,
            "deterministic SUBOPTIMAL status",
        ))
        .expect("SUBOPTIMAL must keep the already-presented frame successful");
    assert_eq!(lifecycle.token().generation, 2);
}
