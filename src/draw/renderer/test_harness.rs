//! `test-harness` 图形控制与规范像素回读信号。
//!
//! 信号只在显式 feature 下编译；故障仍由窗口现有图形 FSM 执行，像素回读
//! 则由 Renderer 在已执行绘制、尚未 present 的 owner-thread 边界完成。
//! `agent-control` 复用同一回读信号实现协议截屏；故障注入入口在该构建下
//! 保持未接线，按 dead code 静默。

// agent-control-only 构建不接线故障注入，避免死代码告警淹没真实问题。
#![cfg_attr(not(feature = "test-harness"), allow(dead_code))]

// 引入一次性结果通道与共享信号同步原语。
use std::sync::{
    // 共享信号使用引用计数，待处理请求使用互斥所有权。
    Arc,
    Mutex,
    // 原子标记只承载附着与一次性故障状态。
    atomic::{AtomicBool, Ordering},
    // 使用标准通道把 owner-thread 结果交还测试线程。
    mpsc::{self, Receiver, RecvTimeoutError, Sender},
};
// 引入公开票据等待使用的有界时长。
use std::time::Duration;

use crate::core::{Errc, Error, Result};
// 只让应用观察 Drawing 层已经规范化的像素快照。
use crate::draw::SurfaceReadback;

/// 一次 surface 像素回读请求的异步结果票据。
///
/// 应在非 UI 线程等待；UI 线程负责执行下一帧并完成回读。
pub struct SurfaceReadbackTicket {
    // 独占一次性结果接收端，避免同一帧结果被多方消费。
    receiver: Receiver<Result<SurfaceReadback>>,
}

// 为公开票据提供有界等待，禁止测试无限阻塞。
impl SurfaceReadbackTicket {
    /// 在给定时限内等待规范像素快照。
    pub fn recv_timeout(self, timeout: Duration) -> Result<SurfaceReadback> {
        // 把通道状态映射成稳定的 UIX typed error。
        match self.receiver.recv_timeout(timeout) {
            // owner thread 完成后原样返回像素或底层 typed failure。
            Ok(result) => result,
            // 超时只表示目标帧尚未完成，不伪造空图像。
            Err(RecvTimeoutError::Timeout) => Err(Error::new(
                // 使用通用超时分类供自动验收稳定判断。
                Errc::Timeout,
                // 诊断明确指出等待的测试边界。
                "surface readback did not complete before the timeout",
            )),
            // 发送端消失表示窗口或渲染器已结束生命周期。
            Err(RecvTimeoutError::Disconnected) => Err(Error::new(
                // 生命周期提前结束属于无效状态。
                Errc::InvalidState,
                // 不把通道实现细节暴露给应用测试。
                "surface readback ended before producing a result",
            )),
        }
    }
}

// 保存由 Renderer owner thread 唯一完成的一次请求。
pub(crate) struct SurfaceReadbackRequest {
    // 发送端只在最终 present 结果确定后消费。
    sender: Sender<Result<SurfaceReadback>>,
}

// 将完成动作封装在请求值上，避免 Renderer 接触通道细节。
impl SurfaceReadbackRequest {
    // 消费请求并尝试交付结果；票据已丢弃时无需影响渲染。
    pub(crate) fn complete(self, result: Result<SurfaceReadback>) {
        // 调试日志只记录完成边界与成功状态，不输出像素载荷。
        tracing::debug!(
            // 使用稳定事件名便于定位多帧请求时序。
            success = result.is_ok(),
            // 说明一次 owner-thread 回读事务已经结束。
            "surface readback request completed"
        );
        // 测试接收端提前释放不是图形运行时失败。
        let _ = self.sender.send(result);
    }
}

#[derive(Clone, Default)]
pub(crate) struct GraphicsFaultSignal {
    inner: Arc<GraphicsFaultSignalInner>,
}

#[derive(Default)]
struct GraphicsFaultSignalInner {
    attached: AtomicBool,
    device_lost_pending: AtomicBool,
    surface_lost_pending: AtomicBool,
    // 标记当前窗口是否拥有能消费回读请求的 Renderer。
    readback_attached: AtomicBool,
    // 每个窗口同一时刻只允许一个待完成回读请求。
    readback_pending: Mutex<Option<SurfaceReadbackRequest>>,
}

impl GraphicsFaultSignal {
    pub(crate) fn attach_recovery_driver(&self) {
        self.inner.attached.store(true, Ordering::Release);
    }

