// 引入公开 ViewNode 类型以约束宏展开结果。
use uix::prelude::ViewNode;

// 验证关闭全部默认 capability 后仍可编译核心 FocusTrap 映射。
#[test]
// 声明无默认特性的 FocusTrap 外部消费测试。
fn public_uix_macro_compiles_focus_trap_without_default_features() {
    // 让过程宏生成核心 FocusTrap、两个按钮后代与公共自动化属性。
    let _view: ViewNode = uix::uix!(
        r#"<FocusTrap automationId="core-focus-scope"><Button>确定</Button><Button>取消</Button></FocusTrap>"#
    );
    // 编译成功即证明映射不依赖 feedback 或其他可选 capability。
}
