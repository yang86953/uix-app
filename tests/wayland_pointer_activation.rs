// 测试目标在 Windows 主机上复用公开 WindowId 值类型。
mod core {
    // 重新导出真实稳定窗口身份供私有注册表源码使用。
    pub use uix::core::WindowId;
// 结束 core 测试适配模块。
}

// 测试目标只为源码级私有模块提供等价的不透明身份路径。
mod native {
    // 保持与生产模块一致的 windowing 层级。
    pub(crate) mod windowing {
        // 保持与生产模块一致的 event 层级。
        pub(crate) mod event {
            // 测试身份只比较因果关联，不承载任何 Wayland 数据。
            #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
            // 维持与生产不透明身份相同的私有形状。
            pub(crate) struct PointerActivationId(u64);

            // 提供注册表源码所需的受控构造入口。
            impl PointerActivationId {
                // 使用测试签发的非零编号建立身份。
                pub(crate) const fn new(raw: u64) -> Self {
                    // 保存不可解释编号。
                    Self(raw)
                // 结束测试身份构造。
                }
            // 结束测试身份实现。
            }
        // 结束 event 测试适配模块。
        }
    // 结束 windowing 测试适配模块。
    }
// 结束 native 测试适配模块。
}

// 直接编译生产私有注册表，使 Windows 聚焦测试覆盖真实状态机。
#[path = "../src/native/backends/linux/wayland/pointer_activation.rs"]
// 注册表不依赖 Wayland 动态库或 Linux 系统调用。
mod pointer_activation;

// 直接编译生产 capability 边沿决策，使 Windows 测试执行真实幂等策略。
#[path = "../src/native/backends/linux/wayland/input_proxy_lifecycle.rs"]
// 该纯模块不依赖 Wayland 协议对象或 Linux 系统调用。
mod input_proxy_lifecycle;

// 引入稳定窗口身份构造测试场景。
use core::WindowId;
// 引入生产注册表及其确定性消费结果。
use pointer_activation::{
    // 授权结果用于验证精确 serial 与一次性消费。
    PointerActivationOutcome,
    // 拒绝原因用于验证错事件、错窗口和错 surface 不串用。
    PointerActivationRejection,
    // 注册表是本测试的唯一状态机受测对象。
    WaylandPointerActivationRegistry,
// 结束生产注册表类型导入。
};
// 引入生产输入代理边沿决策与结果枚举。
use input_proxy_lifecycle::{input_proxy_transition, InputProxyTransition};

// 验证重复 capability 快照只创建和释放一次输入代理。
#[test]
// 该回归执行生产纯决策，不依赖源码文本匹配。
fn repeated_capability_snapshots_have_idempotent_proxy_edges() {
    // 模拟 owner 槽当前是否持有唯一代理。
    let mut proxy_bound = false;
    // 记录 bind 边沿实际发生次数。
    let mut bind_count = 0;
    // 记录 release 边沿实际发生次数。
    let mut release_count = 0;
    // 依次模拟有、有、无、无、重新有的完整 capability 快照。
    for capability_available in [true, true, false, false, true] {
        // 使用生产决策计算当前唯一生命周期动作。
        match input_proxy_transition(capability_available, proxy_bound) {
            // capability 首次出现或重新出现时绑定。
            InputProxyTransition::Bind => {
                // 更新模拟 owner 槽状态。
                proxy_bound = true;
                // 精确记录一次创建。
                bind_count += 1;
            // 结束 bind 分支。
            }
            // capability 真正丢失时释放。
            InputProxyTransition::Release => {
                // 更新模拟 owner 槽状态。
                proxy_bound = false;
                // 精确记录一次释放。
                release_count += 1;
            // 结束 release 分支。
            }
            // 重复快照不得改变 owner 状态或计数。
            InputProxyTransition::Unchanged => {}
        // 结束生命周期动作分派。
        }
    // 结束 capability 快照序列。
    }
    // 首次出现与 regain 各创建一次。
    assert_eq!(bind_count, 2);
    // 连续两次缺失快照只释放一次。
    assert_eq!(release_count, 1);
    // 最终 regain 后 owner 槽应重新持有代理。
    assert!(proxy_bound);
// 结束 capability 幂等回归测试。
}

