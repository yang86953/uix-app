//! Wayland 逐窗移动与调整大小协议交互。

// 引入统一结果类型。
use crate::core::Result;
// 引入当前原生 PointerDown 携带的一次性激活身份。
use crate::platform::windowing::event::PointerActivationId;
// 引入平台中立的八方向窗口缩放契约。
use crate::platform::windowing::WindowResizeEdge;
// 引入 xdg-shell 顶层窗口协议及其缩放方向。
use wayland_protocols::xdg::shell::client::xdg_toplevel::ResizeEdge as XdgResizeEdge;

// 引入私有授权消费结果与唯一注册表 owner。
use super::pointer_activation::{PointerActivationOutcome, WaylandPointerActivationRegistry};
// 引入逐窗协议对象 owner。
use super::window_ops::WaylandWindowOps;

// 描述一次由窗口管理器接管的 xdg_toplevel 交互请求。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WaylandInteractiveRequest {
    // 移动不携带额外方向。
    Move,
    // 缩放保留公共八方向值直到协议提交边界。
    Resize(WindowResizeEdge),
}

// 将平台中立方向映射为 xdg-shell wire 枚举。
const fn resize_edge(edge: WindowResizeEdge) -> XdgResizeEdge {
    // 每个公共方向都必须映射为唯一且方向一致的协议值。
    match edge {
        // 上边协议方向。
        WindowResizeEdge::Top => XdgResizeEdge::Top,
        // 下边协议方向。
        WindowResizeEdge::Bottom => XdgResizeEdge::Bottom,
        // 左边协议方向。
        WindowResizeEdge::Left => XdgResizeEdge::Left,
        // 右边协议方向。
        WindowResizeEdge::Right => XdgResizeEdge::Right,
        // 左上角协议方向。
        WindowResizeEdge::TopLeft => XdgResizeEdge::TopLeft,
        // 右上角协议方向。
        WindowResizeEdge::TopRight => XdgResizeEdge::TopRight,
        // 左下角协议方向。
        WindowResizeEdge::BottomLeft => XdgResizeEdge::BottomLeft,
        // 右下角协议方向。
        WindowResizeEdge::BottomRight => XdgResizeEdge::BottomRight,
    }
}

// 返回用于结构化日志的稳定交互类别。
const fn interaction_name(request: WaylandInteractiveRequest) -> &'static str {
    // 日志只区分协议动作，不宣称 compositor 已执行结果。
    match request {
        // 移动动作名称。
        WaylandInteractiveRequest::Move => "move",
        // 所有方向缩放共享动作名称，方向另记为字段。
        WaylandInteractiveRequest::Resize(_) => "resize",
    }
}

