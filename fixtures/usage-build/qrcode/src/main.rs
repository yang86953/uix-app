// 导入应用与二维码组件，覆盖单 capability 公开入口。
use uix::prelude::{App, QRCode};

// 提供可执行 fixture 的稳定入口。
fn main() {
    // 构造应用以保留主 crate 基础门面编译验证。
    let app = App::new();
    // 构造二维码组件以证明 capability 公开面可达。
    let qrcode = QRCode::new("https://uix.dev/usage-build");
    // 显式消费两个值，避免无意义的未使用警告。
    drop((app, qrcode));
}
