//! 自 src/draw/mod.rs 迁出的文档测试样例；原检查语义保留，仅源仓 RUSTDOCFLAGS
//! --cfg uix_repo_doc_contract 下的 cargo test --doc 收集，不进发布包。


/// 迁出的文档样例 1（原 src/draw/mod.rs:28）：
/// ```compile_fail
/// // 旧平铺路径不得复活：Canvas2D 归 painting。
/// use uix_app::draw::api::Canvas2D;
/// ```
pub struct DocExample1;


/// 迁出的文档样例 2（原 src/draw/mod.rs:33）：
/// ```compile_fail
/// // 旧平铺路径不得复活：FrameEncoder 归 painting。
/// use uix_app::draw::command::FrameEncoder;
/// ```
pub struct DocExample2;


/// 迁出的文档样例 3（原 src/draw/mod.rs:38）：
/// ```compile_fail
/// // CPU 光栅化器已上移 System 边界，不得经 backend 路径访问。
/// use uix_app::draw::backend::cpu::rasterizer::SoftwareRasterizer;
/// ```
pub struct DocExample3;


/// 迁出的文档样例 4（原 src/draw/mod.rs:43）：
/// ```compile_fail
/// // NodeId 归 scene，renderer 路径不得复活。
/// use uix_app::draw::renderer::NodeId;
/// ```
pub struct DocExample4;


/// 迁出的文档样例 5（原 src/draw/mod.rs:48）：
/// ```compile_fail
/// // 自动探测与 CPU fallback 属于框架内部策略，不是第二套公开图形选择入口。
/// use uix_app::draw::renderer::bootstrap::bootstrap_renderer;
/// ```
pub struct DocExample5;
