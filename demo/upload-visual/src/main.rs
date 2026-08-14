// 导入应用构建器、上传队列契约与 uix-lang 编译期入口。
use uix::prelude::*;
// 引入进程级受控队列状态的单次初始化容器。
use std::sync::OnceLock;

// 保持验收页面 reconcile 后仍使用同一上传队列真值。
static UPLOAD_FILES: OnceLock<State<Vec<UploadFile>>> = OnceLock::new();

// 为独立验收页面构造调用方拥有的上传队列状态。
fn upload_files() -> State<Vec<UploadFile>> {
    // 首次构造包含四种视觉状态的唯一队列，后续只克隆同一状态句柄。
    UPLOAD_FILES
        // 延迟初始化避免静态构造阶段创建响应式状态。
        .get_or_init(|| {
            // 构造等待上传的文件项。
            let pending = UploadFile::new("产品原型.png", 2_457_600);
            // 构造上传中的文件项。
            let mut uploading = UploadFile::new("交互说明,最终版.jpg", 6_291_456);
            // 由应用服务写回上传中状态。
            uploading.status = UploadStatus::Uploading;
            // 使用中段进度核对进度条宽度。
            uploading.progress = 0.58;
            // 构造已完成文件项。
            let mut done = UploadFile::new("验收记录.png", 845_120);
            // 由应用服务写回完成状态。
            done.status = UploadStatus::Done;
            // 完成项显示完整进度。
            done.progress = 1.0;
            // 构造失败文件项。
            let mut failed = UploadFile::new("失败样例.jpg", 3_145_728);
            // 由应用服务写回错误状态。
            failed.status = UploadStatus::Error;
            // 保存包含四种视觉状态的唯一队列真值。
            State::new(vec![pending, uploading, done, failed])
        })
        // 返回轻量句柄而不复制状态所有权。
        .clone()
}

// 接收 UIX 类型化 UploadChange 借用以核对处理器签名。
fn record_upload(_change: &UploadChange) {
    // 真窗验收直接观察状态写回后的队列，不启动网络传输。
}

// 启动 Upload 独立真窗验收应用。
fn main() {
    // 编译期读取验收声明并进入现有原生窗口事件循环。
    uix_app!("src/main.uix").run();
}
