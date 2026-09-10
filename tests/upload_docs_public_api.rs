// 声明本文件只编译 Upload UIX 文档，不启动窗口、文件选择或传输。
#![allow(dead_code)]

// 引入文档承诺的 UIX 宏、受控状态与上传类型。
use uix_app::prelude::*;

// 提供应用拥有的类型化上传队列变更处理器。
fn on_upload_change(change: &UploadChange) {
    // 只读取框架发布的最新队列事实，不执行文件或网络 I/O。
    let _files = change.files();
}

// 编译受控文件队列与 Upload UIX 属性映射。
fn compile_example() {
    // 创建由应用拥有的唯一上传队列状态。
    let upload_files = State::new(Vec::<UploadFile>::new());
    // 在 Rust 应用边界计算 UIX 受限表达式不负责的大小策略。
    let max_upload_size = 10_u64 * 1024 * 1024;

    // 把内嵌 UIX 文档生成的 Upload 节点接入公开 View。
    let _upload = embed(uix!(
        r#"
<Upload
    files={upload_files}
    accept=".png,*.jpg"
    multiple
    maxCount="8"
    maxSize={max_upload_size}
    drag
    @change="on_upload_change($event)"
/>
"#
    ));
}
