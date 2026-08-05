// 故意导入未启用 capability 的基础与泛型导航类型，门禁要求此行无法编译。
use uix::prelude::{Breadcrumb, Navigation};

// 提供 compile-fail fixture 的稳定入口。
fn main() {
    // 若基础面包屑公开面意外泄漏，构造值会使门禁检测到编译成功并失败。
    let breadcrumb = Breadcrumb::new();
    // 若泛型导航公开面意外泄漏，类型标注会阻止部分门控静默通过。
    let navigation: Option<Navigation<u8>> = None;
    // 显式消费值，避免公开面泄漏时只产生未使用警告。
    drop((breadcrumb, navigation));
}
