//! FramePlan 内类型化资源与 DrawPacket 原子契约验证。

// 引入稳定错误类型和结果别名。
use crate::core::error::{Errc, Error, Result};
// 引入绘制包与封闭条件采样角色。
use crate::native::present::rhi::{DrawPacket, DrawSamplingBinding};

// 引入同层计划命令与 pass。
use super::{FramePlanCommand, RenderPassPlan, RenderTargetRef};

// 验证当前 Draw 的条件采样角色不会把离屏输出重新作为输入。
pub(super) fn validate_draw_sampling_target(
    // 接收当前 pass 已冻结的逻辑目标。
    target: RenderTargetRef,
    // 接收当前 DrawPacket 自身拥有的条件采样角色。
    sampling: DrawSamplingBinding,
) -> Result<()> {
    // 无采样 Draw 不携带任何可能形成反馈环的纹理身份。
    let Some(binding) = sampling.sampled_texture() else {
        // 明确无采样可以直接通过目标关系门禁。
        return Ok(());
    };
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
    // Surface target、无采样或不同纹理绑定不构成反馈环。
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
    // 一次取得 DrawPacket 不可拆的顶点与 Uniform 资源身份。
    let buffers = packet.buffers();
    // 一次取得 DrawPacket 已冻结的互斥绘制范围。
    let range = packet.range();
    // 空顶点句柄不能进入不同 Adapter 的资源表解释。
    if buffers.vertex().raw() == 0 {
        // 使用计划参数错误而不是让后端各自返回不同状态。
        return Err(Error::new(
            Errc::InvalidArgument,
            "FramePlan draw vertex buffer must be bound",
        ));
    }
    // 空 Uniform 句柄也不能作为 typed bindings 的有效资源身份。
    if buffers.uniform().raw() == 0 {
        // 使用计划参数错误保持两个 Adapter 的拒绝分类一致。
        return Err(Error::new(
            Errc::InvalidArgument,
            "FramePlan draw uniform buffer must be bound",
        ));
    }
    // 条件采样资源必须由当前 DrawPacket 自身完整拥有并匹配 pipeline。
    if !packet.has_valid_sampling() {
        // 禁止 Adapter 用 pass 历史状态补齐缺失或错配绑定。
        return Err(Error::new(
            Errc::InvalidArgument,
            "FramePlan draw sampling binding does not match pipeline contract",
        ));
    }
    // 动态栅格状态必须由当前 DrawPacket 自身完整拥有。
    if !packet.has_valid_raster() {
        // 禁止 Adapter 从 pass 历史状态补齐 viewport 或 scissor。
        return Err(Error::new(
            Errc::InvalidArgument,
            "FramePlan draw raster state is invalid",
        ));
    }
    // 读取绑定身份唯一允许的顶点、Uniform、采样和混合事实。
    let contract = packet.pipeline().contract();
    // 只检查 draw 之前已经生效的命令。
    let preceding = &pass.commands[..command_index];
    // 查找同一顶点 buffer 最近一次类型化上传。
    let latest_vertex = preceding.iter().rev().find_map(|command| {
        // 只有句柄匹配的顶点上传才影响当前 draw。
        match command {
            // 返回最近一次匹配的类型化顶点载荷。
            FramePlanCommand::UploadVertex { buffer, data, .. } if *buffer == buffers.vertex() => {
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
    if let Some(index_binding) = range.index_binding() {
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
            .max_index_in_range(range.first_index(), range.index_count())
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
    if range.index_binding().is_none() {
        // checked 末端溢出必须在进入任一 Adapter 前拒绝。
        let vertex_end = range.checked_vertex_end().ok_or_else(|| {
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
    // 当前固定 pipeline 的完整 Uniform 身份已经由 typed bindings 保证存在。
    let uniform_buffer = buffers.uniform();
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
    // 当前 draw 的所有类型化资源事实与共享 pipeline 契约一致。
    Ok(())
}
