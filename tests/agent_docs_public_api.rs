// 声明本文件只编译 Agent 控制文档，不运行应用或发布 IPC 端点。
#![allow(dead_code)]

// 隔离 agent-enable 围栏中的显式应用门禁。
mod agent_enable {
    // 引入文档承诺的公开 Application 与 View prelude。
    use uix::prelude::*;

    // 编译 agent-control feature 与 Application 开关的第二层门禁。
    fn documented_main() {
        // 配置显式启用本机 Agent 端点的应用。
        App::new()
            // 启用 Application 运行期开关。
            .enable_agent_control()
            // 安装业务根视图。
            .root(main_view)
            // 保留文档运行入口供编译器检查。
            .run();
    }

    // 声明示例所需的业务根视图。
    fn main_view() -> ViewNode {
        // 返回稳定的占位业务内容。
        label("应用")
    }
}

// 隔离 agent-policy 围栏中的动作授权策略。
mod agent_policy {
    // 引入公开语义动作类别。
    use uix::ui::SemanticActionKind;
    // 引入文档承诺的公开 Application 与 View prelude。
    use uix::prelude::*;

    // 编译只读、目标保护、动作拒绝与确认要求的策略链。
    fn documented_main() {
        // 配置按应用组合根拥有的 Agent 授权策略。
        App::new()
            // 显式启用 Agent 控制入口。
            .enable_agent_control()
            // 收紧为只读策略。
            .agent_read_only()
            // 保护破坏性组件的稳定身份。
            .agent_protect("delete-button")
            // 全局拒绝修改输入值的语义动作。
            .agent_deny_action(SemanticActionKind::SetValue)
            // 为指定破坏性目标要求用户确认。
            .agent_require_confirm("danger-button")
            // 安装业务根视图。
            .root(main_view)
            // 保留文档运行入口供编译器检查。
            .run();
    }

    // 声明示例所需的业务根视图。
    fn main_view() -> ViewNode {
        // 返回稳定的占位业务内容。
        label("应用")
    }
}

// 隔离 agent-confirm-ui 围栏中的用户确认注入。
mod agent_confirm_ui {
    // 引入公开的 Agent 确认请求值类型。
    use uix::app::AgentConfirmationRequest;
    // 引入文档承诺的公开 Application 与 View prelude。
    use uix::prelude::*;

    // 编译确认目标声明与 UI turn 回调注入。
    fn documented_main() {
        // 配置需要用户确认的 Agent 应用入口。
        App::new()
            // 显式启用 Agent 控制入口。
            .enable_agent_control()
            // 为破坏性目标要求用户确认。
            .agent_require_confirm("danger-button")
            // 注入只接收公开确认请求的应用 UI 回调。
            .agent_confirm_ui(|request: AgentConfirmationRequest| {
                // 由真实应用展示确认界面并在用户决定后回交结果。
                let _ = request;
            })
            // 安装业务根视图。
            .root(main_view)
            // 保留文档运行入口供编译器检查。
            .run();
    }

    // 声明示例所需的业务根视图。
    fn main_view() -> ViewNode {
        // 返回稳定的占位业务内容。
        label("应用")
    }
}

// 隔离 agent-automation-id 围栏中的稳定语义身份。
mod agent_automation_id {
    // 引入文档承诺的公开 Application、组件与语义扩展 prelude。
    use uix::prelude::*;

    // 编译只通过稳定 automation_id 暴露语义目标的根视图。
    fn compile_example() {
        // 构造启用 Agent 控制但尚未运行的应用。
        let app = App::new()
            // 显式启用 Agent 控制入口。
            .enable_agent_control()
            // 声明两个具有稳定身份的语义节点。
            .root(|| {
                // 纵向组合保存动作与状态文本。
                column((
                    // 为保存按钮声明稳定动作身份。
                    button("保存").automation_id("save-button"),
                    // 为状态文本声明稳定读取身份。
                    label("状态").automation_id("status-label"),
                ))
            });
        // 消费未运行的 App builder，避免测试产生原生副作用。
        let _ = app;
    }
}