    pub(crate) fn arm_device_lost(&self) -> Result<()> {
        if !self.inner.attached.load(Ordering::Acquire) {
            return Err(Error::new(
                Errc::NotImplemented,
                "graphics fault injection requires an active recoverable GPU engine",
            ));
        }
        self.inner
            .device_lost_pending
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                Error::new(
                    Errc::InvalidState,
                    "a graphics device-lost injection is already pending",
                )
            })?;
        Ok(())
    }

    pub(crate) fn take_device_lost(&self) -> bool {
        self.inner.device_lost_pending.swap(false, Ordering::AcqRel)
    }

    // 安排下一次帧边界把 surface-lost 注入送入真实 adapter。
    pub(crate) fn arm_surface_lost(&self) -> Result<()> {
        // 没有恢复包装器时，不能把故障信号送入不存在的 owner thread。
        if !self.inner.attached.load(Ordering::Acquire) {
            return Err(Error::new(
                Errc::NotImplemented,
                "graphics fault injection requires an active recoverable GPU engine",
            ));
        }
        // 禁止同一时刻重复安排 surface 故障，保持一次注入一次恢复。
        self.inner
            .surface_lost_pending
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                Error::new(
                    Errc::InvalidState,
                    "a graphics surface-lost injection is already pending",
                )
            })?;
        Ok(())
    }

    // 读取并消费已经安排的 surface-lost 信号。
    pub(crate) fn take_surface_lost(&self) -> bool {
        self.inner
            .surface_lost_pending
            .swap(false, Ordering::AcqRel)
    }

    // 在 Renderer 装配期声明该窗口具备帧边界回读消费者。
    pub(crate) fn attach_surface_readback(&self) {
        // Release 保证随后请求能观察到完整 Renderer 装配。
        self.inner.readback_attached.store(true, Ordering::Release);
    }

    // 从任意应用线程安排下一次成功帧的规范 surface 回读。
    pub(crate) fn request_surface_readback(&self) -> Result<SurfaceReadbackTicket> {
        // 没有 Renderer 消费者时立即返回能力缺失，避免悬挂票据。
        if !self.inner.readback_attached.load(Ordering::Acquire) {
            // 返回稳定的可选能力错误。
            return Err(Error::new(
                // 未装配消费者属于未实现的测试能力。
                Errc::NotImplemented,
                // 诊断不泄漏具体后端或平台。
                "surface readback requires an active renderer",
            ));
        }
        // 为本次请求创建唯一结果通道。
        let (sender, receiver) = mpsc::channel();
        // 锁住待处理槽位，使重复请求得到确定结果。
        let mut pending = self
            // 借用共享请求槽位。
            .inner
            // 进入互斥区。
            .readback_pending
            // 等待当前短临界区。
            .lock()
            // poison 时仍恢复所有权，避免测试控制面永久失效。
            .unwrap_or_else(|error| error.into_inner());
        // 单槽位非空时拒绝覆盖先前票据。
        if pending.is_some() {
            // 返回可重试的状态冲突。
            return Err(Error::new(
                // 重复安排属于当前状态不允许的操作。
                Errc::InvalidState,
                // 稳定说明窗口已有等待中的请求。
                "a surface readback request is already pending",
            ));
        }
        // 把唯一发送端交给 owner-thread Renderer。
        *pending = Some(SurfaceReadbackRequest { sender });
        // 记录请求进入共享单槽的时序，不暴露应用或像素数据。
        tracing::debug!("surface readback request queued");
        // 返回只拥有接收端的公开票据。
        Ok(SurfaceReadbackTicket { receiver })
    }

    // 由 Renderer 在适合回读的帧边界消费一次待处理请求。
    pub(crate) fn take_surface_readback(&self) -> Option<SurfaceReadbackRequest> {
        // 只在短临界区内转移请求所有权。
        let request = self
            .inner
            // 借用共享请求槽位。
            .readback_pending
            // 等待当前短临界区。
            .lock()
            // poison 时继续保证请求可以被完成。
            .unwrap_or_else(|error| error.into_inner())
            // 清空槽位并返回原请求。
            .take();
        // 记录当前最终帧边界是否取得请求，用于诊断跨线程竞争。
        tracing::debug!(
            // 只报告布尔状态，避免测试控制对象逸出。
            pending = request.is_some(),
            // 使用稳定事件名关联排队与完成日志。
            "surface readback request sampled"
        );
        // 返回唯一请求所有权或明确空值。
        request
    }

    // 失败路径取消仍在排队的请求；已被 owner thread 取走的请求无法撤回，
    // 会在下一帧完成时静默丢弃（接收端已释放）。
    pub(crate) fn cancel_surface_readback(&self) {
        let Some(request) = self
            .inner
            // 借用共享请求槽位。
            .readback_pending
            // 等待当前短临界区。
            .lock()
            // poison 时槽位内容仍然可以安全清理。
            .unwrap_or_else(|error| error.into_inner())
            // 取出唯一待处理请求；槽位为空说明请求已被 owner thread 取走。
            .take()
        else {
            return;
        };
        // 以稳定的取消错误完成票据，调用方按未捕获结果处理。
        request.complete(Err(Error::new(
            // 取消属于调用方主动放弃，不是图形失败。
            Errc::InvalidState,
            // 诊断固定在取消边界。
            "surface readback request was cancelled",
        )));
    }
}
