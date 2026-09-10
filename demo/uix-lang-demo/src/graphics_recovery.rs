// 导入线程安全的逐应用控制器所有权基元。
use std::sync::{Arc, Mutex};

// 导入 App、AppHandle、State 与声明式 View 入口。
use uix_app::prelude::*;

// 保存初始等待状态的稳定语义文本。
const WAITING_STATUS: &str = "图形恢复测试：等待注入";
// 保存故障已安排且等待恢复后交互的稳定语义文本。
const INJECTED_STATUS: &str = "图形恢复测试：已注入，等待恢复后交互";
// 保存恢复后交互成功的稳定语义文本。
const VERIFIED_STATUS: &str = "图形恢复测试：恢复后交互成功";

// 区分公开测试入口支持的两类图形故障。
#[derive(Clone, Copy)]
enum GraphicsFault {
    // 表示设备级丢失。
    DeviceLost,
    // 表示交换表面级丢失。
    SurfaceLost,
}

// 持有专用验收页面的状态与逐窗 AppHandle。
struct GraphicsRecoveryController {
    // 保存 Agent 语义树可观察的当前验收状态。
    status: State<String>,
    // 保存恢复后交互按钮的唯一禁用状态。
    verification_disabled: State<bool>,
    // 只保存 on_start 交付的逐窗句柄，不建立全局服务定位器。
    handle: Mutex<Option<AppHandle>>,
}

// 实现验收页面的狭窄状态机与公开故障入口适配。
impl GraphicsRecoveryController {
    // 创建尚未取得窗口句柄的初始状态机。
    fn new() -> Self {
        // 返回等待首次注入的控制器。
        Self {
            // 初始语义状态明确等待注入。
            status: State::new(WAITING_STATUS.to_string()),
            // 尚未恢复前禁止后续交互断言。
            verification_disabled: State::new(true),
            // AppHandle 只能由 App System 的 on_start 回调安装。
            handle: Mutex::new(None),
        }
    }

    // 安装当前验收窗口的 Application System 句柄。
    fn attach_handle(&self, handle: AppHandle) {
        // 中毒锁仍取回唯一内部值，避免另造句柄所有者。
        let mut slot = self
            // 锁定狭窄的可选句柄槽。
            .handle
            // 等待当前短事务完成。
            .lock()
            // 保留中毒后可恢复的唯一值。
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // 用 on_start 交付的当前逐窗句柄填充空槽。
        *slot = Some(handle);
    }

    // 请求在下一次真实绘制中注入指定图形故障。
    fn inject(&self, fault: GraphicsFault) {
        // 在锁内只克隆轻量 AppHandle，不跨公开调用持锁。
        let handle = self
            // 锁定唯一句柄槽。
            .handle
            // 等待当前短事务完成。
            .lock()
            // 保留中毒后可恢复的唯一值。
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            // 借用已经安装的句柄。
            .as_ref()
            // 克隆 Application System 公共句柄供本次调用使用。
            .cloned();
        // 缺失句柄时返回类型化状态错误，不伪造注入成功。
        let result = handle
            // 把可选句柄转换为类型化结果。
            .ok_or_else(|| {
                Error::new(
                    Errc::InvalidState,
                    "graphics recovery AppHandle is not ready",
                )
            })
            // 只调用 AppHandle 已公开的 test-harness 边界。
            .and_then(|handle| match fault {
                // 设备丢失进入 Application System 的设备故障入口。
                GraphicsFault::DeviceLost => handle.inject_graphics_device_lost_for_test(),
                // 表面丢失进入 Application System 的表面故障入口。
                GraphicsFault::SurfaceLost => handle.inject_graphics_surface_lost_for_test(),
            });
        // 把安排结果转换为下一帧可观察状态。
        self.record_injection_result(result);
    }

    // 把故障安排结果投影为声明页的确定状态。
    fn record_injection_result(&self, result: std::result::Result<(), Error>) {
        // 分别处理已安排与类型化失败。
        match result {
            // 已安排故障时开放恢复后交互断言。
            Ok(()) => {
                // 先开放后续按钮。
                self.verification_disabled.set(false);
                // 再更新语义文本，以强制故障后的下一帧真实呈现。
                self.status.set(INJECTED_STATUS.to_string());
            }
            // 注入失败时保留不可继续状态并公开原因。
            Err(error) => {
                // 禁止把失败路径误判成可验证恢复。
                self.verification_disabled.set(true);
                // 把类型化错误投影为可观察文本。
                self.status
                    // 保留固定前缀并附加底层错误。
                    .set(format!("图形恢复测试：注入失败：{error}"));
            }
        }
    }

