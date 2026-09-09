// ============================================================================
// platform/linux/wayland/text_input.rs — ITextInput impl (zwp_text_input_v3)
//
// 通过 zwp_text_input_v3 协议实现 Linux 下的输入法（IME）支持。
// 当 IME 提交文本时，将 UiEvent::text_input 推入事件队列。
// ============================================================================

// queue-guard 窄端口需要显式事件队列类型。
use std::collections::VecDeque;
// owner 检查 helper 固定 composition→events 的 guard 类型与顺序。
use std::sync::atomic::Ordering;
use std::sync::{Mutex, MutexGuard};
use wayland_client::Proxy;
use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_v3;

use crate::core::{Errc, Error, Rect, Result, WindowId};
// IME 事务直接借用统一窗口事件队列。
use crate::platform::windowing::ITextInput;
use crate::platform::windowing::event::UiEvent;
// Wayland adapter 使用共享 Component 的无锁 queue-guard 入口。
use crate::native::windowing::shared::ime_events::{
    // composition 状态继续由共享值类型定义。
    ImeCompositionState,
    // Done callback 继续复用双缓冲 batch。
    PendingImeBatch,
    // Enter/Leave/stop 复用统一的窗口定向 unmark 事件序列。
    on_unmark_text_for_window_in_queue,
};

use super::WaylandBackend;

// enter 之前只保存期望状态；协议 surface 就绪后才发布 enable 与光标矩形。
#[derive(Clone, Copy, Default)]
pub(crate) struct TextInputProtocolState {
    entered: bool,
    cursor_rect: Option<Rect>,
}

fn send_cursor_rect(text_input: &super::compat::Main<super::ZwpTextInputV3>, rect: Rect) {
    text_input.set_cursor_rectangle(
        rect.x.round() as i32,
        rect.y.round() as i32,
        rect.w.max(0.0).round() as i32,
        rect.h.max(0.0).round() as i32,
    );
}

// 固定 Wayland IME composition 与事件队列的双 guard 事务类型。
type ImeStateGuards<'a> = (
    // 第一把 guard 唯一修改 composition 状态。
    MutexGuard<'a, ImeCompositionState>,
    // 第二把 guard 唯一提交窗口 IME 事件。
    MutexGuard<'a, VecDeque<UiEvent>>,
);

// 按固定 composition→events 顺序取得 IME 事务所需 owner。
fn lock_ime_state_checked<'a>(
    // composition owner 必须先验证健康。
    composition: &'a Mutex<ImeCompositionState>,
    // event queue owner 只能在第一把 guard 后验证。
    events: &'a Mutex<VecDeque<UiEvent>>,
    // 阶段文本区分 callback 与同步 stop。
    stage: &str,
    // Result 保留同步端口的 typed failure 通道。
) -> Result<ImeStateGuards<'a>> {
    // composition 损坏时不得查看或修改 IME 状态。
    let composition = composition.lock().map_err(|_| {
        // 状态损坏统一分类为 InvalidState。
        Error::new(
            // 使用稳定共享状态错误类别。
            Errc::InvalidState,
            // 诊断保留协议 adapter、阶段与 owner。
            format!("Wayland text_input {stage} composition mutex poisoned"),
        )
    })?;
    // event queue 损坏时保持已验证的 composition 不变。
    let events = match events.lock() {
        // 两把健康 guards 组成唯一提交事务。
        Ok(events) => events,
        // 第二个 owner 损坏时显式失败。
        Err(_) => {
            // 先释放 composition guard，避免错误转交时跨 owner 持锁。
            drop(composition);
            // 返回稳定的 typed failure。
            return Err(Error::new(
                // 使用稳定共享状态错误类别。
                Errc::InvalidState,
                // 诊断保留协议 adapter、阶段与 owner。
                format!("Wayland text_input {stage} event queue mutex poisoned"),
            ));
        }
    };
    // 调用方只能在成功分支中修改两份状态。
    Ok((composition, events))
}

// Wayland text-input Module 提供无协议请求的 backend teardown 端口。
impl WaylandBackend {
    // 在任何 text-input owner 或协议访问前检查 backend 生命周期。
    fn ensure_text_input_open(&self, operation: &str) -> Result<()> {
        // closed 事实由 WaylandBackend owner-thread 唯一提交。
        if self.closed {
            // 关闭后访问属于稳定生命周期错误，禁止复活 IME session。
            return Err(Error::new(
                // 使用 InvalidState 区分生命周期与 compositor 能力错误。
                Errc::InvalidState,
                // 保留具体 text-input 操作以便定位调用方。
                format!("Wayland text_input {operation} requested after backend shutdown"),
            ));
        }
        // 健康 backend 继续进入既有 text-input 行为。
        Ok(())
    }

