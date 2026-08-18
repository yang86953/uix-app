//! FramePlan 内类型化资源与 pass-local raster state 的顺序验证。

// 引入稳定错误类型和结果别名。
use crate::core::error::{Errc, Error, Result};
// 引入绘制包与采样语义闭集。
use crate::native::present::rhi::{DrawPacket, PipelineSampling, SampledTextureBinding};

// 引入同层计划命令与 pass。
use super::{FramePlanCommand, RenderPassPlan, RenderTargetRef};

// 验证采样绑定不会把当前离屏输出纹理重新作为采样输入。
pub(super) fn validate_sampled_binding_target(
    // 接收当前 pass 已冻结的逻辑目标。
    target: RenderTargetRef,
    // 接收当前命令即将建立的原子采样绑定。
    binding: SampledTextureBinding,
) -> Result<()> {
    // 只有离屏 texture target 才可能形成纹理反馈环。
    if let RenderTargetRef::Texture(target_texture) = target {
        // 同一纹理同时读写会依赖原生 API 的未定义反馈行为。
        if target_texture == binding.texture() {
            // 在进入 Device 前统一拒绝反馈环参数。
            return Err(Error::new(
                Errc::InvalidArgument,
                "FramePlan texture feedback loop is invalid",
            ));
        }
    }
    // Surface target 或不同纹理绑定不构成反馈环。
    Ok(())
}

// 验证一次 draw 前已经建立完整的 pass-local raster state。
fn validate_raster_state(preceding: &[FramePlanCommand]) -> Result<()> {
    // viewport 必须由当前 FramePlan 显式建立，Adapter 不得继承历史状态。
    if !preceding
        .iter()
        // 只接受当前 draw 之前出现的 viewport 命令。
        .any(|command| matches!(command, FramePlanCommand::SetViewport(_)))
    {
        // 缺失 viewport 属于计划参数错误而不是 Adapter 默认值选择。
        return Err(Error::new(
            Errc::InvalidArgument,
            "FramePlan draw must follow an explicit viewport",
        ));
    }
    // scissor 必须由当前 FramePlan 显式建立，None 也代表明确关闭裁剪。
    if !preceding
        .iter()
        // 只接受当前 draw 之前出现的 Some 或 None scissor 命令。
        .any(|command| matches!(command, FramePlanCommand::SetScissor(_)))
    {
        // 缺失 scissor 属于计划参数错误而不是 Adapter 默认值选择。
        return Err(Error::new(
            Errc::InvalidArgument,
            "FramePlan draw must follow an explicit scissor",
        ));
    }
    // 当前 draw 已拥有完整的 pass-local raster state。
    Ok(())
}

