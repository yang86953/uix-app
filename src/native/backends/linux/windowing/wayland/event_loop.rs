// ============================================================================
// platform/linux/wayland/event_loop.rs — Wayland 事件分发
//
// WaylandBackend 的事件分发方法：
//   try_dispatch()      — 非阻塞分发
//   dispatch_blocking() — 阻塞等待分发
//   dispatch_timeout()  — 带超时分发
//
// 注意：不再直接实现 IEventLoop，由 LinuxPlatform 通过 OsEventSource 获得。
// ============================================================================

use std::os::fd::{AsFd, AsRawFd, RawFd};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::{Duration, Instant};

use libc::{POLLERR, POLLHUP, POLLIN, POLLNVAL, POLLOUT, poll, pollfd};
use wayland_client::backend::WaylandError;

use crate::core::{Errc, Error};
use crate::platform::windowing::KeyMod;
use crate::platform::windowing::event::UiEvent;

use super::WaylandBackend;
use super::keycode::keycode_to_char;
use crate::native::windowing::shared::nonblocking_read::NonBlockingReadStatus;
use crate::native::windowing::shared::nonblocking_write::NonBlockingWriteStatus;

impl WaylandBackend {
    /// 非阻塞事件分发。
    /// 先 dispatch 已在内部队列中的事件，再用 poll() 检查 socket。
    pub(crate) fn try_dispatch(&mut self) -> bool {
        if self.closed {
            return false;
        }
        if !self.dispatch_pending_checked("dispatch_pending") {
            return false;
        }
        self.dispatch_polled(0, "dispatch")
    }

    /// 阻塞等待 Wayland 事件。
    ///
    /// 当有按键按住且客户端侧按键重复已启用时，使用 poll 超时循环
    /// 确保 generate_key_repeats() 被周期性调用。否则使用传统阻塞 dispatch。
    pub(crate) fn dispatch_blocking(&mut self) -> bool {
        if self.closed {
            return false;
        }

        // 先在独立短锁中读取是否存在客户端重复按键。
        let has_held = match self.held_key_info.lock() {
            // 健康 owner 只复制布尔事实，guard 随本分支结束释放。
            Ok(held_key) => held_key.is_some(),
            // 损坏状态不得继续影响阻塞调度决策。
            Err(_) => {
                // failure 进入 backend 已有 source，留待 App owner-thread 处理。
                self.enqueue_failure(Error::new(
                    // 客户端重复 owner 已无法安全读取。
                    Errc::InvalidState,
                    // 保留 held-key 与阻塞调度阶段。
                    "Wayland dispatch_blocking held-key mutex poisoned",
                ));
                // 终止本次 dispatch，不进入 poll 或重复生成。
                return false;
            }
        };
        // 第一把 guard 已释放后再读取独立的重复速率状态。
        let repeat_rate = match self.repeat_rate.lock() {
            // 健康 owner 只复制协议速率值。
            Ok(rate) => *rate,
            // 损坏速率不得被恢复为调度参数。
            Err(_) => {
                // failure 进入同一 backend pending source。
                self.enqueue_failure(Error::new(
                    // 重复速率 owner 已无法安全读取。
                    Errc::InvalidState,
                    // 保留 repeat-rate 与阻塞调度阶段。
                    "Wayland dispatch_blocking repeat-rate mutex poisoned",
                ));
                // 终止本次 dispatch，不进入 poll 或重复生成。
                return false;
            }
        };

        // 只有实际按键与正重复速率同时成立才使用超时 dispatch。
        if has_held && repeat_rate > 0 {
            // 按键按住 + 客户端重复启用：使用 poll 循环，
            // 超时时间基于重复速率，确保 generate_key_repeats() 按时触发
            let interval_ms = (1000 / repeat_rate).max(10).min(100) as i32;
            return self.dispatch_timeout(Duration::from_millis(interval_ms as u64));
        }

        // 无按键按住：传统阻塞 dispatch
        self.dispatch_polled(-1, "dispatch_blocking")
    }

    /// 带超时的 Wayland 事件分发。
    pub(crate) fn dispatch_timeout(&mut self, timeout: Duration) -> bool {
        if self.closed {
            return false;
        }
        let timeout_ms = timeout.as_millis().min(i32::MAX as u128) as i32;
        self.dispatch_polled(timeout_ms, "dispatch_timeout")
    }

