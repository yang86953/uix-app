// Wayland cursor-state Component 独占期望形状、可见性与 Enter serial。

// 原子状态允许 seat callback 与同步平台端口共享而不引入可中毒 owner。
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

// CursorType 是跨平台公开形状契约，协议映射仍留在 Adapter。
use crate::platform::windowing::CursorType;

// 零值保留给“尚无 Enter serial”，真实 u32 serial 统一加一保存。
const NO_ENTER_SERIAL: u64 = 0;

// Component 对外只暴露一次一致用途的只读快照。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WaylandCursorSnapshot {
    // 最近一次 wl_pointer.enter 的协议授权。
    pub(crate) enter_serial: Option<u32>,
    // 下次有焦点时应应用的平台无关形状。
    pub(crate) cursor: CursorType,
    // false 表示下次 Enter 必须继续隐藏指针。
    pub(crate) visible: bool,
}

// 三份原子事实共同组成 backend 生命周期内唯一 cursor intent。
#[derive(Debug)]
pub(crate) struct WaylandCursorState {
    // u32 serial 加一后保存，避免用协议零值伪造 None。
    enter_serial: AtomicU64,
    // 私有编码只服务进程内 CursorType 快照。
    cursor: AtomicU32,
    // 可见性默认开启，与 compositor 默认行为一致。
    visible: AtomicBool,
}

// 新 backend 从可见默认箭头且无 pointer focus 开始。
impl Default for WaylandCursorState {
    // 构造不产生任何 Wayland 协议请求。
    fn default() -> Self {
        // 初始化三份独立但同 owner-thread 提交的原子事实。
        Self {
            // 初始没有可用于 set_shape/set_cursor 的 Enter serial。
            enter_serial: AtomicU64::new(NO_ENTER_SERIAL),
            // 默认形状映射为 Arrow 的稳定私有编码。
            cursor: AtomicU32::new(encode_cursor(CursorType::Arrow)),
            // compositor 默认显示指针。
            visible: AtomicBool::new(true),
        }
    }
}

// Component 端口只提交 intent 或 pointer-focus 生命周期事实。
impl WaylandCursorState {
    // 读取当前 intent，供同步端口或 Enter callback 决定协议动作。
    pub(crate) fn snapshot(&self) -> WaylandCursorSnapshot {
        // SeqCst 保持三份 owner-thread 事实的直观观察顺序。
        WaylandCursorSnapshot {
            // 解码最近 Enter serial；零值明确表示无焦点。
            enter_serial: decode_serial(self.enter_serial.load(Ordering::SeqCst)),
            // 私有编码只会还原为受支持的公开枚举。
            cursor: decode_cursor(self.cursor.load(Ordering::SeqCst)),
            // 读取期望可见性。
            visible: self.visible.load(Ordering::SeqCst),
        }
    }

    // Enter callback 先发布新 serial，再取得需要立即重放的完整 intent。
    pub(crate) fn record_enter(&self, serial: u32) -> WaylandCursorSnapshot {
        // u64 空间可无损保存 u32::MAX 加一。
        self.enter_serial
            // 发布本次协议事件携带的唯一合法 serial。
            .store(u64::from(serial) + 1, Ordering::SeqCst);
        // 返回包含新 serial 的重放快照。
        self.snapshot()
    }

    // Leave、pointer capability 移除与 backend shutdown 共用幂等失效端口。
    pub(crate) fn clear_enter(&self) {
        // 清零后同步入口只能记录待下次 Enter 应用的 intent。
        self.enter_serial
            // 不保留跨 pointer focus 或代理代次的授权。
            .store(NO_ENTER_SERIAL, Ordering::SeqCst);
    }

    // 协议请求已排队或无焦点待应用时才发布新形状 intent。
    pub(crate) fn commit_cursor(&self, cursor: CursorType) {
        // Adapter 已拒绝 Custom，Component 仍保留全枚举稳定编码。
        self.cursor
            // 提交下次 Enter 需要重放的形状。
            .store(encode_cursor(cursor), Ordering::SeqCst);
    }

    // 协议请求已排队或无焦点待应用时才发布可见性 intent。
    #[allow(dead_code)] // ICursor 已实现该能力，但当前 App 生产路径尚未调用 show_cursor。
    pub(crate) fn commit_visibility(&self, visible: bool) {
        // 同一值重复提交保持幂等。
        self.visible.store(visible, Ordering::SeqCst);
    }
}

// 把公开枚举收窄成只在本 Component 内持久的稳定整数。
const fn encode_cursor(cursor: CursorType) -> u32 {
    // 显式匹配避免依赖公开枚举未来的声明顺序。
    match cursor {
        // 默认箭头使用零值。
        CursorType::Arrow => 0,
        // 文本光标使用一。
        CursorType::IBeam => 1,
        // 十字光标使用二。
        CursorType::Crosshair => 2,
        // 手形光标使用三。
        CursorType::Hand => 3,
        // 水平缩放使用四。
        CursorType::ResizeH => 4,
        // 垂直缩放使用五。
        CursorType::ResizeV => 5,
        // 东北到西南缩放使用六。
        CursorType::ResizeNE => 6,
        // 西北到东南缩放使用七。
        CursorType::ResizeNW => 7,
        // 移动光标使用八。
        CursorType::Move => 8,
        // 等待光标使用九。
        CursorType::Wait => 9,
        // 禁止光标使用十。
        CursorType::NotAllowed => 10,
        // Custom 仅可作为未提交候选参与测试，Adapter 不会发布它。
        CursorType::Custom => 11,
    }
}

// 从私有原子值恢复平台枚举；未知内存值安全回落为默认箭头。
fn decode_cursor(encoded: u32) -> CursorType {
    // 每个已登记编码保持一一对应。
    match encoded {
        // 文本光标编码。
        1 => CursorType::IBeam,
        // 十字光标编码。
        2 => CursorType::Crosshair,
        // 手形光标编码。
        3 => CursorType::Hand,
        // 水平缩放编码。
        4 => CursorType::ResizeH,
        // 垂直缩放编码。
        5 => CursorType::ResizeV,
        // 东北到西南缩放编码。
        6 => CursorType::ResizeNE,
        // 西北到东南缩放编码。
        7 => CursorType::ResizeNW,
        // 移动光标编码。
        8 => CursorType::Move,
        // 等待光标编码。
        9 => CursorType::Wait,
        // 禁止光标编码。
        10 => CursorType::NotAllowed,
        // Custom 编码只允许 Component round-trip，不代表协议支持。
        11 => CursorType::Custom,
        // 零值与未知值都安全回落到默认箭头。
        _ => CursorType::Arrow,
    }
}

// 解码保留零值的 Enter serial。
fn decode_serial(encoded: u64) -> Option<u32> {
    // 只有非零编码代表真实协议 serial。
    encoded
        // 先还原加一编码。
        .checked_sub(1)
        // 构造保证结果始终位于 u32 范围。
        .and_then(|serial| u32::try_from(serial).ok())
}
