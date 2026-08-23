    // 引入当前模块的组件与步骤类型。
    use super::{Step, StepStatus, Steps};
    // 引入公开响应式状态句柄。
    use crate::ui::State;

    // 验证外部更新、用户选择和越界归一化共享同一 State。
    #[test]
    // 声明双向受控 current 回归。
    fn controlled_current_stays_bidirectional() {
        // 创建声明端唯一状态源。
        let current = State::new(1_usize);
        // 构造三个可选步骤并绑定 current。
        let mut steps = Steps::new(vec![Step::new("A"), Step::new("B"), Step::new("C")])
            // 绑定声明端 current。
            .current_state(&current);
        // 初次物化必须读取外部值。
        assert_eq!(steps.get_current(), 1);
        // 模拟组件拥有的用户选择路径。
        steps.select(2);
        // 用户选择必须先写回外部 State。
        assert_eq!(current.get(), 2);
        // 外部业务随后切换到首步骤。
        current.set(0);
        // 输入入口采用的同步 helper 应吸收外部更新。
        steps.sync_bound_value();
        // 组件镜像必须与 State 一致。
        assert_eq!(steps.get_current(), 0);
        // 外部写入越界索引。
        current.set(9);
        // 同步时执行组件域归一化。
        steps.sync_bound_value();
        // 组件必须钳制到末步骤。
        assert_eq!(steps.get_current(), 2);
        // 唯一状态源也必须收到相同归一化值。
        assert_eq!(current.get(), 2);
    }

    // 验证声明树重建采用新绑定值而不沿用旧交互镜像。
    #[test]
    // 声明 reconcile 受控状态回归。
    fn reconcile_prefers_the_declared_current_state() {
        // 创建初始声明状态。
        let current = State::new(0_usize);
        // 物化初始受控组件。
        let mut steps = Steps::new(vec![Step::new("A"), Step::new("B")])
            // 绑定初始 current。
            .current_state(&current);
        // 模拟旧实例中的用户交互镜像。
        steps.select(1);
        // 业务在下一次声明前改回首步骤。
        current.set(0);
        // 构造下一棵声明树中的组件配置。
        let next = Steps::new(vec![Step::new("A"), Step::new("B"), Step::new("C")])
            // 继续绑定同一业务状态。
            .current_state(&current);
        // 让运行时复用旧组件实例并同步新声明。
        steps.sync_from(next);
        // reconcile 必须服从声明端值。
        assert_eq!(steps.get_current(), 0);
        // 步骤数据也必须采用新声明。
        assert_eq!(steps.step_count(), 3);
    }

    // 验证默认步骤状态随 current 推进，显式状态仍可覆盖自动结果。
    #[test]
    fn current_drives_implicit_step_statuses() {
        // 当前位于第二步时，前项完成、当前项处理中、后项等待。
        let steps = Steps::new(vec![Step::new("A"), Step::new("B"), Step::new("C")]).current(1);
        assert_eq!(
            steps.steps[0].resolved_status(0, steps.get_current()),
            StepStatus::Finish
        );
        assert_eq!(
            steps.steps[1].resolved_status(1, steps.get_current()),
            StepStatus::Process
        );
        assert_eq!(
            steps.steps[2].resolved_status(2, steps.get_current()),
            StepStatus::Wait
        );

        // 显式错误状态不得被 current 覆盖。
        let error = Step::new("B").status(StepStatus::Error);
        assert_eq!(error.resolved_status(1, 0), StepStatus::Error);
        // 显式 Wait 同样必须保持等待，而不是退回自动推导。
        let wait = Step::new("B").status(StepStatus::Wait);
        assert_eq!(wait.resolved_status(1, 1), StepStatus::Wait);
    }