    /// 从事件队列弹出下一个事件。
    pub(crate) fn next_event(&self) -> Option<UiEvent> {
        // 无 Result 通道时先检查事件队列 owner，再决定是否访问队列。
        let Ok(mut events) = self.events.lock() else {
            // typed failure 进入 backend 已有 source，留待 App owner-thread 处理。
            self.enqueue_failure(Error::new(
                // 损坏的事件队列属于稳定共享状态错误。
                Errc::InvalidState,
                // 保留 Wayland next_event 与事件队列阶段。
                "Wayland next_event queue mutex poisoned",
            ));
            // 不把损坏状态恢复成伪事件或继续访问队列。
            return None;
        };
        // 健康队列仍只弹出最早的一项事件。
        events.pop_front()
    }

    // ── 内部辅助 ──────────────────────────────────────────

    fn dispatch_polled(&mut self, timeout_ms: i32, context: &str) -> bool {
        if !self.flush_checked(context) {
            return false;
        }
        let wayland_fd = self.display.as_fd().as_raw_fd();
        // 先检查 clipboard read FD owner，再构造 poll 快照。
        let clipboard_fd = match self.clipboard_read.lock() {
            // 健康 owner 只复制当前可选 FD，guard 随分支结束释放。
            Ok(active_read) => active_read
                // 只查看当前活动 read owner。
                .as_ref()
                // poll 快照不接管 FD 生命周期。
                .map(super::clipboard::ClipboardRead::fd),
            // 损坏 read owner 的 FD 不得进入系统 poll。
            Err(_) => {
                // failure 进入 backend 已有 source。
                self.enqueue_failure(Error::new(
                    // 非阻塞 read owner 已无法安全读取。
                    Errc::InvalidState,
                    // 保留 poll 快照的 clipboard-read 阶段。
                    "Wayland dispatch_polled clipboard-read mutex poisoned",
                ));
                // 终止本次 dispatch，不调用系统 poll。
                return false;
            }
        };
        // 文件拖放读队列损坏时不得构造部分 poll 快照。
        let Some(file_drop_fds) = super::file_drop::read_fds(
            // 读取 backend 唯一的拖放 Component。
            &self.file_drop_state,
            // 快照失败进入既有 pending source。
            &self.pending_failures,
        ) else {
            // typed failure 已入队，终止本轮 dispatch。
            return false;
        };
        self.poll_fds.clear();
        self.poll_fds.push(pollfd {
            fd: wayland_fd,
            events: POLLIN,
            revents: 0,
        });
        self.poll_fds.push(pollfd {
            fd: self.wake_read_fd,
            events: POLLIN,
            revents: 0,
        });
        let clipboard_read_index = clipboard_fd.map(|fd| {
            let index = self.poll_fds.len();
            self.poll_fds.push(pollfd {
                fd,
                events: POLLIN,
                revents: 0,
            });
            index
        });
        // 记录文件拖放 FD 在 poll 数组中的连续区间。
        let file_drop_read_start = self.poll_fds.len();
        // 每个已 Drop transfer 作为独立可读 FD。
        self.poll_fds.extend(file_drop_fds.iter().map(|fd| pollfd {
            // 快照只复制 FD identity。
            fd: *fd,
            // 数据或 EOF 均由可读/HUP completion 处理。
            events: POLLIN,
            // poll 将同步填写结果位。
            revents: 0,
        }));
        // 区间终点同时是 clipboard write 起点。
        let file_drop_read_end = self.poll_fds.len();
        let clipboard_write_start = self.poll_fds.len();
        // 独立检查全部 clipboard write FD owners。
        {
            // 损坏 write queue 中的 FD 不得进入系统 poll。
            let writes = match self.clipboard_writes.lock() {
                // 健康 guard 只在构造本次 poll 快照期间存活。
                Ok(writes) => writes,
                // owner 损坏时停止本次调度。
                Err(_) => {
                    // failure 进入同一 backend pending source。
                    self.enqueue_failure(Error::new(
                        // 非阻塞 write owners 已无法安全读取。
                        Errc::InvalidState,
                        // 保留 poll 快照的 clipboard-write 阶段。
                        "Wayland dispatch_polled clipboard-write mutex poisoned",
                    ));
                    // 终止本次 dispatch，不调用系统 poll。
                    return false;
                }
            };
            // 健康队列按既有顺序把所有 write FD 附加到快照。
            self.poll_fds.extend(writes.iter().map(|write| pollfd {
                fd: write.fd(),
                events: POLLOUT,
                revents: 0,
            }));
        }
        let poll_len = self.poll_fds.len();
        // SAFETY：poll_fds 由本对象独占（&mut self），指针指向存活 Vec 缓冲且
        // nfds 等于其实际长度；列表内 fd 均为当前有效描述符（display、wake pipe、
        // clipboard 读写端），poll 只同步写入数组内的 revents 字段。
        let ret = unsafe {
            poll(
                self.poll_fds.as_mut_ptr(),
                poll_len as libc::nfds_t,
                timeout_ms,
            )
        };
        if ret < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                return true;
            }
            return self.close_after_failure(
                Errc::IoError,
                format!("Wayland {context} poll error: {error}"),
            );
        }

        let wayland_revents = self.poll_fds[0].revents;
        let wake_revents = self.poll_fds[1].revents;
        if (wake_revents & POLLIN) != 0 {
            self.drain_wake_pipe();
        }
        if (wayland_revents & (POLLERR | POLLHUP | POLLNVAL)) != 0 {
            return self.close_after_failure(
                Errc::IoError,
                format!("Wayland {context} display fd reported an error"),
            );
        }
        if (wayland_revents & POLLIN) != 0 {
            if let Some(read_guard) = self.display.prepare_read() {
                if let Err(e) = read_guard.read() {
                    return self.close_after_failure(
                        Errc::IoError,
                        format!("Wayland {context} read error: {e}"),
                    );
                }
            } else {
                // system backend：libwayland 内部队列已有未转发事件（其他路径已读入），
                // 必须先转发到客户端 EventQueue，否则事件永远卡在 C 库内部队列
                // （表现为输入无响应、装饰协商不生效）。
                if let Err(e) = self.display.backend().dispatch_inner_queue() {
                    return self.close_after_failure(
                        Errc::PlatformError,
                        format!("Wayland {context} inner queue dispatch error: {e}"),
                    );
                }
            }
            if !self.dispatch_pending_checked(context) {
                return false;
            }
        }
        if let Some(polled_fd) = clipboard_fd {
            let clipboard_revents = clipboard_read_index
                .map(|index| self.poll_fds[index].revents)
                .unwrap_or(0);
            if (clipboard_revents & (POLLIN | POLLHUP)) != 0 {
                // completion helper 失败时终止本次 owner-thread 调度。
                if !self.read_clipboard_pipe(polled_fd) {
                    // 状态失败已经进入 backend pending source。
                    return false;
                }
            } else if (clipboard_revents & (POLLERR | POLLNVAL)) != 0 {
                // poll error 也必须先通过 checked owner 端口移除 read。
                if !self.discard_clipboard_read_after_poll_error(polled_fd) {
                    // 损坏 owner 不得继续进入 write completion。
                    return false;
                }
            }
        }
        // 按 poll 快照顺序推进全部文件拖放 URI transfer。
        for index in file_drop_read_start..file_drop_read_end {
            // completion 必须使用同一槽位的 FD 与结果位。
            if !super::file_drop::complete_polled_read(
                // 传入稳定 FD identity。
                self.poll_fds[index].fd,
                // 传入本轮 readiness/error 位。
                self.poll_fds[index].revents,
                // 访问唯一拖放 Component。
                &self.file_drop_state,
                // 完成事件进入 backend 队列。
                &self.events,
                // 共享 typed failure source。
                &self.pending_failures,
            ) {
                // owner 状态失败时立即终止本轮 dispatch。
                return false;
            }
        }
        let mut write_budget = super::clipboard::CLIPBOARD_WRITE_BUDGET;
        for index in clipboard_write_start..poll_len {
            if write_budget == 0 {
                break;
            }
            let polled_fd = self.poll_fds[index].fd;
            let revents = self.poll_fds[index].revents;
            let slots_left = poll_len - index;
            let budget = (write_budget / slots_left).max(1);
            // write helper 用 None 显式传播 owner 状态失败。
            let Some(written) = self.write_clipboard_pipe(polled_fd, revents, budget) else {
                // 失败项不消耗本次 write budget。
                return false;
            };
            write_budget = write_budget.saturating_sub(written);
        }
        // 按键重复生成的 owner 状态失败必须终止本轮 dispatch。
        if !self.generate_key_repeats() {
            // failure 已进入 backend pending source。
            return false;
        }
        // poll 与所有 completion 均成功。
        true
    }

    // 根据健康 owner 快照编排一次客户端按键重复。
    fn generate_key_repeats(&mut self) -> bool {
        // held-key owner 损坏时不得读取重复输入事实。
        let held = match self.held_key_info.lock() {
            // HeldKeyInfo 可复制，guard 随分支结束释放。
            Ok(held) => *held,
            // 锁中毒必须显式失败。
            Err(_) => {
                // failure 只进入既有 backend source。
                self.enqueue_failure(Error::new(
                    // 状态损坏统一分类为 InvalidState。
                    Errc::InvalidState,
                    // 诊断保留 held-key 与生成阶段。
                    "Wayland key repeat held-key mutex poisoned during generation",
                ));
                // 调用方必须停止本轮 dispatch。
                return false;
            }
        };
        // 没有按住的键时保持健康无事件语义。
        let Some(held) = held else {
            // 无重复候选不属于失败。
            return true;
        };
        // 复制重复事件所需的稳定按键值。
        let code = held.code;
        // 复制按下时的修饰键事实。
        let mods = held.mods;
        // 复制首次按下时刻用于 delay 计算。
        let first_press = held.first_press;
        // 复制事件路由窗口。
        let window_id = held.window_id;
        // surface 路由损坏时不得基于未知 target 生成事件。
        let current_target = match self.surface_windows.lock() {
            // 健康 guard 只复制当前 keyboard target。
            Ok(targets) => targets.keyboard_target(),
            // 锁中毒必须显式失败。
            Err(_) => {
                // failure 只进入既有 backend source。
                self.enqueue_failure(Error::new(
                    // 状态损坏统一分类为 InvalidState。
                    Errc::InvalidState,
                    // 诊断保留 surface target 与生成阶段。
                    "Wayland key repeat surface target mutex poisoned during generation",
                ));
                // 调用方必须停止本轮 dispatch。
                return false;
            }
        };
        // 重复候选已不属于当前键盘焦点时事务化清理。
        if current_target != Some(window_id) {
            // 清理 helper 负责同时提交两份 owner 状态。
            return self.clear_stale_key_repeat_state();
        }

        // repeat-rate owner 损坏时不得计算调度 interval。
        let rate = match self.repeat_rate.lock() {
            // 健康 guard 只复制 compositor rate。
            Ok(rate) => *rate,
            // 锁中毒必须显式失败。
            Err(_) => {
                // failure 只进入既有 backend source。
                self.enqueue_failure(Error::new(
                    // 状态损坏统一分类为 InvalidState。
                    Errc::InvalidState,
                    // 诊断保留 repeat-rate 与生成阶段。
                    "Wayland key repeat rate mutex poisoned during generation",
                ));
                // 调用方必须停止本轮 dispatch。
                return false;
            }
        };
        // compositor 禁用重复时保持候选但不生成事件。
        if rate <= 0 {
            // 健康禁用状态不属于失败。
            return true;
        }
        // repeat-delay owner 损坏时不得计算首个触发时刻。
        let delay_ms = match self.repeat_delay.lock() {
            // 健康 guard 只复制 compositor delay。
            Ok(delay) => *delay,
            // 锁中毒必须显式失败。
            Err(_) => {
                // failure 只进入既有 backend source。
                self.enqueue_failure(Error::new(
                    // 状态损坏统一分类为 InvalidState。
                    Errc::InvalidState,
                    // 诊断保留 repeat-delay 与生成阶段。
                    "Wayland key repeat delay mutex poisoned during generation",
                ));
                // 调用方必须停止本轮 dispatch。
                return false;
            }
        };

        // owner-thread 单调时钟决定本轮触发判断。
        let now = Instant::now();
        // 计算候选键已经保持的时间。
        let elapsed = now.duration_since(first_press);
        // delay 至少为 1ms，保留既有防御语义。
        let delay = Duration::from_millis(delay_ms.max(1) as u64);
        // 首次 delay 尚未到期时不访问提交 owners。
        if elapsed < delay {
            // 健康等待状态不属于失败。
            return true;
        }

        // 依据 compositor rate 计算重复间隔。
        let interval = Duration::from_secs_f32(1.0 / rate as f32);
        // 继续保留 10ms 的最小节拍限制。
        let min_interval = Duration::from_millis(10);
        // 选择协议间隔与最小限制中的较大值。
        let interval = interval.max(min_interval);
        // 到期判断与事件提交由同一 guard 事务完成。
        self.commit_key_repeat_if_due(now, first_press, delay, interval, code, mods, window_id)
    }

    // 同时清理失去焦点的 held-key 与重复节拍状态。
    fn clear_stale_key_repeat_state(&mut self) -> bool {
        // 先取得 held-key guard，保持固定锁顺序。
        let mut held_key = match self.held_key_info.lock() {
            // 健康 guard 暂不修改，等待第二个 owner。
            Ok(held_key) => held_key,
            // 锁中毒必须显式失败。
            Err(_) => {
                // failure 只进入既有 backend source。
                self.enqueue_failure(Error::new(
                    // 状态损坏统一分类为 InvalidState。
                    Errc::InvalidState,
                    // 诊断保留焦点失配清理阶段。
                    "Wayland key repeat held-key mutex poisoned during stale-state cleanup",
                ));
                // 不产生任何半清理。
                return false;
            }
        };
        // 再取得 last-time guard，两个 owner 健康后才允许提交。
        let mut last_repeat = match self.last_repeat_time.lock() {
            // 健康 guard 与 held-key guard 组成清理事务。
            Ok(last_repeat) => last_repeat,
            // 第二个 owner 损坏时保持第一份状态不变。
            Err(_) => {
                // 先释放第一把 guard，避免跨 owner 入队。
                drop(held_key);
                // failure 只进入既有 backend source。
                self.enqueue_failure(Error::new(
                    // 状态损坏统一分类为 InvalidState。
                    Errc::InvalidState,
                    // 诊断保留 last-time 与焦点失配清理阶段。
                    "Wayland key repeat last-time mutex poisoned during stale-state cleanup",
                ));
                // 不产生 held-key 半清理。
                return false;
            }
        };
        // 两个 owner 均健康后清除重复候选。
        *held_key = None;
        // 在同一事务中清除历史节拍。
        *last_repeat = None;
        // 焦点失配清理成功。
        true
    }

    // 在重复到期时事务化提交节拍和窗口事件。
    #[allow(clippy::too_many_arguments)]
    // 参数均为 generate_key_repeats 的不可变健康快照。
    fn commit_key_repeat_if_due(
        // event loop owner 负责提交事务。
        &mut self,
        // 本轮 owner-thread 单调时刻。
        now: Instant,
        // 物理按键首次按下时刻。
        first_press: Instant,
        // compositor 首次重复延迟。
        delay: Duration,
        // compositor 重复间隔与 10ms 限制的结果。
        interval: Duration,
        // 统一框架键码。
        code: crate::platform::windowing::KeyCode,
        // 按下时的修饰键快照。
        mods: KeyMod,
        // 重复事件目标窗口。
        window_id: crate::core::WindowId,
        // 布尔结果显式传播 owner 状态失败。
    ) -> bool {
        // 先取得 event queue guard，对齐 keyboard 全局锁顺序。
        let mut events = match self.events.lock() {
            // 健康 guard 保持到到期判断与最终提交完成。
            Ok(events) => events,
            // 队列锁中毒必须显式失败。
            Err(_) => {
                // failure 只进入既有 backend source。
                self.enqueue_failure(Error::new(
                    // 状态损坏统一分类为 InvalidState。
                    Errc::InvalidState,
                    // 诊断保留 event queue 与重复提交阶段。
                    "Wayland key repeat event queue mutex poisoned during event commit",
                ));
                // 不访问 last-time，也不写入事件。
                return false;
            }
        };
        // 再取得 last-time guard，与焦点/释放事务保持 events→last 顺序。
        let mut last_repeat = match self.last_repeat_time.lock() {
            // 健康 guard 与 event queue guard 组成提交事务。
            Ok(last_repeat) => last_repeat,
            // 节拍损坏时保持事件队列不变。
            Err(_) => {
                // 先释放 event queue guard，避免跨 owner 入队。
                drop(events);
                // failure 只进入既有 backend source。
                self.enqueue_failure(Error::new(
                    // 状态损坏统一分类为 InvalidState。
                    Errc::InvalidState,
                    // 诊断保留 last-time 与事件提交阶段。
                    "Wayland key repeat last-time mutex poisoned during event commit",
                ));
                // 不推进时间，也不写入事件。
                return false;
            }
        };
        // 在两把健康 guards 内重新计算本轮是否到期。
        let should_fire = match *last_repeat {
            // 后续重复依据上次成功提交时刻。
            Some(last) => now.duration_since(last) >= interval,
            // 首次重复依据物理按下时刻与 delay。
            None => now >= first_press + delay,
        };
        // 尚未到期时保持两份 owner 状态不变。
        if !should_fire {
            // 健康等待状态不属于失败。
            return true;
        }
        // 两个 owner 均健康后提交本次重复时刻。
        *last_repeat = Some(now);
        // 根据按下时快照决定文本转换的 Shift 状态。
        let shift_down = mods.intersects(KeyMod::SHIFT);
        // 先提交与物理按下同形的 key-down 事件。
        events.push_back(UiEvent::key_down(code, mods).for_window(window_id));
        // 可打印键继续生成紧随其后的文本事件。
        if let Some(text) = keycode_to_char(code, shift_down) {
            // 文本事件保持同一窗口路由与既有顺序。
            events.push_back(UiEvent::text_input(text).for_window(window_id));
        }
        // 时间戳与全部事件已在同一双 guard 事务中提交。
        true
    }

    // 检查式移除 poll 已报告错误的 clipboard read owner。
    fn discard_clipboard_read_after_poll_error(&mut self, polled_fd: RawFd) -> bool {
        // 先在独立作用域中提交 owner 移除事实。
        let removed = {
            // read owner 损坏时不查看或移除其中的 FD。
            let mut active = match self.clipboard_read.lock() {
                // 健康 guard 才能检查本轮 poll 的 FD 身份。
                Ok(active) => active,
                // 锁中毒是稳定的 owner 状态失败。
                Err(_) => {
                    // failure 只进入既有 backend source。
                    self.enqueue_failure(Error::new(
                        // 状态损坏统一分类为 InvalidState。
                        Errc::InvalidState,
                        // 诊断保留 read owner 与 poll completion 阶段。
                        "Wayland clipboard read owner mutex poisoned during poll completion",
                    ));
                    // 调用方必须立即终止 dispatch。
                    return false;
                }
            };
            // 只有仍由当前 owner 持有的同一 FD 才能被移除。
            let matches_polled_fd = active
                // 检查可选 read owner。
                .as_ref()
                // 比较本轮 poll 快照中的稳定 FD identity。
                .is_some_and(|read| read.fd() == polled_fd);
            // 匹配时提交 read owner 释放事实。
            if matches_polled_fd {
                // 释放由 poll 明确报告无效的 read owner。
                *active = None;
            }
            // 将是否移除传出锁作用域。
            matches_polled_fd
        };
        // 仅为实际移除的 owner 投递 I/O failure。
        if removed {
            // failure 在 owner guard 释放后进入 pending source。
            self.enqueue_failure(Error::new(
                // FD 异常保持既有 I/O 分类。
                Errc::IoError,
                // 保留既有稳定诊断文本。
                "Wayland clipboard read fd reported an error",
            ));
        }
        // 健康 owner 路径允许继续处理其余 completion。
        true
    }

    // 事务化处理 clipboard read completion。
    fn read_clipboard_pipe(&mut self, polled_fd: RawFd) -> bool {
        // 把 I/O error 带出锁作用域后再投递 failure。
        let outcome = {
            // read owner 损坏时不得读取 FD。
            let mut active = match self.clipboard_read.lock() {
                // 健康 guard 才允许检查 active read。
                Ok(active) => active,
                // 锁中毒必须显式失败。
                Err(_) => {
                    // failure 只进入既有 backend source。
                    self.enqueue_failure(Error::new(
                        // 状态损坏统一分类为 InvalidState。
                        Errc::InvalidState,
                        // 诊断保留 read owner 与 completion 阶段。
                        "Wayland clipboard read owner mutex poisoned during poll completion",
                    ));
                    // 调用方必须停止本轮 dispatch。
                    return false;
                }
            };
            // 已切换或已完成的 read 不消费旧 poll 事件。
            if !active
                // 检查当前可选 owner。
                .as_ref()
                // 仅接受本轮 poll 的同一 FD。
                .is_some_and(|read| read.fd() == polled_fd)
            {
                // 健康的陈旧事件不属于失败。
                return true;
            }
            // 文本 owner 必须在执行非阻塞 read 前确认健康。
            let mut clipboard_text = match self.clipboard_text.lock() {
                // 健康文本 guard 与 read guard 组成提交事务。
                Ok(clipboard_text) => clipboard_text,
                // 文本状态损坏时保持 read owner 与 FD 不变。
                Err(_) => {
                    // 先释放健康 read guard，避免跨 owner 入队。
                    drop(active);
                    // failure 只进入既有 backend source。
                    self.enqueue_failure(Error::new(
                        // 状态损坏统一分类为 InvalidState。
                        Errc::InvalidState,
                        // 诊断区分文本提交 owner。
                        "Wayland clipboard text owner mutex poisoned during poll completion",
                    ));
                    // 调用方必须停止本轮 dispatch。
                    return false;
                }
            };
            // FD identity 已在同一 read guard 下验证。
            let Some(read) = active.as_mut() else {
                // 防御性保持健康空 owner 的幂等语义。
                return true;
            };
            // 两个 owner 都健康后才执行非阻塞 I/O。
            match read.read_available() {
                // 尚未读完时保留 read 与文本 owner。
                Ok(NonBlockingReadStatus::Pending) => None,
                // 完成时原子提交文本并释放 read owner。
                Ok(NonBlockingReadStatus::Complete(bytes)) => {
                    // 先提交 UTF-8 容错转换后的文本事实。
                    *clipboard_text = String::from_utf8_lossy(&bytes).into_owned();
                    // 文本成功提交后再释放 read FD owner。
                    *active = None;
                    // 成功完成无需投递 failure。
                    None
                }
                // I/O error 释放失败的 read owner。
                Err(error) => {
                    // 同一健康 guard 内提交 owner 释放。
                    *active = None;
                    // 将 error 带出 guard 作用域。
                    Some(error)
                }
            }
        };

        // I/O error 在所有 clipboard guards 释放后入队。
        if let Some(error) = outcome {
            // failure 进入既有 backend source。
            self.enqueue_failure(Error::new(
                // read syscall failure 保持既有 I/O 分类。
                Errc::IoError,
                // 保留底层 cause 文本。
                format!("Wayland clipboard read failed: {error}"),
            ));
        }
        // 健康 owner 已完成或仍处于 pending。
        true
    }

    // 检查式处理单个 clipboard write completion。
    fn write_clipboard_pipe(
        // event loop owner 负责 completion 编排。
        &mut self,
        // 本轮 poll 返回的 FD identity。
        polled_fd: RawFd,
        // 本轮 FD readiness/error 标志。
        revents: i16,
        // 为该 FD 分配的有界写预算。
        budget: usize,
        // None 专用于传播 owner 状态失败。
    ) -> Option<usize> {
        // write queue 损坏时不得查找、写入或移除任何 FD owner。
        let mut writes = match self.clipboard_writes.lock() {
            // 健康 guard 才允许进入 completion。
            Ok(writes) => writes,
            // 锁中毒必须显式失败。
            Err(_) => {
                // failure 只进入既有 backend source。
                self.enqueue_failure(Error::new(
                    // 状态损坏统一分类为 InvalidState。
                    Errc::InvalidState,
                    // 诊断保留 write owner 与 completion 阶段。
                    "Wayland clipboard write owner mutex poisoned during poll completion",
                ));
                // 调用方必须停止本轮 dispatch 且不消耗预算。
                return None;
            }
        };
        // 在健康队列中定位本轮 poll 的 FD。
        let Some(index) = writes.iter().position(|write| write.fd() == polled_fd) else {
            // 陈旧 FD 不产生写进度，也不属于失败。
            return Some(0);
        };

        // receiver 关闭或 FD 无效时移除对应健康 owner。
        if (revents & (POLLERR | POLLHUP | POLLNVAL)) != 0 {
            // 健康 guard 内提交失败 owner 移除。
            writes.swap_remove(index);
            // 先释放 write guard，避免跨 owner 入队。
            drop(writes);
            // failure 进入既有 backend source。
            self.enqueue_failure(Error::new(
                // receiver 关闭保持既有 I/O 分类。
                Errc::IoError,
                // 保留既有稳定诊断文本。
                "Wayland clipboard receiver closed before send completed",
            ));
            // I/O failure 不消耗本次写预算。
            return Some(0);
        }
        // 非可写 readiness 保持 owner 与预算不变。
        if (revents & POLLOUT) == 0 {
            // 健康但无进度。
            return Some(0);
        }

        // 仅在健康 write owner 上执行有界非阻塞写。
        match writes[index].write_available(budget) {
            // 健康写返回本次实际进度。
            Ok(progress) => {
                // 完成时释放对应 write owner。
                if progress.status == NonBlockingWriteStatus::Complete {
                    // 健康 guard 内提交完成移除。
                    writes.swap_remove(index);
                }
                // Some 区分健康进度与 owner failure。
                Some(progress.written)
            }
            // syscall error 释放失败 owner 并投递 typed failure。
            Err(error) => {
                // 健康 guard 内提交失败 owner 移除。
                writes.swap_remove(index);
                // 先释放 write guard，避免跨 owner 入队。
                drop(writes);
                // failure 进入既有 backend source。
                self.enqueue_failure(Error::new(
                    // write syscall failure 保持既有 I/O 分类。
                    Errc::IoError,
                    // 保留底层 cause 文本。
                    format!("Wayland clipboard send failed: {error}"),
                ));
                // I/O failure 不消耗本次写预算。
                Some(0)
            }
        }
    }

    // 把首次致命运行期失败收敛为幂等 backend 关闭边沿。
    fn close_after_failure(&mut self, code: Errc, message: String) -> bool {
        // 已关闭 backend 不重复入队根因或重复拆除输入代理。
        if self.closed {
            // 所有 dispatch 调用继续观察到退出状态。
            return false;
        }
        // 首次根因必须在 source 仍开放时进入 owner-thread 队列。
        self.enqueue_failure(Error::new(code, message));
        // 先建立 closed 事实，阻止后续入口开始新协议工作。
        self.closed = true;
        // 先失效独立 text-input callback 与 IME session owner。
        self.shutdown_text_input();
        // 文件拖放先释放 offer/read，再注销捕获其状态的 data-device callback。
        self.shutdown_file_drop();
        // 随后立即撤销输入授权、焦点、重复状态与 seat 派生 callbacks。
        self.shutdown_seat_and_input();
        // 输入 callbacks 停止后确定性释放在途 clipboard I/O owners。
        self.shutdown_clipboard_io();
        // 业务回调全部停止后注销 backend 全局 callbacks 与显示状态 owner。
        self.shutdown_global_callbacks();
        // 运行期失败统一终止当前 dispatch。
        false
    }

    pub(crate) fn dispatch_pending_checked(&mut self, context: &str) -> bool {
        match catch_unwind(AssertUnwindSafe(|| {
            self.event_queue.dispatch_pending(&mut self.dispatch_state)
        })) {
            Ok(Ok(_)) => true,
            Ok(Err(error)) => self.close_after_failure(
                Errc::PlatformError,
                format!("Wayland {context} dispatch error: {error}"),
            ),
            Err(_) => self.close_after_failure(
                Errc::PlatformError,
                format!("Wayland {context} callback panicked during dispatch"),
            ),
        }
    }

    // flush 致命失败必须取得 backend owner 并进入同一关闭边沿。
    pub(crate) fn flush_checked(&mut self, context: &str) -> bool {
        // 只向当前 Wayland connection 提交既有请求。
        match self.display.flush() {
            // 健康 flush 允许继续 poll。
            Ok(()) => true,
            // 非阻塞 socket 暂时不可写不是 backend 失效。
            Err(WaylandError::Io(error)) if error.kind() == std::io::ErrorKind::WouldBlock => true,
            // 其他 socket 错误表示本 backend 无法继续调度。
            Err(WaylandError::Io(error)) => self.close_after_failure(
                // 保留既有 I/O 错误分类。
                Errc::IoError,
                // 保留 flush context 与底层错误文本。
                format!("Wayland {context} flush error: {error}"),
            ),
            // Wayland 协议错误同样终止当前 backend 生命周期。
            Err(WaylandError::Protocol(error)) => self.close_after_failure(
                // 保留既有平台协议错误分类。
                Errc::PlatformError,
                // 保留 flush context 与协议错误文本。
                format!("Wayland {context} protocol error: {error}"),
            ),
        }
    }
}
