// ============================================================================
// platform/linux/wayland/cursor.rs — ICursor impl for WaylandBackend
// ============================================================================

// 共享 cursor intent 由 seat callback 与同步端口共同消费。
use std::sync::Arc;

// 平台位置值继续服务未实现的位置查询契约。
use crate::core::Point;
// callback invariant failure 进入 backend 统一 pending source。
use crate::diagnostics::PendingFailureSource;
// ICursor 是 App System 依赖的唯一窄端口。
use crate::platform::windowing::CursorType;
// 引入窄端口 trait。
use crate::platform::windowing::ICursor;
// typed error 区分能力缺失、关闭与 owner 损坏。
use crate::native::{Errc, Error, Result};

// wl_pointer.set_cursor(None) 提供受 serial 约束的隐藏操作。
use wayland_client::protocol::wl_pointer;
// cursor-shape device 枚举定义 compositor 可选择的标准形状。
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::Shape;
// 可选 manager global 为当前 pointer 创建短命令 device。
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_manager_v1::WpCursorShapeManagerV1;

// compat Main 保留当前 backend queue handle 与代理身份。
use super::compat::Main;
// cursor-state Component 独占可重放 intent 与 Enter serial。
use super::cursor_state::WaylandCursorState;
// backend 只在本 Adapter 内暴露协议 owner。
use super::WaylandBackend;

// 把平台无关形状穷尽映射到 cursor-shape 协议枚举。
fn protocol_shape(cursor: CursorType) -> Result<Shape> {
    // 每个公开标准形状都只映射一个协议值。
    match cursor {
        // 默认箭头使用协议 default。
        CursorType::Arrow => Ok(Shape::Default),
        // 文本输入使用 text。
        CursorType::IBeam => Ok(Shape::Text),
        // 十字准星保持同名协议语义。
        CursorType::Crosshair => Ok(Shape::Crosshair),
        // 可点击手形对应 CSS pointer。
        CursorType::Hand => Ok(Shape::Pointer),
        // 水平双向缩放对应 east-west。
        CursorType::ResizeH => Ok(Shape::EwResize),
        // 垂直双向缩放对应 north-south。
        CursorType::ResizeV => Ok(Shape::NsResize),
        // 东北到西南缩放保持对角线方向。
        CursorType::ResizeNE => Ok(Shape::NeswResize),
        // 西北到东南缩放保持对角线方向。
        CursorType::ResizeNW => Ok(Shape::NwseResize),
        // 移动操作使用 CSS move。
        CursorType::Move => Ok(Shape::Move),
        // 等待操作使用 wait。
        CursorType::Wait => Ok(Shape::Wait),
        // 禁止操作使用 not-allowed。
        CursorType::NotAllowed => Ok(Shape::NotAllowed),
        // 位图或调用方自定义形状不属于枚举协议能力。
        CursorType::Custom => Err(Error::new(
            // 保持稳定 capability absence，供 App 去重。
            Errc::NotImplemented,
            // 诊断明确只有自定义位图形状缺失。
            "WaylandBackend::set_cursor(Custom): custom cursor images are not implemented",
        )),
    }
}

// 使用短命 cursor-shape device 提交一次形状请求。
fn submit_shape(
    // manager 是构造期绑定的可选 global owner。
    manager: &Main<WpCursorShapeManagerV1>,
    // pointer 必须与 Enter serial 属于同一代理代次。
    pointer: &Main<wl_pointer::WlPointer>,
    // serial 必须来自该 pointer 最新一次 Enter。
    serial: u32,
    // shape 已由 Adapter 穷尽映射。
    shape: Shape,
    // 协议请求本身无同步错误返回。
) {
    // 每次提交创建只服务当前请求的 device handle。
    let device = manager.get_pointer(
        // 传入当前 pointer 原始代理。
        pointer.as_ref(),
        // child object 登记到同一 backend event queue。
        &manager.queue_handle(),
        // cursor-shape device 没有事件数据。
        (),
    );
    // compositor 只在 serial 仍是最新 Enter 时应用请求。
    device.set_shape(serial, shape);
    // 协议保证 destroy 后当前形状保持不变。
    device.destroy();
}