// 验证一次 draw 只能消费前序命令中与其 pipeline 匹配的类型化资源事实。
pub(super) fn validate_draw_uploads(
    // 接收当前完整 pass 以保留 painter order。
    pass: &RenderPassPlan,
    // 接收 draw 在 pass 中的位置，只允许观察已经执行的前序命令。
    command_index: usize,
    // 接收已经绑定句柄与 PipelineKind 的绘制包。
    packet: DrawPacket,
) -> Result<()> {
    // 空顶点句柄不能进入不同 Adapter 的资源表解释。
    if packet.vertex_buffer.raw() == 0 {
        // 使用计划参数错误而不是让后端各自返回不同状态。
        return Err(Error::new(
            Errc::InvalidArgument,
            "FramePlan draw vertex buffer must be bound",
        ));
    }
    // 读取绑定身份唯一允许的顶点、Uniform、采样和混合事实。
    let contract = packet.pipeline.contract();
    // 只检查 draw 之前已经生效的命令。
    let preceding = &pass.commands[..command_index];
    // FramePlan 必须在进入 Adapter 前拥有完整的 raster state。
    validate_raster_state(preceding)?;
    // 查找同一顶点 buffer 最近一次类型化上传。
    let latest_vertex = preceding.iter().rev().find_map(|command| {
        // 只有句柄匹配的顶点上传才影响当前 draw。
        match command {
            // 返回最近一次匹配的类型化顶点载荷。
            FramePlanCommand::UploadVertex { buffer, data, .. }
                if *buffer == packet.vertex_buffer =>
            {
                // 借出载荷供共享布局比较。
                Some(data)
            }
            // 其它命令不改变当前顶点 buffer 的布局事实。
            _ => None,
        }
    });
    // 每个 draw 都必须在当前 pass 内先建立同一 vertex buffer 的类型化内容事实。
    let data = latest_vertex.ok_or_else(|| {
        // 禁止 Adapter 复用未由当前 FramePlan 明确交付的旧顶点内容。
        Error::new(
            Errc::InvalidArgument,
            "FramePlan draw must follow a typed vertex upload",
        )
    })?;
    // 先行类型化上传必须与 pipeline 的共享顶点布局完全一致。
    if data.layout() != contract.vertex {
        // 在进入 Adapter 前拒绝布局错配。
        return Err(Error::new(
            Errc::InvalidArgument,
            "FramePlan vertex upload layout does not match pipeline contract",
        ));
    }
    // 索引 DrawRange 必须消费同一 buffer 的最近一次类型化索引上传。
    if let Some(index_binding) = packet.range.index_binding() {
        // 只观察 draw 前且句柄匹配的索引上传。
        let latest_index = preceding.iter().rev().find_map(|command| {
            // 只有类型化索引上传才建立可读取的索引内容事实。
            match command {
                // 返回最近一次匹配索引 buffer 的类型化载荷。
                FramePlanCommand::UploadIndex { buffer, data }
                    if *buffer == index_binding.buffer() =>
                {
                    // 借出载荷供格式和范围比较。
                    Some(data)
                }
                // 其它命令不改变当前索引 buffer 的内容事实。
                _ => None,
            }
        });
        // 缺少前序类型化索引上传不得让 Adapter 读取旧内容。
        let index_data = latest_index.ok_or_else(|| {
            // 使用稳定参数错误明确缺少 typed index upload。
            Error::new(
                Errc::InvalidArgument,
                "FramePlan indexed draw must follow a typed index upload",
            )
        })?;
        // 索引 payload 格式必须与同次 DrawRange 绑定格式一致。
        if index_data.format() != index_binding.format() {
            // 禁止 Adapter 按自身索引格式猜测字节宽度。
            return Err(Error::new(
                Errc::InvalidArgument,
                "FramePlan index upload format does not match draw range",
            ));
        }
        // 索引范围必须完整落在本次类型化索引上传内。
        let selected_max = index_data
            .max_index_in_range(packet.range.first_index(), packet.range.index_count())
            .ok_or_else(|| {
                // 使用稳定参数错误明确范围越过 typed index upload。
                Error::new(
                    Errc::InvalidArgument,
                    "FramePlan draw index range exceeds typed upload",
                )
            })?;
        // 读取同次 draw 已匹配的最近顶点 payload 完整顶点数。
        let uploaded_vertex_count = data.vertex_count().ok_or_else(|| {
            // 残缺顶点 payload 不得成为索引访问的隐式依据。
            Error::new(
                Errc::InvalidArgument,
                "FramePlan indexed draw vertex exceeds typed upload",
            )
        })?;
        // 最大索引必须严格落在最近类型化顶点 payload 内。
        if selected_max >= uploaded_vertex_count {
            // 在进入 Device 前统一拒绝越界顶点访问。
            return Err(Error::new(
                Errc::InvalidArgument,
                "FramePlan indexed draw vertex exceeds typed upload",
            ));
        }
    }
    // 非索引 DrawRange 必须完整落在最近一次类型化顶点上传内。
    if packet.range.index_binding().is_none() {
        // checked 末端溢出必须在进入任一 Adapter 前拒绝。
        let vertex_end = packet.range.checked_vertex_end().ok_or_else(|| {
            // 使用稳定参数错误表达共享 u32 末端无法表示。
            Error::new(
                Errc::InvalidArgument,
                "FramePlan draw vertex range end is invalid",
            )
        })?;
        // 从同一类型化 payload 派生完整顶点数量。
        let uploaded_vertex_count = data.vertex_count().ok_or_else(|| {
            // 残缺 payload 不得成为范围比较的隐式依据。
            Error::new(
                Errc::InvalidArgument,
                "FramePlan vertex upload vertex count is invalid",
            )
        })?;
        // DrawRange 末端不得超过最近上传的完整顶点数量。
        if vertex_end > uploaded_vertex_count {
            // 在 Device 执行前统一拒绝越界读取。
            return Err(Error::new(
                Errc::InvalidArgument,
                "FramePlan draw vertex range exceeds typed upload",
            ));
        }
    }
    // 当前固定 pipeline 都要求一个完整类型化 Uniform buffer。
    let uniform_buffer = packet.uniform_buffer.ok_or_else(|| {
        // 缺失常量不能由 Adapter 用零值或旧帧数据猜测。
        Error::new(
            Errc::InvalidArgument,
            "FramePlan draw uniform buffer must be bound",
        )
    })?;
    // 查找同一 Uniform buffer 在 draw 前最近一次完整上传。
    let latest_uniform = preceding.iter().rev().find_map(|command| {
        // 只接受类型化 Uniform 命令。
        match command {
            // 返回最近一次匹配的共享值对象。
            FramePlanCommand::UploadUniform { buffer, data } if *buffer == uniform_buffer => {
                // 借出 Copy 载荷供布局比较。
                Some(*data)
            }
            // 其它命令不改变当前 Uniform buffer 的语义事实。
            _ => None,
        }
    });
    // 每次 draw 都必须显式刷新其依赖的 viewport 与图元常量。
    let uniform = latest_uniform.ok_or_else(|| {
        // 禁止复用不可审计的旧帧 Uniform 内容。
        Error::new(
            Errc::InvalidArgument,
            "FramePlan draw must follow a typed uniform upload",
        )
    })?;
    // 类型化值对象必须与 pipeline 的唯一 Uniform ABI 匹配。
    if uniform.layout() != contract.uniform {
        // 在字节编码前拒绝任何语义错位。
        return Err(Error::new(
            Errc::InvalidArgument,
            "FramePlan uniform upload layout does not match pipeline contract",
        ));
    }
    // 查找 draw 前最近一次完整的类型化采样绑定。
    let latest_sampled = preceding.iter().rev().find_map(|command| {
        // 只有采样绑定命令才更新当前 pipeline 的资源事实。
        match command {
            // 返回最近一次原子采样绑定。
            FramePlanCommand::BindSampledTexture(binding) => Some(*binding),
            // 其它命令不改变采样绑定事实。
            _ => None,
        }
    });
    // 需要采样的 pipeline 必须拥有与自身语义匹配的最近绑定。
    if contract.sampling != PipelineSampling::None {
        // 缺少绑定必须在进入 Device 前被共享层拒绝。
        let binding = latest_sampled.ok_or_else(|| {
            // 禁止不同 API 复用各自残留的纹理状态。
            Error::new(
                Errc::InvalidArgument,
                "FramePlan sampled draw must follow a texture binding",
            )
        })?;
        // 绑定语义必须与当前 Draw pipeline contract 完全一致。
        if !binding.matches_pipeline(packet.pipeline) {
            // 禁止错误 pipeline 继续把资源兼容性推迟给 Adapter。
            return Err(Error::new(
                Errc::InvalidArgument,
                "FramePlan sampled binding does not match pipeline contract",
            ));
        }
    }
    // 当前 draw 的所有类型化资源事实与共享 pipeline 契约一致。
    Ok(())
}