    // 记录故障恢复后仍能执行用户交互。
    fn verify_recovered_interaction(&self) {
        // 禁用状态下不允许 Agent 或指针绕过注入前置条件。
        if self.verification_disabled.get() {
            // 保持等待或失败状态不变。
            return;
        }
        // 更新为稳定成功文本并触发一次新的真实呈现。
        self.status.set(VERIFIED_STATUS.to_string());
    }
}

// 构造由现有 Application System 持有的专用验收 App。
pub(super) fn build_app() -> App {
    // 创建单个页面控制器并由 App 根工厂共享。
    let controller = Arc::new(GraphicsRecoveryController::new());
    // 克隆控制器供 on_start 安装逐窗句柄。
    let start_controller = controller.clone();
    // 返回尚未运行的现有 App builder。
    App::new()
        // 使用独立标题明确测试载体身份。
        .title("UIX 图形恢复验收")
        // 使用足够容纳三项操作的固定初始尺寸。
        .size(720, 460)
        // 每次协调只重建声明式 View，不替换控制器或 AppHandle。
        .root(move || build_view(controller.clone()))
        // 仅由 Application System 生命周期回调交付逐窗句柄。
        .on_start(move |handle| start_controller.attach_handle(handle))
}

// 从共享控制器构造一次声明式验收视图。
fn build_view(controller: Arc<GraphicsRecoveryController>) -> ViewNode {
    // 克隆状态句柄供声明式文本建立结构订阅。
    let status = controller.status.clone();
    // 克隆禁用状态供按钮建立结构订阅。
    let verification_disabled = controller.verification_disabled.clone();
    // 为设备故障按钮创建独占控制器闭包。
    let inject_device_lost = {
        // 克隆共享控制器进入按钮事件。
        let controller = controller.clone();
        // 只请求设备故障，不直接操作图形后端。
        move || controller.inject(GraphicsFault::DeviceLost)
    };
    // 为表面故障按钮创建独占控制器闭包。
    let inject_surface_lost = {
        // 克隆共享控制器进入按钮事件。
        let controller = controller.clone();
        // 只请求表面故障，不直接操作图形后端。
        move || controller.inject(GraphicsFault::SurfaceLost)
    };
    // 为恢复后交互按钮创建独占控制器闭包。
    let verify_recovered_interaction = {
        // 移动最后一份局部控制器进入按钮事件。
        let controller = controller;
        // 只推进页面状态，不建立第二条 UI 写管线。
        move || controller.verify_recovered_interaction()
    };
    // 编译独立声明文件并返回单一 ViewNode。
    uix!("src/graphics_recovery.uix")
}

// 保存不依赖真实窗口的图形恢复页面状态机回归测试。
#[cfg(test)]
mod tests {
    // 导入当前模块的私有控制器与稳定状态文本。
    use super::*;

    // 验证成功安排故障后可以完成恢复后交互。
    #[test]
    fn advances_from_waiting_to_injected_and_verified() {
        // 创建未绑定真实窗口的纯状态控制器。
        let controller = GraphicsRecoveryController::new();
        // 初始状态必须等待故障注入。
        assert_eq!(controller.status.get(), WAITING_STATUS);
        // 初始状态必须禁止恢复后交互。
        assert!(controller.verification_disabled.get());
        // 模拟 Application System 已成功安排一次故障。
        controller.record_injection_result(Ok(()));
        // 状态必须进入等待恢复后交互。
        assert_eq!(controller.status.get(), INJECTED_STATUS);
        // 成功安排后必须开放后续交互。
        assert!(!controller.verification_disabled.get());
        // 模拟恢复呈现后的用户确认动作。
        controller.verify_recovered_interaction();
        // 最终状态必须留下稳定的交互成功证据。
        assert_eq!(controller.status.get(), VERIFIED_STATUS);
    }

    // 验证 on_start 尚未交付句柄时不能伪造注入成功。
    #[test]
    fn rejects_injection_before_app_handle_is_attached() {
        // 创建尚未取得逐窗 AppHandle 的控制器。
        let controller = GraphicsRecoveryController::new();
        // 尝试安排设备故障。
        controller.inject(GraphicsFault::DeviceLost);
        // 失败路径必须保持后续交互禁用。
        assert!(controller.verification_disabled.get());
        // 可观察状态必须明确报告注入失败。
        assert!(
            controller
                // 读取最新状态快照。
                .status
                // 取得字符串值。
                .get()
                // 只锁定稳定失败前缀。
                .starts_with("图形恢复测试：注入失败：")
        );
    }
}
