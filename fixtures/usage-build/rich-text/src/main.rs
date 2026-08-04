// 导入应用、富文本组件与解析辅助，覆盖单 capability 公开入口。
use uix::prelude::{parse_rich_text, App, RichText};

// 提供可执行 fixture 的稳定入口。
fn main() {
    // 构造应用以保留主 crate 基础门面编译验证。
    let app = App::new();
    // 解析一段包含链接与强调的富文本内容。
    let segments = parse_rich_text("**UIX** [文档](https://uix.dev)");
    // 构造富文本组件以证明 capability 公开面可达。
    let rich_text = RichText::new().content(segments);
    // 显式消费两个值，避免无意义的未使用警告。
    drop((app, rich_text));
}
