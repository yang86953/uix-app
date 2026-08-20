// 引入生成代码承诺调用的公开 prelude。
use crate::prelude::*;

// 验证 Tooltip 方向与触发方式只依赖公开 UIX 运行时契约。
#[test]
fn tooltip_configuration_compiles_against_public_uix_api() {
    // 展开右侧上下文菜单触发和真实按钮子树的消费者文档。
    let _view: ViewNode = uix!(
        r#"<Tooltip text="更多操作" placement="right" trigger="contextMenu"><Button>打开菜单</Button></Tooltip>"#
    );
}
