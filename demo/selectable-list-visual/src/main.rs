// 导入应用构建器、类型化条目与 uix-lang 编译期入口。
use uix::prelude::*;

// 为 UIX 验收声明提供拥有型稳定条目集合。
fn selectable_items() -> Vec<SelectableItem> {
    // 返回包含图标与纯文本形态的确定性条目。
    vec![
        // 首项用于核对初始空选中后的指针激活。
        SelectableItem::new("overview", "项目概览").icon("home"),
        // 次项用于核对键盘向下导航与活动高亮。
        SelectableItem::new("tasks", "任务队列").icon("check-square"),
        // 第三项用于核对稳定 id 与展示文字分离。
        SelectableItem::new("settings", "项目设置").icon("settings"),
        // 末项不带图标，用于核对文本对齐与 End 键。
        SelectableItem::new("help", "帮助与支持"),
    ]
}

// 接收 UIX Change 载荷以在编译期核对稳定 id 处理器签名。
fn record_active(_active_id: &str) {
    // 真窗验收只观察状态写回后的高亮，不产生额外副作用。
}

// 启动 SelectableList 独立真窗验收应用。
fn main() {
    // 编译期读取验收声明并进入现有原生窗口事件循环。
    uix_app!("src/main.uix").run();
}