// pointer Enter callback 记录新授权并立即重放当前 intent。
pub(crate) fn apply_cursor_on_pointer_enter(
    // callback 自身提供与 serial 同代次的 pointer 代理。
    pointer: &Main<wl_pointer::WlPointer>,
    // 本次 wl_pointer.enter 的唯一授权。
    serial: u32,
    // 缺少可选 manager 时保留 compositor 默认光标。
    manager: Option<&Main<WpCursorShapeManagerV1>>,
    // Component 先发布 serial，再返回完整重放快照。
    state: &Arc<WaylandCursorState>,
    // 仅内部状态不变量失败需要进入 pending source。
    pending_failures: &PendingFailureSource,
) {
    // 新 Enter 永远覆盖旧 surface 的 serial。
    let snapshot = state.record_enter(serial);
    // 没有协议 global 时同步端口会报告稳定 NotImplemented，callback 不伪造动作。
    let Some(manager) = manager else {
        // compositor 默认 cursor 保持不变。
        return;
    };
    // 隐藏 intent 必须在每次 Enter 使用新 serial 重放。
    if !snapshot.visible {
        // 空 surface 是 wl_pointer 定义的隐藏操作。
        pointer.set_cursor(serial, None, 0, 0);
        // 隐藏重放完成，不创建 shape device。
        return;
    }
    // 可见 intent 必须映射为已登记标准形状。
    let shape = match protocol_shape(snapshot.cursor) {
        // 正常状态取得唯一协议值。
        Ok(shape) => shape,
        // Component 不应保存 Adapter 已拒绝的 Custom。
        Err(error) => {
            // 把不变量破坏交给 owner-thread Diagnostics。
            let _ = pending_failures.enqueue(Error::new(
                // callback 内部状态不一致属于 InvalidState。
                Errc::InvalidState,
                // 保留原始映射错误作为诊断文本。
                format!("Wayland cursor Enter replay failed: {}", error.message()),
            ));
            // 不向 compositor 提交猜测形状。
            return;
        }
    };
    // 使用本次 Enter serial 提交可见形状。
    submit_shape(manager, pointer, serial, shape);
}

// pointer Leave 与 capability teardown 共用 serial 失效端口。
pub(crate) fn clear_cursor_pointer_focus(state: &Arc<WaylandCursorState>) {
    // Component 保留形状和可见性 intent，只撤销过期协议授权。
    state.clear_enter();
}

// Wayland cursor Adapter 的同步端口实现。
impl WaylandBackend {
    // closed backend 不得再记录或提交 cursor intent。
    fn ensure_cursor_open(&self, operation: &str) -> Result<()> {
        // closed 事实由 backend owner-thread 唯一提交。
        if self.closed {
            // 返回稳定生命周期错误。
            return Err(Error::new(
                // 关闭后调用属于无效操作。
                Errc::InvalidOperation,
                // 保留具体 cursor 入口。
                format!("Wayland cursor {operation} requested after backend shutdown"),
            ));
        }
        // backend 仍可接受同步请求。
        Ok(())
    }

    // 取得可选 cursor-shape global，缺失时不发布本地 intent。
    fn cursor_shape_manager(&self, operation: &str) -> Result<&Main<WpCursorShapeManagerV1>> {
        // 构造期只读能力发现决定本 backend 生命周期的稳定结果。
        self.cursor_shape_manager.as_ref().ok_or_else(|| {
            // 返回可由 App 去重的 capability absence。
            Error::new(
                // optional global 缺失属于未实现能力。
                Errc::NotImplemented,
                // 诊断保留入口与具体协议名。
                format!("WaylandBackend::{operation}: wp_cursor_shape_manager_v1 is unavailable"),
            )
        })
    }

    // 使用当前 pointer owner 执行一个需要 Enter serial 的同步协议动作。
    fn with_active_cursor_pointer<T>(
        // 只在本次同步调用期间借用 backend。
        &self,
        // 诊断保留具体 cursor 入口。
        operation: &str,
        // 闭包在健康 pointer guard 内排队协议请求。
        apply: impl FnOnce(&Main<wl_pointer::WlPointer>) -> Result<T>,
    ) -> Result<T> {
        // pointer owner 损坏时不得恢复或猜测代理。
        let pointer = self.pointer.lock().map_err(|_| {
            // 返回稳定 owner 状态错误。
            Error::new(
                // poisoned slot 属于 InvalidState。
                Errc::InvalidState,
                // 保留同步入口与 pointer owner 阶段。
                format!("WaylandBackend::{operation}: pointer proxy slot mutex poisoned"),
            )
        })?;
        // serial 存在时必须同时持有同代次 pointer 代理。
        let pointer = pointer.as_ref().ok_or_else(|| {
            // 构造状态不变量错误。
            Error::new(
                // serial 与 pointer 不一致属于 InvalidState。
                Errc::InvalidState,
                // 诊断明确代理缺失。
                format!("WaylandBackend::{operation}: Enter serial has no active pointer proxy"),
            )
        })?;
        // 在代理仍由 owner 槽持有时提交请求。
        apply(pointer)
    }
}

