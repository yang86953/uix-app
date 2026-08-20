// 引入生成代码承诺调用的公开 prelude。
use crate::prelude::*;

// 验证 Popover 完整配置只依赖公开 UIX 运行时契约。
#[test]
fn popover_configuration_compiles_against_public_uix_api() {
    // 创建由调用方拥有且允许运行时写回的打开状态。
    let popover_open = State::new(false);
    // 展开完整标题、方向、箭头、焦点触发和受控打开的真实消费者文档。
    let _view: ViewNode = uix!(
        r#"<Popover content="详情" title="帮助" placement="rightTop" arrow trigger="focus" open={popover_open}><Button>查看</Button></Popover>"#
    );
}