    // 失效当前 IME session、callback 与全部本地 owner 状态。
    pub(crate) fn shutdown_text_input(&mut self) {
        // 首先推进 generation，使所有旧 callback 在共享状态访问前失效。
        self.text_input_generation.fetch_add(1, Ordering::SeqCst);
        // 随后发布 session 已禁用事实，阻止其他 owner 继续视为活跃。
        self.text_input_enabled.store(false, Ordering::SeqCst);
        self.text_input_protocol
            .set(TextInputProtocolState::default());
        // teardown 确定性取得 composition owner。
        let mut composition = self
            // 访问 backend 唯一 IME composition 状态。
            .text_input_composition
            // owner-thread 等待当前 callback 退出后取得 guard。
            .lock()
            // 关闭时恢复中毒 guard 只用于清除失效状态。
            .unwrap_or_else(|error| error.into_inner());
        // 清除未完成 composition，不发布任何 unmark UI 事件。
        composition.active = false;
        // 先释放 composition guard，避免跨越 proxy callback registry。
        drop(composition);
        // 清除活跃 session 的窗口身份。
        self.active_text_input_window_id = None;
        // 清除尚未启动 session 的目标窗口身份。
        self.text_input_window_id = None;
        // 最后从唯一 owner 槽取走协议 proxy。
        if let Some(text_input) = self.text_input.take() {
            // fatal/Drop teardown 只注销本地 callback，不在失效连接上发送请求。
            text_input.clear_callback();
            // proxy 随分支结束释放本地 owner。
        }
        // teardown 不关闭 pending source，根因仍由平台 owner-thread 提取。
    }
}

// ════════════════════════════════════════════════════════════════════════════
// ITextInput for WaylandBackend
// ════════════════════════════════════════════════════════════════════════════

impl ITextInput for WaylandBackend {
    fn set_target_window(
        &mut self,
        window_id: WindowId,
        _native_window: *mut std::ffi::c_void,
    ) -> Result<()> {
        // 生命周期 gate 必须先于目标窗口 owner 写入。
        self.ensure_text_input_open("set_target_window")?;
        self.text_input_window_id = Some(window_id);
        Ok(())
    }