// 验证同一 native 批次中的后续 press 不会被较早事件错误消费。
#[test]
// 该回归直接锁定 down1、up1、down2 的批量分发时序。
fn batched_press_release_press_never_cross_consumes_serials() {
    // 创建空的 Wayland 私有授权注册表。
    let mut registry = WaylandPointerActivationRegistry::new();
    // 建立稳定的目标窗口身份。
    let window = WindowId::new(7);
    // 登记该窗口当前 surface。
    registry.register_surface(31, window);
    // 捕获当前 pointer 代理代次。
    let pointer_generation = registry.pointer_generation();
    // 为第一下主键 press 签发身份与 serial。
    let first = registry
        // 绑定代次、surface、窗口与第一下 serial。
        .issue_primary_press(pointer_generation, 31, window, 101)
        // 当前注册完整，签发必须成功。
        .expect("第一下主键 press 应签发激活身份");
    // 同批 release 在 app 处理 down1 前撤销第一下授权。
    registry.revoke_primary_press(pointer_generation);
    // 为同批后续第二下主键 press 签发新的身份与 serial。
    let second = registry
        // 绑定相同窗口 surface 但不同 press serial。
        .issue_primary_press(pointer_generation, 31, window, 202)
        // 第二下 press 仍应独立签发成功。
        .expect("第二下主键 press 应签发新激活身份");
    // app 随后处理 down1 时不得消费当前属于 down2 的 serial。
    assert_eq!(
        // 尝试以第一下事件身份消费。
        registry.consume(first, window, 31),
        // 注册表必须保留第二下授权并报告身份不匹配。
        PointerActivationOutcome::Ignored(PointerActivationRejection::ActivationMismatch),
    // 结束第一下错序消费断言。
    );
    // app 处理 down2 时只能得到第二下精确 serial。
    assert_eq!(
        // 使用第二下事件身份消费。
        registry.consume(second, window, 31),
        // 返回 compositor 为第二下 press 签发的值。
        PointerActivationOutcome::Authorized { serial: 202 },
    // 结束第二下精确消费断言。
    );
    // 同一身份再次请求必须被一次性语义拒绝。
    assert_eq!(
        // 重放已经消费的第二下身份。
        registry.consume(second, window, 31),
        // 已消费与已撤销统一表现为稳定非致命缺失。
        PointerActivationOutcome::Ignored(PointerActivationRejection::MissingOrRevoked),
    // 结束一次性消费断言。
    );
// 结束批量分发回归测试。
}

