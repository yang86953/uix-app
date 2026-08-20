// 交错入场辅助方法归属于 ViewAdapter，避免主实现文件超过规模上限。
impl crate::ui::adapter::ViewAdapter {
    // 按子节点序号计算交错动画的安全截止时间。
    pub(super) fn stagger_deadline(
        // 接收本轮展开共用的时间锚点。
        anchor: std::time::Instant,
        // 接收相邻子节点的动画间隔秒数。
        interval_secs: f64,
        // 接收当前子节点在同级中的序号。
        rank: usize,
        // 返回可用于延迟入场的截止时间。
    ) -> Option<std::time::Instant> {
        // 首个节点或非正间隔不需要延迟。
        if rank == 0 || interval_secs <= 0.0 {
            // 明确返回无延迟。
            return None;
        }
        // 将浮点间隔转换为不会溢出的时长。
        let delay = std::time::Duration::try_from_secs_f64(interval_secs * rank as f64)
            // 无法表示时回退为最大时长。
            .unwrap_or(std::time::Duration::MAX);
        // 先尝试直接相加，溢出时逐步缩短到可表示的时刻。
        anchor.checked_add(delay).or_else(|| {
            // 保存可缩短的候选时长。
            let mut candidate = delay;
            // 继续寻找仍在时间范围内的延迟。
            loop {
                // 对半缩短候选时长。
                candidate /= 2;
                // 找到可表示时间后结束搜索。
                if let Some(deadline) = anchor.checked_add(candidate) {
                    // 返回安全的截止时间。
                    break Some(deadline);
                }
            }
        })
    }

    // 将父节点声明的交错配置写入一个子 ViewNode。
    pub(super) fn configure_staggered_child(
        // 接收需要更新的声明子节点。
        child: &mut crate::ui::view::ViewNode,
        // 接收可选的交错动画配置。
        stagger: Option<(f64, crate::ui::animation::AnimationConfig)>,
        // 接收本轮展开共用的时间锚点。
        anchor: std::time::Instant,
        // 接收当前子节点在同级中的序号。
        rank: usize,
    ) {
        // 缺少交错配置时保留子节点原状。
        let Some((interval_secs, animation)) = stagger else {
            // 结束无需变更的路径。
            return;
        };
        // 继承声明的入场动画。
        child.enter_animation = Some(animation);
        // 写入按序号计算的入场截止时间。
        child.enter_deadline = Self::stagger_deadline(anchor, interval_secs, rank);
    }
}
