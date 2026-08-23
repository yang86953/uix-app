//! OpenGL ES 对共享类型化顶点属性序列的机械映射。

// 引入 glow 的上下文扩展方法。
use glow::HasContext as _;
// 引入统一结果类型。
use crate::core::error::Result;
// 引入薄 RHI 拥有的槽位、格式与布局事实。
use crate::platform::presentation::rhi::{
    PIPELINE_VERTEX_ATTRIBUTE_SLOT_COUNT, PipelineVertexFormat, PipelineVertexLayout,
};
// 引入当前 OpenGL Device 的稳定参数错误构造器。
use super::rhi_invalid;

// 把共享顶点格式翻译为 OpenGL 元素类型与归一化开关。
fn gl_vertex_attribute_type(format: PipelineVertexFormat) -> (u32, bool) {
    // 穷尽共享格式闭集，新增格式时必须显式映射。
    match format {
        // float1 保持原始浮点值。
        PipelineVertexFormat::Float32 => (glow::FLOAT, false),
        // float2 保持原始浮点值。
        PipelineVertexFormat::Float32x2 => (glow::FLOAT, false),
        // float4 保持原始浮点值。
        PipelineVertexFormat::Float32x4 => (glow::FLOAT, false),
    }
}

/// 从共享类型化序列配置当前 VAO 的全部顶点属性。
///
/// # Safety
/// 调用者必须保证当前绑定的是与 layout 匹配的 VAO 和顶点 buffer，且 context current。
pub(super) unsafe fn configure_vertex_attributes(
    // 接收当前有效 OpenGL 上下文。
    gl: &glow::Context,
    // 接收 FramePlan pipeline 拥有的共享布局。
    layout: PipelineVertexLayout,
) -> Result<()> {
    // 共享布局本身必须满足槽位、偏移与步长值域。
    if !layout.is_valid() {
        // 返回稳定的薄 RHI 参数错误。
        return Err(rhi_invalid("OpenGL RHI vertex layout is invalid"));
    }
    // 读取经过共享层检查的有符号步长。
    let stride = layout
        // 使用唯一 checked 投影，禁止 Adapter 直接强转。
        .stride_bytes_i32()
        // 防御未来新增布局绕过共享门禁。
        .ok_or_else(|| rhi_invalid("OpenGL RHI vertex stride is invalid"))?;
    // 先关闭共享槽位集合，避免较小布局继承上一 packet 的属性。
    for location in 0..PIPELINE_VERTEX_ATTRIBUTE_SLOT_COUNT {
        // SAFETY：调用者已绑定当前 VAO，位置落在共享槽位集合内。
        unsafe { gl.disable_vertex_attrib_array(location) };
    }
    // 按共享序列逐项机械映射原生属性。
    for attribute in layout.attributes() {
        // 读取该格式对应的 OpenGL 元素类型与归一化语义。
        let (element_type, normalized) = gl_vertex_attribute_type(attribute.format());
        // 读取经过共享层检查的有符号字节偏移。
        let offset = attribute
            // 使用唯一 checked 投影，禁止 Adapter 重写偏移。
            .offset_bytes_i32()
            // 防御未来新增属性绕过共享门禁。
            .ok_or_else(|| rhi_invalid("OpenGL RHI vertex offset is invalid"))?;
        // SAFETY：位置、分量、格式、步长和偏移全部来自已验证共享布局。
        unsafe {
            gl.vertex_attrib_pointer_f32(
                // 使用共享 shader 位置。
                attribute.location(),
                // 使用共享格式分量数量。
                attribute.format().component_count_i32(),
                // 使用当前 Adapter 的机械格式映射。
                element_type,
                // 使用当前 Adapter 的机械归一化映射。
                normalized,
                // 使用共享布局步长。
                stride,
                // 使用共享属性偏移。
                offset,
            )
        };
        // SAFETY：位置来自已验证共享槽位集合，当前 VAO 已绑定。
        unsafe { gl.enable_vertex_attrib_array(attribute.location()) };
    }
    // 全部共享属性已经完成配置。
    Ok(())
}
