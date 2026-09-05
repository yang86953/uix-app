//! 真实终端演示组合：PTY 会话生命周期由 Rust 宿主显式拥有，
//! 声明式页面只经 KernelView 拿到屏幕投影。
//!
//! SMC 职责：会话权威在本模块的 `LIVE` 槽（UI 线程独占）；读线程经
//! `on_output` → `AppHandle::post_to_ui` 回 UI 线程驱动 pump。屏幕重绘由
//! 会话自身的输出代际 State 精确失效（组件 render 已订阅）；启停与宽度
//! 切换是结构性变化，经 phase State 重建投影。页面卸载或应用退出时
//! drop 会话即完成关闭、SIGHUP 与子进程回收。

use std::cell::RefCell;
use std::sync::{Mutex, OnceLock};
use uix::prelude::*;

// UI 线程独占的演示状态；跨线程只经 post_to_ui 进入。
struct LiveTerminalDemo {
    session: Option<TerminalSession>,
    wide: bool,
    last_error: Option<String>,
}

// 会话句柄内部是 Rc：只能挂在 UI 线程的 thread_local 上。
thread_local! {
    static LIVE: RefCell<Option<LiveTerminalDemo>> = const { RefCell::new(None) };
}
// 启停与宽度属于面板结构：变化时经 map 重建投影。
static PHASE: OnceLock<State<(bool, bool)>> = OnceLock::new();
static UI_HANDLE: Mutex<Option<AppHandle>> = Mutex::new(None);

// 安装主窗口句柄，供读线程唤醒回调 post_to_ui；on_start 时调用一次。
pub(crate) fn install_ui_handle(handle: &AppHandle) {
    let mut slot = UI_HANDLE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    *slot = Some(handle.clone());
}

fn phase_state() -> &'static State<(bool, bool)> {
    PHASE.get_or_init(|| State::new((false, false)))
}

// 读线程唤醒入口：UI 线程消费输出；重绘由会话代际 State 精确驱动。
fn pump_live_output() {
    with_live(|live| live.session.as_ref().is_some_and(TerminalSession::pump));
}

// 惰性初始化并访问演示状态；回调在借用外执行避免重建重入。
fn with_live<R>(visit: impl FnOnce(&mut LiveTerminalDemo) -> R) -> Option<R> {
    LIVE.with(|slot| {
        let mut guard = slot.borrow_mut();
        if guard.is_none() {
            *guard = Some(LiveTerminalDemo {
                session: None,
                wide: false,
                last_error: None,
            });
        }
        guard.as_mut().map(visit)
    })
}

fn set_phase(running: bool, wide: bool) {
    phase_state().set((running, wide));
}

// 按当前事实重建相位。
fn bump_phase() {
    let (running, wide) = with_live(|live| (live.session.is_some(), live.wide))
        .unwrap_or((false, false));
    set_phase(running, wide);
}

// 声明式按钮的启停入口：未运行时启动，运行中关闭。
pub fn live_terminal_toggle() {
    let running = with_live(|live| live.session.is_some()).unwrap_or(false);
    if running {
        close_live_terminal();
    } else {
        start_live_terminal();
    }
}

// 显式启动真实交互式 shell；失败保留 typed 摘要给面板展示。
pub fn start_live_terminal() {
    let Some(handle) = UI_HANDLE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
    else {
        return;
    };
    let mut config = TerminalSessionConfig::default();
    // 演示会话禁用 Powerlevel10k 配置向导：向导会抢占首个交互回合，
    // 且只写进本会话环境，不修改用户 shell 配置。
    config.env = vec![(
        "POWERLEVEL9K_DISABLE_CONFIGURATION_WIZARD".to_string(),
        "true".to_string(),
    )];
    config.on_output = Some(std::sync::Arc::new(move || {
        handle.post_to_ui(pump_live_output);
    }));
    match TerminalSession::spawn(&config) {
        Ok(session) => {
            with_live(|live| {
                live.session = Some(session);
                live.last_error = None;
            });
            bump_phase();
        }
        Err(error) => {
            with_live(|live| {
                live.last_error = Some(error.what().to_string());
            });
            bump_phase();
        }
    }
}

// 显式关闭会话：drop 句柄即完成 fd 关闭、SIGHUP、join 与子进程回收。
pub fn close_live_terminal() {
    with_live(|live| live.session = None);
    bump_phase();
}

// 切换演示宽度：组件布局变化换算新列数并同步 PTY。
pub fn live_terminal_toggle_width() {
    with_live(|live| live.wide = !live.wide);
    bump_phase();
}

// 演示投影：由声明式页面的 KernelView 引用；phase 变化重建结构，
// 会话输出只经组件订阅的代际 State 精确重绘屏幕。
pub fn live_terminal_view() -> ViewNode {
    phase_state().map(|_| build_live_terminal_body())
}

fn build_live_terminal_body() -> ViewNode {
    let (running, wide, error) =
        with_live(|live| (live.session.is_some(), live.wide, live.last_error.clone()))
            .unwrap_or((false, false, None));
    if running {
        let session = with_live(|live| live.session.clone()).flatten();
        match session {
            Some(session) => embed(TerminalScreen::new(&session))
                .width(if wide { 660.0 } else { 440.0 })
                .height(340.0)
                .align_self(AlignItems::Start)
                .automation_id("live-terminal-screen"),
            None => embed(label("会话已关闭")),
        }
    } else if let Some(error) = error {
        embed(label(format!("启动失败：{error}")).color(Color::RED))
    } else {
        embed(label("点击上方「启动 / 关闭会话」在 Linux PTY 上运行真实交互式终端。"))
    }
}