impl ICursor for WaylandBackend {
    // 更新标准 cursor shape；无 pointer focus 时保存为下次 Enter intent。
    fn set_cursor(&mut self, cursor: CursorType) -> Result<()> {
        // 生命周期 gate 必须先于能力发现或 intent 修改。
        self.ensure_cursor_open("set_cursor")?;
        // Custom 在读取 manager 前得到更精确的稳定 capability 诊断。
        let shape = protocol_shape(cursor)?;
        // optional global 缺失时不得把请求保存为已支持 intent。
        let manager = self.cursor_shape_manager("set_cursor")?;
        // 快照决定当前是否需要立即排队协议请求。
        let snapshot = self.cursor_state.snapshot();
        // 隐藏状态只更新下次显示/Enter 使用的形状，不得意外显现。
        if snapshot.visible {
            // 有焦点时成功必须已经提交 set_shape。
            if let Some(serial) = snapshot.enter_serial {
                // pointer 与 serial 必须属于同一活动代理代次。
                self.with_active_cursor_pointer("set_cursor", |pointer| {
                    // 提交标准 cursor-shape 请求。
                    submit_shape(manager, pointer, serial, shape);
                    // 请求已排队。
                    Ok(())
                })?;
            }
        }
        // 有焦点时请求已排队；无焦点或隐藏时记录待应用 intent。
        self.cursor_state.commit_cursor(cursor);
        // 返回成功表示 intent 已被 Component 接受且不会丢失。
        Ok(())
    }
    // 更新可见性，并在有焦点时立即排队隐藏或恢复请求。
    fn show_cursor(&mut self, visible: bool) -> Result<()> {
        // 生命周期 gate 必须先于能力发现或 intent 修改。
        self.ensure_cursor_open("show_cursor")?;
        // 隐藏与恢复必须作为对称能力出现，避免无 manager 时永久隐藏。
        let manager = self.cursor_shape_manager("show_cursor")?;
        // 当前形状与 Enter serial 由同一 Component 提供。
        let snapshot = self.cursor_state.snapshot();
        // 无焦点请求保留到下一次 Enter，不伪造当前 compositor 动作。
        if let Some(serial) = snapshot.enter_serial {
            // 当前 pointer owner 必须与 serial 同时存在。
            self.with_active_cursor_pointer("show_cursor", |pointer| {
                // 恢复可见时重放当前标准形状。
                if visible {
                    // 已提交 intent 不可能包含 Custom；仍检查不变量。
                    let shape = protocol_shape(snapshot.cursor)?;
                    // cursor-shape 会替换先前的空 surface 隐藏状态。
                    submit_shape(manager, pointer, serial, shape);
                } else {
                    // 空 cursor surface 由核心 wl_pointer 协议定义为隐藏。
                    pointer.set_cursor(serial, None, 0, 0);
                }
                // 请求已排队。
                Ok(())
            })?;
        }
        // 协议请求已排队或无焦点待应用后，才发布新可见性 intent。
        self.cursor_state.commit_visibility(visible);
        // Component 将在后续每次 Enter 重放该状态。
        Ok(())
    }
    fn cursor_position(&self) -> Result<Point> {
        Err(Error::new(
            Errc::NotImplemented,
            "WaylandBackend::cursor_position: not provided by wl_seat",
        ))
    }
    fn set_cursor_position(&mut self, _: i32, _: i32) -> Result<()> {
        Err(Error::new(
            Errc::NotImplemented,
            "WaylandBackend::set_cursor_position: compositor-controlled",
        ))
    }
    fn confine_cursor(&mut self, _: bool) -> Result<()> {
        Err(Error::new(
            Errc::NotImplemented,
            "WaylandBackend::confine_cursor: compositor-controlled",
        ))
    }
    fn capture_mouse(&mut self) -> Result<()> {
        Err(Error::new(
            Errc::NotImplemented,
            "WaylandBackend::capture_mouse: compositor-controlled",
        ))
    }
    fn release_mouse(&mut self) -> Result<()> {
        Err(Error::new(
            Errc::NotImplemented,
            "WaylandBackend::release_mouse: compositor-controlled",
        ))
    }
}