// 消费同一次 PointerDown 授权并提交对应 xdg_toplevel 请求。
fn submit(
    // 逐窗 owner 提供 surface、seat、toplevel 与授权注册表。
    window: &mut WaylandWindowOps,
    // 请求值在私有 Component 内决定最终协议方法。
    request: WaylandInteractiveRequest,
    // 未携带身份时不得猜测或复用任何 raw serial。
    pointer_activation: Option<PointerActivationId>,
) -> Result<()> {
    // 为全部退出路径预先建立稳定动作名称。
    let interaction = interaction_name(request);
    // 没有原生 PointerDown 身份的动作不得猜测或复用任何 serial。
    let Some(pointer_activation) = pointer_activation else {
        // 提供稳定调试诊断且不终止任一窗口事件循环。
        tracing::debug!(
            // 记录动作所属窗口用于多窗口排查。
            window_id = window.window_id.raw(),
            // 区分移动与缩放请求。
            interaction,
            // 使用固定原因文本支持定向日志检索。
            reason = "missing_pointer_activation",
            // 说明本次交互请求被安全忽略。
            "Wayland interactive window request ignored"
        );
        // 正常竞态或非原生动作不构成平台错误。
        return Ok(());
    };
    // 已关闭或尚未登记 surface 的窗口不能消费交互授权。
    let Some(surface_id) = window.surface_id else {
        // 记录稳定的 surface 生命周期拒绝原因。
        tracing::debug!(
            // 记录动作所属窗口。
            window_id = window.window_id.raw(),
            // 区分移动与缩放请求。
            interaction,
            // 标明原生 surface 当前不可用。
            reason = "surface_unavailable",
            // 说明本次请求被安全忽略。
            "Wayland interactive window request ignored"
        );
        // surface 缺失是关闭竞态而非致命失败。
        return Ok(());
    };
    // Component 检查共享 owner 后原子校验并消费一次性授权。
    let outcome = WaylandPointerActivationRegistry::consume_checked(
        // 传入 raw serial 的唯一共享 owner。
        &window.pointer_activations,
        // 校验当前原生 PointerDown 身份。
        pointer_activation,
        // 校验动作所属的稳定窗口身份。
        window.window_id,
        // 校验当前 surface 协议身份与代次。
        surface_id,
    )?;
    // 根据私有消费结果决定提交或安全忽略。
    match outcome {
        // 只有完整匹配的一次性授权可以获得 raw serial。
        PointerActivationOutcome::Authorized { serial } => {
            // capability 初始化或关闭竞态可能使 seat 不可用。
            let Some(seat) = window.seat.as_ref() else {
                // serial 已消费，缺少 seat 时绝不回填或伪造。
                tracing::debug!(
                    // 记录动作所属窗口。
                    window_id = window.window_id.raw(),
                    // 区分移动与缩放请求。
                    interaction,
                    // 标明 seat 协议对象不可用。
                    reason = "seat_unavailable",
                    // 说明请求被安全忽略。
                    "Wayland interactive window request ignored"
                );
                // seat 缺失在交互竞态中保持非致命。
                return Ok(());
            };
            // 窗口关闭竞态可能已经释放 xdg_toplevel。
            let Some(toplevel) = window.toplevel.as_ref() else {
                // serial 已消费，缺少顶层对象时不允许重试。
                tracing::debug!(
                    // 记录动作所属窗口。
                    window_id = window.window_id.raw(),
                    // 区分移动与缩放请求。
                    interaction,
                    // 标明顶层协议对象不可用。
                    reason = "toplevel_unavailable",
                    // 说明请求被安全忽略。
                    "Wayland interactive window request ignored"
                );
                // 关闭竞态不应结束应用事件循环。
                return Ok(());
            };
            // 根据私有动作值提交唯一对应的 xdg_toplevel 请求。
            match request {
                // 移动沿用协议保留字方法。
                WaylandInteractiveRequest::Move => toplevel._move(seat.as_ref(), serial),
                // 缩放额外提交精确 xdg-shell 边或角。
                WaylandInteractiveRequest::Resize(edge) => {
                    // 方向在最后协议边界才被解释。
                    toplevel.resize(seat.as_ref(), serial, resize_edge(edge));
                }
            }
            // 记录协议请求已排队，不宣称 compositor 已实际移动或缩放窗口。
            tracing::debug!(
                // 记录提交请求的窗口身份。
                window_id = window.window_id.raw(),
                // 记录授权绑定的 surface 协议身份。
                surface_id,
                // 区分移动与缩放请求。
                interaction,
                // 记录公共缩放方向；移动请求为 None。
                edge = ?match request {
                    // 移动不携带方向。
                    WaylandInteractiveRequest::Move => None,
                    // 缩放保留原始公共方向。
                    WaylandInteractiveRequest::Resize(edge) => Some(edge),
                },
                // 说明请求已交付给 Wayland 代理队列。
                "Wayland interactive window request submitted"
            );
        }
        // 缺失、过期或身份不匹配都是正常的输入生命周期结果。
        PointerActivationOutcome::Ignored(reason) => {
            // 使用结构化原因支持定向回归与现场排查。
            tracing::debug!(
                // 记录动作所属窗口。
                window_id = window.window_id.raw(),
                // 记录当前窗口 surface 身份。
                surface_id,
                // 区分移动与缩放请求。
                interaction,
                // 记录稳定的私有拒绝原因枚举。
                ?reason,
                // 说明本次请求被安全忽略。
                "Wayland interactive window request ignored"
            );
        }
    }
    // 协议请求已排队或正常竞态已安全降级。
    Ok(())
}

// 提交自定义标题栏移动手势。
pub(super) fn begin_move_drag(
    // 逐窗协议 owner 只在本次同步调用中被借用。
    window: &mut WaylandWindowOps,
    // 当前 PointerDown 的不可解释激活身份。
    pointer_activation: Option<PointerActivationId>,
) -> Result<()> {
    // 复用唯一授权消费与协议提交入口。
    submit(window, WaylandInteractiveRequest::Move, pointer_activation)
}

// 提交指定边或角的自定义窗口缩放手势。
pub(super) fn begin_resize_drag(
    // 逐窗协议 owner 只在本次同步调用中被借用。
    window: &mut WaylandWindowOps,
    // 保留 UI 热区声明的精确调整大小方向。
    edge: WindowResizeEdge,
    // 当前 PointerDown 的不可解释激活身份。
    pointer_activation: Option<PointerActivationId>,
) -> Result<()> {
    // 复用唯一授权消费与协议提交入口并携带方向。
    submit(
        // 转交逐窗协议 owner。
        window,
        // 把公共方向保留到最终协议边界。
        WaylandInteractiveRequest::Resize(edge),
        // 转交当前原生事件身份。
        pointer_activation,
    )
}
