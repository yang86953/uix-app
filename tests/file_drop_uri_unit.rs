// 以子模块方式载入纯 URI Component，保持生产 pub(super) 可见性契约。
#[path = "../src/native/backends/linux/wayland/file_drop_uri.rs"]
// 内嵌 #[cfg(test)] 用例由本测试 crate 的 harness 发现并执行。
mod file_drop_uri;
