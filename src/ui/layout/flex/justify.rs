// 导入 Flex 共享枚举与基础类型。
use super::*;

/// 计算主轴剩余空间对应的有效间距与起始偏移，供两条 Flex 路径共享。
pub(in crate::ui::layout) fn compute_justify(
    // 接收扣除子项、边距与基础间距后的主轴差值。
    remaining: f32,
    // 接收当前行内的子项数量。
    count: usize,
    // 接收声明的基础间距。
    gap: f32,
    // 接收当前主轴分布模式。
    justify: JustifyContent,
    // 返回最终间距与行起始偏移。
) -> (f32, f32) {
    // 正剩余空间允许分散型模式扩大间距。
    if remaining > 0.0 {
        // 根据分散型模式解析最终行内间距。
        let new_gap = match justify {
            // SpaceBetween 只在至少两个子项时分配中间空间。
            JustifyContent::SpaceBetween => {
                // 单项行没有可分配的中间槽位。
                if count <= 1 {
                    // 保留声明的基础间距。
                    gap
                // 多项行把剩余空间均分到相邻槽位。
                } else {
                    // 在基础间距上追加均分结果。
                    gap + remaining / (count - 1) as f32
                }
            }
            // SpaceAround 为每个子项分配一份环绕空间。
            JustifyContent::SpaceAround => gap + remaining / count as f32,
            // SpaceEvenly 同时为两端与相邻项分配等量空间。
            JustifyContent::SpaceEvenly => gap + remaining / (count + 1) as f32,
            // 其他模式不改写基础间距。
            _ => gap,
        };
        // 根据对齐模式解析行起始偏移。
        let offset = match justify {
            // Center 消费一半正剩余空间。
            JustifyContent::Center => remaining * 0.5,
            // End 消费全部正剩余空间。
            JustifyContent::End => remaining,
            // SpaceAround 在起点保留半份单项环绕空间。
            JustifyContent::SpaceAround => remaining * 0.5 / count as f32,
            // SpaceEvenly 在起点保留一份均分空间。
            JustifyContent::SpaceEvenly => remaining / (count + 1) as f32,
            // Start、SpaceBetween 与 Stretch 从自然起点开始。
            _ => 0.0,
        };
        // 返回正剩余空间下的间距与偏移。
        (new_gap, offset)
    // 非正剩余空间禁止分散模式反向压缩声明间距。
    } else {
        // 只有 Center 与 End 保留负差值的溢出对齐语义。
        let offset = match justify {
            // Center 向主轴起点外溢一半差值。
            JustifyContent::Center => remaining * 0.5,
            // End 向主轴起点外溢全部差值。
            JustifyContent::End => remaining,
            // 其他模式保持自然起点。
            _ => 0.0,
        };
        // 返回基础间距与负空间偏移。
        (gap, offset)
    }
}
