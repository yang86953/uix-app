// 引入生成代码承诺调用的公开 prelude。
use crate::prelude::*;

// 验证 FocusTrap active 只依赖公开 UIX 运行时契约。
#[test]
fn focus_trap_active_compiles_against_public_uix_api() {
    // 声明调用方拥有的普通布尔启用事实。
    let trap_active = false;
    // 展开包含动态开关与真实焦点后代的消费者文档。
    let _view: ViewNode = uix!(
        r#"<FocusTrap active={trap_active}><Button>确定</Button><Button>取消</Button></FocusTrap>"#
    );
}