    fn start(&mut self) -> Result<()> {
        // 生命周期 gate 必须先于目标、seat、manager 或 generation 访问。
        self.ensure_text_input_open("start")?;
        let window_id = self.text_input_window_id.ok_or_else(|| {
            Error::new(
                Errc::InvalidOperation,
                "Wayland text_input: no target window selected",
            )
        })?;
        if self.text_input.is_some() || self.active_text_input_window_id.is_some() {
            self.stop()?;
        }
        let seat = match self.seat.as_ref() {
            Some(s) => s,
            None => {
                return Err(Error::new(
                    Errc::InvalidOperation,
                    "Wayland text_input: no seat available",
                ));
            }
        };

        let manager = match self.text_input_manager.as_ref() {
            Some(m) => m,
            None => {
                return Err(Error::new(
                    Errc::NotImplemented,
                    "Wayland text_input: compositor has no zwp_text_input_manager_v3",
                ));
            }
        };

        // 如果已有活跃的 text_input，先销毁
        self.text_input = None;

        let ti = manager.get_text_input(seat);
        let events = self.events.clone();
        let surface_windows = self.surface_windows.clone();
        let composition = self.text_input_composition.clone();
        // callback failure 复用 backend 已有 pending source。
        let pending_failures = self.pending_failures.clone();
        let active_generation = self.text_input_generation.clone();
        let generation = active_generation
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_add(1);
        self.text_input_protocol
            .set(TextInputProtocolState::default());
        let protocol = self.text_input_protocol.clone();
        let mut pending = PendingImeBatch::default();

        ti.quick_assign(move |text_input, event, _| {
            if active_generation.load(Ordering::SeqCst) != generation {
                return;
            }
            match event {
                zwp_text_input_v3::Event::Enter { surface } => {
                    // surface registry 损坏时不得更新 callback focus。
                    let owns_surface = match surface_windows.lock() {
                        // 健康 registry 才能解析协议 surface 的窗口 owner。
                        Ok(targets) => {
                            // 只接受本 text-input session 的目标窗口。
                            targets.window_for_surface(surface.id().protocol_id())
                                == Some(window_id)
                        }
                        // 锁中毒必须转交 owner-thread。
                        Err(_) => {
                            // callback 只构造并入队 typed failure。
                            let _ = pending_failures.enqueue(Error::new(
                                // 状态损坏统一分类为 InvalidState。
                                Errc::InvalidState,
                                // 诊断区分 Enter 的 surface lookup owner。
                                "Wayland text_input enter callback surface registry mutex poisoned",
                            ));
                            // 不修改 focused、pending 或任何共享状态。
                            return;
                        }
                    };
                    // 已聚焦 session 进入非目标 surface 时先事务化 unmark。
                    if protocol.get().entered && !owns_surface {
                        // 两个 owner 必须在任何 composition/event 修改前健康。
                        let (mut state, mut event_queue) = match lock_ime_state_checked(
                            // 第一把 guard 是 composition owner。
                            &composition,
                            // 第二把 guard 是事件队列 owner。
                            &events,
                            // 阶段诊断区分 Enter 的隐式 unmark。
                            "enter-unmark callback",
                        ) {
                            // 健康 guards 进入共享无锁窄端口。
                            Ok(guards) => guards,
                            // callback failure 只入队，不执行错误策略。
                            Err(error) => {
                                // 同一 backend source 承接 typed failure。
                                let _ = pending_failures.enqueue(error);
                                // 保持 focused、pending、composition 与 events 不变。
                                return;
                            }
                        };
                        // 使用同一健康事件 guard 提交 unmark 事务。
                        on_unmark_text_for_window_in_queue(
                            // 传入已验证健康的事件队列。
                            &mut event_queue,
                            // 传入同一事务的 composition 状态。
                            &mut state,
                            // 事件继续路由到 session 目标窗口。
                            window_id,
                        );
                    }
                    // owner 事务成功后再提交 callback focus。
                    let mut state = protocol.get();
                    state.entered = owns_surface;
                    protocol.set(state);
                    // 新 Enter 边界丢弃上一批未 Done 的输入。
                    pending = PendingImeBatch::default();
                    if owns_surface {
                        // enter/enable 会使旧状态失效；在同一提交中重发最新光标。
                        text_input.enable();
                        if let Some(rect) = state.cursor_rect {
                            send_cursor_rect(text_input, rect);
                        }
                        text_input.commit();
                    }
                }
                zwp_text_input_v3::Event::Leave { .. } => {
                    // 只有当前聚焦 session 需要投递 unmark。
                    if protocol.get().entered {
                        // 两个 owner 必须在任何 composition/event 修改前健康。
                        let (mut state, mut event_queue) = match lock_ime_state_checked(
                            // 第一把 guard 是 composition owner。
                            &composition,
                            // 第二把 guard 是事件队列 owner。
                            &events,
                            // 阶段诊断区分 Leave callback。
                            "leave callback",
                        ) {
                            // 健康 guards 进入共享无锁窄端口。
                            Ok(guards) => guards,
                            // callback failure 只入队，不执行错误策略。
                            Err(error) => {
                                // 同一 backend source 承接 typed failure。
                                let _ = pending_failures.enqueue(error);
                                // 保持 focused、pending、composition 与 events 不变。
                                return;
                            }
                        };
                        // 使用同一健康事件 guard 提交 unmark 事务。
                        on_unmark_text_for_window_in_queue(
                            // 传入已验证健康的事件队列。
                            &mut event_queue,
                            // 传入同一事务的 composition 状态。
                            &mut state,
                            // 事件继续路由到 session 目标窗口。
                            window_id,
                        );
                    }
                    // owner 事务成功后再提交失焦事实。
                    let mut state = protocol.get();
                    state.entered = false;
                    protocol.set(state);
                    // Leave 边界丢弃未 Done 的输入。
                    pending = PendingImeBatch::default();
                }
                zwp_text_input_v3::Event::PreeditString { text, .. } if protocol.get().entered => {
                    pending.set_preedit(text);
                }
                zwp_text_input_v3::Event::CommitString { text } if protocol.get().entered => {
                    pending.set_commit(text);
                }
                zwp_text_input_v3::Event::Done { .. } if protocol.get().entered => {
                    // 两个 owner 必须在 take pending batch 前健康。
                    let (mut state, mut event_queue) = match lock_ime_state_checked(
                        // 第一把 guard 是 composition owner。
                        &composition,
                        // 第二把 guard 是事件队列 owner。
                        &events,
                        // 阶段诊断区分 Done batch apply。
                        "done callback",
                    ) {
                        // 健康 guards 进入共享无锁窄端口。
                        Ok(guards) => guards,
                        // callback failure 只入队，不执行错误策略。
                        Err(error) => {
                            // 同一 backend source 承接 typed failure。
                            let _ = pending_failures.enqueue(error);
                            // pending batch 与共享状态均保持不变。
                            return;
                        }
                    };
                    // Done 在同一双 guard 事务中应用并清空 batch。
                    pending.apply_for_window_in_queue(
                        // 传入已验证健康的事件队列。
                        &mut event_queue,
                        // 传入同一事务的 composition 状态。
                        &mut state,
                        // 事件继续路由到 session 目标窗口。
                        window_id,
                    );
                }
                _ => {}
            }
        });

        self.text_input = Some(ti);
        self.active_text_input_window_id = Some(window_id);
        self.text_input_enabled.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        // 生命周期 gate 必须先于 composition guards 与 generation 推进。
        self.ensure_text_input_open("stop")?;
        // 复制 active window，检查失败前不 take session owner。
        let active_window_id = self.active_text_input_window_id;
        // Arc 克隆让 guards 不借用 self，便于后续提交其他 session 字段。
        let stop_composition = self.text_input_composition.clone();
        // 事件队列 owner 使用相同的本地 Arc 生命周期。
        let stop_events = self.events.clone();
        // 活跃 session 必须先验证 composition 与事件队列健康。
        let stop_guards = match active_window_id {
            // 有 active window 时建立同步清理事务。
            Some(_) => Some(lock_ime_state_checked(
                // 第一把 guard 是 composition owner。
                &stop_composition,
                // 第二把 guard 是事件队列 owner。
                &stop_events,
                // 阶段诊断区分同步 stop。
                "stop",
            )?),
            // 没有 active window 时无需访问 IME 状态 owner。
            None => None,
        };
        // owner 全部健康后才失效旧 callback generation。
        self.text_input_generation.fetch_add(1, Ordering::SeqCst);
        // 活跃协议对象保持既有 disable→commit 顺序。
        if let Some(ref ti) = self.text_input {
            // 请求 compositor 停止 text-input session。
            ti.disable();
            // 提交协议状态变更。
            ti.commit();
        }
        // 活跃 session 在已持有的双 guard 事务中 unmark。
        if let (Some(window_id), Some((mut state, mut event_queue))) =
            (active_window_id, stop_guards)
        {
            // 复用共享 IME Component 的无锁 queue-guard 入口。
            on_unmark_text_for_window_in_queue(
                // 传入已验证健康的事件队列。
                &mut event_queue,
                // 传入同一事务的 composition 状态。
                &mut state,
                // 事件继续路由到原 active window。
                window_id,
            );
            // unmark 成功后才释放 active window owner。
            self.active_text_input_window_id = None;
        }
        // 协议 disable 与 IME 状态提交成功后取走代理 owner。
        if let Some(text_input) = self.text_input.take() {
            // 显式注销 callback；compat::Main 的 Drop 不自动清理 registry。
            text_input.clear_callback();
            text_input.destroy();
            // proxy 随分支结束释放本地 owner。
        }
        // 最后发布 session 已禁用事实。
        self.text_input_enabled.store(false, Ordering::SeqCst);
        self.text_input_protocol
            .set(TextInputProtocolState::default());
        // 同步 stop 全部成功。
        Ok(())
    }

    fn set_cursor_rect(&mut self, rect: Rect) -> Result<()> {
        // 生命周期 gate 必须先于 proxy owner 读取与协议请求。
        self.ensure_text_input_open("set_cursor_rect")?;
        let Some(text_input) = self.text_input.as_ref() else {
            return Err(Error::new(
                Errc::InvalidOperation,
                "Wayland text_input: no active IME session",
            ));
        };
        let mut state = self.text_input_protocol.get();
        state.cursor_rect = Some(rect);
        self.text_input_protocol.set(state);
        if state.entered {
            send_cursor_rect(text_input, rect);
            text_input.commit();
        }
        Ok(())
    }
}