// 验证窗口、surface 与 pointer 生命周期均不能串用授权。
#[test]
// 该测试覆盖错误目标保留授权以及各失效边沿。
fn authorization_isolated_and_revoked_across_lifecycles() {
    // 创建独立注册表状态。
    let mut registry = WaylandPointerActivationRegistry::new();
    // 建立授权所属窗口。
    let first_window = WindowId::new(11);
    // 建立另一个不可消费授权的窗口。
    let second_window = WindowId::new(12);
    // 登记第一个窗口 surface。
    registry.register_surface(41, first_window);
    // 捕获初始 pointer 代理代次。
    let first_generation = registry.pointer_generation();
    // 签发第一个可消费授权。
    let activation = registry
        // 绑定第一个窗口与 surface。
        .issue_primary_press(first_generation, 41, first_window, 303)
        // 当前注册完整，签发必须成功。
        .expect("已登记 surface 应签发授权");
    // 其他窗口不能消费该授权。
    assert_eq!(
        // 使用错误窗口请求同一身份。
        registry.consume(activation, second_window, 41),
        // 报告窗口不匹配且保留有效授权。
        PointerActivationOutcome::Ignored(PointerActivationRejection::WindowMismatch),
    // 结束跨窗口拒绝断言。
    );
    // 同窗错误 surface 也不能消费该授权。
    assert_eq!(
        // 使用错误 surface 请求同一身份。
        registry.consume(activation, first_window, 42),
        // 报告 surface 不匹配且保留有效授权。
        PointerActivationOutcome::Ignored(PointerActivationRejection::SurfaceMismatch),
    // 结束跨 surface 拒绝断言。
    );
    // 正确窗口与 surface 仍能消费原授权。
    assert_eq!(
        // 使用精确目标消费。
        registry.consume(activation, first_window, 41),
        // 得到原 press 的精确 serial。
        PointerActivationOutcome::Authorized { serial: 303 },
    // 结束精确目标消费断言。
    );
    // 签发用于 leave 撤销场景的新授权。
    let leave_activation = registry
        // 绑定当前 pointer 代次和相同窗口 surface。
        .issue_primary_press(first_generation, 41, first_window, 404)
        // leave 前授权应存在。
        .expect("leave 前应存在授权");
    // pointer leave 撤销精确 surface 上的未消费授权。
    registry.revoke_pointer_focus(first_generation, 41, first_window);
    // leave 后不得重放该授权。
    assert_eq!(
        // 尝试消费已由 leave 撤销的身份。
        registry.consume(leave_activation, first_window, 41),
        // 返回稳定非致命缺失。
        PointerActivationOutcome::Ignored(PointerActivationRejection::MissingOrRevoked),
    // 结束 leave 失效断言。
    );
    // 签发用于 capability loss 场景的新授权。
    let lost_activation = registry
        // 使用仍有效的旧 pointer 代次签发。
        .issue_primary_press(first_generation, 41, first_window, 505)
        // 能力丢失前授权应存在。
        .expect("capability loss 前应存在授权");
    // 模拟 Pointer capability 从有到无。
    registry.invalidate_pointer();
    // 能力丢失后旧授权不得重放。
    assert_eq!(
        // 尝试消费旧 pointer 代次身份。
        registry.consume(lost_activation, first_window, 41),
        // 已撤销授权安全表现为缺失。
        PointerActivationOutcome::Ignored(PointerActivationRejection::MissingOrRevoked),
    // 结束 capability loss 失效断言。
    );
    // 旧 pointer 回调不得在新代次签发授权。
    assert_eq!(
        // 使用旧代次模拟迟到 press。
        registry.issue_primary_press(first_generation, 41, first_window, 606),
        // 迟到回调必须被拒绝。
        None,
    // 结束旧代理迟到事件断言。
    );
    // 获取 capability regain 后的新 pointer 代次。
    let second_generation = registry.pointer_generation();
    // 新 pointer 代次可以正常签发授权。
    let close_activation = registry
        // 使用新代次和仍登记的 surface 签发。
        .issue_primary_press(second_generation, 41, first_window, 707)
        // regain 后签发必须恢复。
        .expect("新 pointer 代次应签发授权");
    // 窗口关闭或 surface 注销撤销对应授权。
    registry.unregister_surface(41, first_window);
    // 关闭后不得消费旧 surface 授权。
    assert_eq!(
        // 尝试消费已注销 surface 的身份。
        registry.consume(close_activation, first_window, 41),
        // 返回稳定非致命缺失。
        PointerActivationOutcome::Ignored(PointerActivationRejection::MissingOrRevoked),
    // 结束 surface 注销断言。
    );
    // 复用同一协议编号时建立新的窗口注册代次。
    registry.register_surface(41, second_window);
    // 新窗口的新 press 可以独立签发。
    let reused_activation = registry
        // 绑定复用编号的新窗口与当前 pointer 代次。
        .issue_primary_press(second_generation, 41, second_window, 808)
        // 新注册代次必须正常签发。
        .expect("复用 surface 编号的新注册应签发授权");
    // 新注册只能由新窗口消费。
    assert_eq!(
        // 使用新窗口与复用 surface 消费。
        registry.consume(reused_activation, second_window, 41),
        // 得到新 press 的精确 serial。
        PointerActivationOutcome::Authorized { serial: 808 },
    // 结束 surface 复用隔离断言。
    );
// 结束生命周期隔离回归测试。
}

