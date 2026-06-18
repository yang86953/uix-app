//! 此文件已弃用 —— LayerTree 的渲染由外部（Window 层）管理，
//! SoftwareEngine 不再持有 LayerTree，消除了 unsafe 指针拆分问题。
//! 渲染逻辑已合并到 graphics_impl.rs 中的 render_frame 方法。
