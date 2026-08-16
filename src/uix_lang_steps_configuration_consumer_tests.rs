// 引入宏生成代码承诺使用的公开 prelude。
use crate::prelude::*;

// 验证 Steps 点击策略与圆点样式只依赖公开 UIX 运行时契约。
#[test]
fn steps_configuration_compiles_against_public_uix_api() {
    // 创建调用方拥有的当前步骤状态。
    let current_step = State::new(0_usize);
    // 创建公开 Step 元数据集合。
    let step_items = vec![
        // 首步使用完成状态。
        Step::new("注册").status(StepStatus::Finish),
        // 次步使用进行中状态。
        Step::new("验证").status(StepStatus::Process),
        // 末步使用等待状态。
        Step::new("完成").status(StepStatus::Wait),
    ];
    // 声明由 Rust 类型系统核对的动态点击策略。
    let allow_step_change = true;
    // 声明由 Rust 类型系统核对的动态圆点样式。
    let show_step_dots = false;
    // 展开动态点击策略与圆点布尔简写。
    let _interactive: ViewNode = crate::uix!(
        // 使用完整已登记 Steps UIX 形状。
        r#"<Steps current={current_step} items={step_items} clickable={allow_step_change} dot />"#
    );
    // 展开静态只读策略与动态圆点样式。
    let _read_only: ViewNode = crate::uix!(
        // 复用同一状态与数据以证明生成器只取得声明快照。
        r#"<Steps current={current_step} items={step_items} direction="vertical" clickable="false" dot={show_step_dots} />"#
    );
}