// 验证生产接线保持 raw Wayland 授权私有且主副窗口都转交事件身份。
#[test]
// 静态契约补足 Windows 主机无法编译 Linux adapter 的验证边界。
fn production_wiring_keeps_wayland_authorization_private_and_causal() {
    // 取得仓库根目录用于读取生产接线源码。
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    // 读取 Wayland seat 输入与 capability 生命周期接线。
    let seat = std::fs::read_to_string(
        // 定位 Wayland seat owner 源码。
        root.join("src/native/backends/linux/wayland/seat.rs"),
    // 源码必须以 UTF-8 可读。
    )
    // 读取失败表示测试环境或接线文件损坏。
    .expect("读取 Wayland seat 接线");
    // 读取 Wayland 窗口协议提交接线。
    let window_ops = std::fs::read_to_string(
        // 定位 WaylandWindowOps 源码。
        root.join("src/native/backends/linux/wayland/window_ops.rs"),
    // 源码必须以 UTF-8 可读。
    )
    // 读取失败表示测试环境或接线文件损坏。
    .expect("读取 Wayland window ops 接线");
    // 读取主窗口事件到动作执行边界。
    let main_loop = std::fs::read_to_string(
        // 定位主窗口事件循环源码。
        root.join("src/app/event_loop/event_loop.rs"),
    // 源码必须以 UTF-8 可读。
    )
    // 读取失败表示主窗口接线不可验证。
    .expect("读取主窗口事件循环接线");
    // 读取副窗口事件到动作执行边界。
    let secondary = std::fs::read_to_string(
        // 定位副窗口会话源码。
        root.join("src/app/application/application/secondary.rs"),
    // 源码必须以 UTF-8 可读。
    )
    // 读取失败表示副窗口接线不可验证。
    .expect("读取副窗口事件接线");
    // 读取窗口动作映射以锁定 UI 动作枚举不携带平台数据。
    let actions = std::fs::read_to_string(
        // 定位 app 窗口命令适配源码。
        root.join("src/app/window/window_actions.rs"),
    // 源码必须以 UTF-8 可读。
    )
    // 读取失败表示动作接线不可验证。
    .expect("读取窗口动作接线");

    // seat 必须从同一焦点快照取得 surface 与窗口身份。
    assert!(seat.contains(".pointer_target_identity()"));
    // BTN_LEFT press 必须在 Wayland 私有注册表签发身份。
    assert!(seat.contains(".issue_primary_press("));
    // 签发身份必须附着到产生它的同一个 native 事件。
    assert!(seat.contains(".with_pointer_activation(activation)"));
    // BTN_LEFT release 必须撤销未消费授权。
    assert!(seat.contains(".revoke_primary_press(pointer_generation)"));
    // pointer leave 必须撤销对应 surface 的未消费授权。
    assert!(seat.contains(".revoke_pointer_focus("));
    // seat 必须使用生产纯决策处理完整 capability 快照。
    assert!(seat.contains("input_proxy_transition("));
    // 首个 capability 快照前不得创建未受管且可能非法的 keyboard 代理。
    assert!(!seat.contains("let _kbd = seat.get_keyboard();"));
    // seat callback 只能弱引用输入代理 owner 槽，避免强引用环。
    assert!(seat.contains("Arc::downgrade(&self.pointer)"));
    // keyboard owner 槽必须采用同一弱引用策略。
    assert!(seat.contains("Arc::downgrade(&self.keyboard)"));
    // capability loss 必须注销旧兼容回调。
    assert!(seat.contains("pointer.clear_callback();"));
    // 支持协议版本时 capability loss 必须发送 release。
    assert!(seat.contains("pointer.release();"));
    // keyboard capability loss 必须对称注销旧回调。
    assert!(seat.contains("keyboard.clear_callback();"));
    // backend Drop 使用的统一关闭入口必须存在。
    assert!(seat.contains("shutdown_seat_and_input"));
    // WaylandWindowOps 必须以当前事件身份原子消费注册表授权。
    assert!(window_ops.contains(".consume(pointer_activation, self.window_id, surface_id)"));
    // 精确匹配后必须调用生成的 Rust `_move` 方法。
    assert!(window_ops.contains("toplevel._move(seat.as_ref(), serial);"));
    // 正常过期与错目标必须走非致命 Ignored 分支。
    assert!(window_ops.contains("PointerActivationOutcome::Ignored(reason)"));
    // 窗口协议层不得读取剪贴板用途的全局最新 serial。
    assert!(!window_ops.contains("last_input_serial"));
    // 主窗口必须只转交当前 native 事件的激活身份。
    assert!(main_loop.contains("ev.pointer_activation()"));
    // 副窗口必须采用与主窗口相同的激活身份转交策略。
    assert!(secondary.contains("event.pointer_activation()"));
    // UI 动作枚举保持零平台负载，app 仅在执行时传上下文。
    assert!(actions.contains("WindowAction::BeginMoveDrag => window.begin_move_drag(pointer_activation)"));
    // 原生接管后 app 必须显式清除 UI pointer 手势。
    assert!(actions.contains("tree.cancel_pointer_gesture_for_native_handoff();"));
// 结束生产接线静态契约测试。
}
