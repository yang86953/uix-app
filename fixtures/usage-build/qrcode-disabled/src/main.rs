// 故意导入未启用 capability 的公开类型，门禁要求此行无法编译。
use uix::prelude::QRCode;

// 提供 compile-fail fixture 的稳定入口。
fn main() {
    // 若公开面意外泄漏，构造值会使门禁检测到编译成功并失败。
    let qrcode = QRCode::new("https://uix.dev/should-not-compile");
    // 显式消费值，避免公开面泄漏时只产生未使用警告。
    drop(qrcode);
}
