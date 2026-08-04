// 故意导入未启用 capability 的组件与辅助函数，门禁要求此行无法编译。
use uix::prelude::{parse_rich_text, RichText};

// 提供 compile-fail fixture 的稳定入口。
fn main() {
    // 若解析辅助意外泄漏，这次调用会使门禁检测到对应公开面仍可达。
    let segments = parse_rich_text("不应编译的富文本");
    // 若组件类型意外泄漏，构造值会阻止负向场景静默通过。
    let rich_text = RichText::new().content(segments);
    // 显式消费值，避免公开面泄漏时只产生未使用警告。
    drop(rich_text);
}
