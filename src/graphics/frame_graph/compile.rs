//! Frame Graph — 编译子系统。
//!
//! 负责每帧开始前的帧图编译：
//!
//! 1. **自动裁剪（Culling）**：检查每个 Pass 的输入资源版本是否未变，
//!    且输出资源未被下层 Pass 消费，则彻底剔除该 Pass。
//! 2. **拓扑排序**：根据 reads / writes 依赖进行 DAG 排序。
//! 3. **屏障（Barrier）生成**：在读写冲突的 Pass 间插入同步屏障，
//!    确保软件渲染管线的正确顺序。

use std::collections::{HashMap, HashSet};

use super::pass::PassNode;
use super::resource::{PassId, ResourceId, Version};

// ════════════════════════════════════════════════════════════════════════════
// Barrier —— 同步屏障
// ════════════════════════════════════════════════════════════════════════════

/// 屏障类型：Pass 之间对同一资源读写冲突的语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarrierKind {
    /// 前一个 Pass 写入，后一个 Pass 读取（RAW — Read After Write）。
    ReadAfterWrite,
    /// 前一个 Pass 读取，后一个 Pass 写入（WAR — Write After Read）。
    WriteAfterRead,
    /// 前一个 Pass 写入，后一个 Pass 写入（WAW — Write After Write）。
    WriteAfterWrite,
}

/// 一个同步屏障，表示 `from_pass` 完成后才能开始 `to_pass`。
#[derive(Debug, Clone)]
pub struct Barrier {
    pub from_pass: PassId,
    pub to_pass: PassId,
    pub resource: ResourceId,
    pub kind: BarrierKind,
}

/// 对每个资源当前的访问状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccessState {
    /// 上次访问是读取（记录读取者）。
    ReadBy(PassId),
    /// 上次访问是写入（记录写入者）。
    WrittenBy(PassId),
}

// ════════════════════════════════════════════════════════════════════════════
// CompiledGraph —— 单帧编译结果
// ════════════════════════════════════════════════════════════════════════════

/// 编译一帧后生成的执行计划。
#[derive(Debug)]
pub struct CompiledGraph {
    /// 按执行顺序排列的 PassId（已剔除剪裁掉的 Pass）。
    pub execution_order: Vec<PassId>,
    /// 屏障列表（在 ordering 中隐含的同步约束）。
    pub barriers: Vec<Barrier>,
    /// 被裁剪掉的 Pass（仅用于调试 / 统计）。
    pub culled_passes: Vec<PassId>,
    /// 当前帧与上一帧相比是否有任何 Pass 执行。
    /// 为 `true` 时上层可以跳过 present 操作。
    pub zero_frame_cost: bool,
}

impl CompiledGraph {
    /// 空的编译结果（所有 Pass 被裁，无任何 GPU 指令）。
    pub fn empty() -> Self {
        Self {
            execution_order: Vec::new(),
            barriers: Vec::new(),
            culled_passes: Vec::new(),
            zero_frame_cost: true,
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 依赖图结构（编译内部使用）
// ════════════════════════════════════════════════════════════════════════════

/// 用于拓扑排序的边。
#[derive(Debug, Clone)]
struct Edge {
    from: PassId,
    to: PassId,
    reason: BarrierKind,
    resource: ResourceId,
}

// ════════════════════════════════════════════════════════════════════════════
// Compiler —— 帧图编译器
// ════════════════════════════════════════════════════════════════════════════

/// 帧图编译器。每次 `compile()` 产生一个 `CompiledGraph`。
pub struct Compiler {
    /// 记录每个 Pass 上次执行时其输入资源版本快照。
    last_input_versions: HashMap<(PassId, ResourceId), Version>,
    /// 记录每个资源上次被消费时其版本快照（用于输出未被消费检测）。
    last_output_versions: HashMap<(PassId, ResourceId), Version>,
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            last_input_versions: HashMap::new(),
            last_output_versions: HashMap::new(),
        }
    }

    /// 编译帧图。
    ///
    /// # 参数
    /// - `passes`：所有注册的 Pass 节点。
    /// - `current_versions`：当前帧开始时所有资源的版本快照。
    ///
    /// # 返回值
    /// `CompiledGraph`：执行计划。
    pub fn compile(
        &mut self,
        passes: &[PassNode],
        current_versions: &HashMap<ResourceId, Version>,
    ) -> CompiledGraph {
        // ── 阶段 1：自动裁剪 ──────────────────────────────────────

        // 构建资源 → 消费者映射：有哪些 Pass 读取了该资源。
        let mut consumers: HashMap<ResourceId, HashSet<PassId>> = HashMap::new();
        for pass in passes {
            for &res in &pass.reads {
                consumers.entry(res).or_default().insert(pass.id);
            }
        }

        // 构建资源 → 写入者映射。
        let mut writers: HashMap<ResourceId, PassId> = HashMap::new();
        for pass in passes {
            for &res in &pass.writes {
                writers.insert(res, pass.id);
            }
        }

        // 裁剪决策：对每个 Pass，检查输入是否未变 且 输出未被消费。
        let mut culled: HashSet<PassId> = HashSet::new();

        for pass in passes {
            let input_unchanged = pass.reads.iter().all(|&res| {
                // 无历史记录意味着该 Pass 从未见过此资源，视为已变更（强制首次执行）。
                let Some(&last) = self.last_input_versions.get(&(pass.id, res)) else {
                    return false;
                };
                let current = current_versions
                    .get(&res)
                    .copied()
                    .unwrap_or(Version::initial());
                last == current
            });

            let output_unconsumed = pass.writes.iter().all(|&res| {
                // 如果没有后续 Pass 读取该资源，则输出未被消费。
                !consumers.get(&res).map_or(false, |consumers| {
                    consumers
                        .iter()
                        .any(|&pid| pid != pass.id && !culled.contains(&pid))
                })
            });

            if input_unchanged && output_unconsumed {
                culled.insert(pass.id);
            }
        }

        // ── 阶段 2：对未裁剪的 Pass 建立依赖图 ───────────────────

        let alive_passes: Vec<&PassNode> =
            passes.iter().filter(|p| !culled.contains(&p.id)).collect();
        let alive_ids: HashSet<PassId> = alive_passes.iter().map(|p| p.id).collect();

        let mut edges: Vec<Edge> = Vec::new();

        // 扫描所有活跃 Pass 的资源访问冲突，生成依赖边。
        let mut resource_state: HashMap<ResourceId, AccessState> = HashMap::new();

        for pass in &alive_passes {
            // 读取冲突检测：如果前一个操作是写入，需要 RAW 屏障
            for &res in &pass.reads {
                match resource_state.get(&res) {
                    Some(&AccessState::WrittenBy(writer)) => {
                        edges.push(Edge {
                            from: writer,
                            to: pass.id,
                            reason: BarrierKind::ReadAfterWrite,
                            resource: res,
                        });
                    }
                    _ => {}
                }
                resource_state.insert(res, AccessState::ReadBy(pass.id));
            }

            // 写入冲突检测：RAW / WAW
            for &res in &pass.writes {
                match resource_state.get(&res) {
                    Some(&AccessState::WrittenBy(writer)) => {
                        edges.push(Edge {
                            from: writer,
                            to: pass.id,
                            reason: BarrierKind::WriteAfterWrite,
                            resource: res,
                        });
                    }
                    Some(&AccessState::ReadBy(reader)) => {
                        edges.push(Edge {
                            from: reader,
                            to: pass.id,
                            reason: BarrierKind::WriteAfterRead,
                            resource: res,
                        });
                    }
                    _ => {}
                }
                resource_state.insert(res, AccessState::WrittenBy(pass.id));
            }
        }

        // ── 阶段 3：拓扑排序（Kahn 算法） ─────────────────────────

        let order = match topological_sort(&alive_ids, &edges) {
            Some(order) => order,
            None => {
                // 出现循环依赖 —— 降级为按注册顺序执行所有活跃 Pass。
                log::warn!(
                    "FrameGraph: circular dependency detected, falling back to registration order"
                );
                alive_passes.iter().map(|p| p.id).collect()
            }
        };

        // ── 阶段 4：生成 Barrier 列表 ─────────────────────────────

        let barriers: Vec<Barrier> = edges
            .iter()
            .filter(|e| alive_ids.contains(&e.from) && alive_ids.contains(&e.to))
            .map(|e| Barrier {
                from_pass: e.from,
                to_pass: e.to,
                resource: e.resource,
                kind: e.reason,
            })
            .collect();

        // ── 阶段 5：更新版本快照 ──────────────────────────────────

        for pass in passes {
            for &res in &pass.reads {
                if let Some(&v) = current_versions.get(&res) {
                    self.last_input_versions.insert((pass.id, res), v);
                }
            }
            for &res in &pass.writes {
                if let Some(&v) = current_versions.get(&res) {
                    self.last_output_versions.insert((pass.id, res), v);
                }
            }
        }

        let culled_list: Vec<PassId> = culled.into_iter().collect();
        let zero_frame_cost = order.is_empty();

        CompiledGraph {
            execution_order: order,
            barriers,
            culled_passes: culled_list,
            zero_frame_cost,
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 拓扑排序（Kahn 算法）
// ════════════════════════════════════════════════════════════════════════════

/// 对 Pass DAG 进行拓扑排序。如果存在环则返回 `None`。
fn topological_sort(alive: &HashSet<PassId>, edges: &[Edge]) -> Option<Vec<PassId>> {
    // 仅保留两端都在 alive 中的边
    let relevant_edges: Vec<&Edge> = edges
        .iter()
        .filter(|e| alive.contains(&e.from) && alive.contains(&e.to))
        .collect();

    // 计算入度
    let mut in_degree: HashMap<PassId, usize> = HashMap::new();
    let mut adjacency: HashMap<PassId, Vec<PassId>> = HashMap::new();

    for &pid in alive {
        in_degree.entry(pid).or_insert(0);
        adjacency.entry(pid).or_default();
    }

    for e in &relevant_edges {
        adjacency.entry(e.from).or_default().push(e.to);
        *in_degree.entry(e.to).or_insert(0) += 1;
    }

    // Kahn
    let mut queue: Vec<PassId> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(&pid, _)| pid)
        .collect();

    let mut result = Vec::with_capacity(alive.len());

    while let Some(pid) = queue.pop() {
        result.push(pid);
        if let Some(neighbors) = adjacency.get(&pid) {
            for &nbr in neighbors {
                if let Some(deg) = in_degree.get_mut(&nbr) {
                    *deg = deg.saturating_sub(1);
                    if *deg == 0 {
                        queue.push(nbr);
                    }
                }
            }
        }
    }

    if result.len() == alive.len() {
        Some(result)
    } else {
        None // cycle detected
    }
}

#[cfg(test)]
mod tests {
    use super::super::pass::PassNode;
    use super::super::resource::{PassId, ResourceId, Version};
    use super::*;
    use crate::graphics::frame_graph::pass::PassContext;

    fn dummy_pass(
        id: PassId,
        name: &str,
        reads: Vec<ResourceId>,
        writes: Vec<ResourceId>,
    ) -> PassNode {
        let pass_name = String::from(name);
        PassNode::new(
            id,
            pass_name,
            reads,
            writes,
            Box::new(move |_ctx: &mut PassContext| Ok(())),
        )
    }

    fn make_versions(pairs: &[(ResourceId, Version)]) -> HashMap<ResourceId, Version> {
        pairs.iter().copied().collect()
    }

    #[test]
    fn no_history_forces_execution() {
        // 无历史记录时视为已变更 → 首次必须执行
        let mut compiler = Compiler::new();
        let res = ResourceId(0);

        let passes = vec![dummy_pass(PassId(0), "first_run", vec![res], vec![res])];
        let versions = make_versions(&[(res, Version::initial())]);

        let result = compiler.compile(&passes, &versions);

        // reads=[res] 无历史 → input_unchanged=false → 必须执行
        assert!(!result.zero_frame_cost);
        assert_eq!(result.execution_order.len(), 1);

        // 第二次编译：历史版本匹配当前 → 输入未变；输出无外部消费者 → 裁剪
        let result2 = compiler.compile(&passes, &versions);
        assert!(result2.zero_frame_cost);
        assert!(result2.execution_order.is_empty());
    }

    #[test]
    fn chain_passes() {
        // Pass0 写入 resA，Pass1 读取 resA → Pass1 依赖 Pass0
        let mut compiler = Compiler::new();
        let a = ResourceId(0);
        let b = ResourceId(1);

        let passes = vec![
            dummy_pass(PassId(0), "producer", vec![], vec![a]),
            dummy_pass(PassId(1), "consumer", vec![a], vec![b]),
        ];
        let versions = make_versions(&[(a, Version::initial()), (b, Version::initial())]);

        let result = compiler.compile(&passes, &versions);

        // 第一次编译：所有资源版本为初始值，输入版本快照为空，
        // 所以 input_unchanged = false（因为 last_input_versions 中没有记录）
        // 因此两个 Pass 都不会被裁剪。
        assert!(!result.zero_frame_cost);
        assert_eq!(result.execution_order.len(), 2);
        // 拓扑序：Pass0 → Pass1
        assert_eq!(result.execution_order[0], PassId(0));
        assert_eq!(result.execution_order[1], PassId(1));

        // 应该有 RAW 屏障 b/w Pass0 → Pass1
        let has_raw = result.barriers.iter().any(|b| {
            b.from_pass == PassId(0)
                && b.to_pass == PassId(1)
                && b.kind == BarrierKind::ReadAfterWrite
        });
        assert!(has_raw);

        // 第二次编译：版本不变 → 输入版本稳定
        // Pass0 输入无 reads，输出(a)被 Pass1 消费 → 不裁
        // Pass1 输入(a)未变，输出(b)无消费 → 裁
        let result2 = compiler.compile(&passes, &versions);
        assert_eq!(result2.execution_order.len(), 1);
        assert_eq!(result2.execution_order[0], PassId(0));
        assert!(result2.culled_passes.contains(&PassId(1)));
    }

    #[test]
    fn changed_input_triggers_execution() {
        // 如果版本变了，Pass 应执行
        let mut compiler = Compiler::new();
        let a = ResourceId(0);

        let passes = vec![dummy_pass(PassId(0), "dep", vec![a], vec![])];
        let versions = make_versions(&[(a, Version::initial())]);

        // 第一次编译：无历史记录 → 输入视为已变化 → 应执行
        let r1 = compiler.compile(&passes, &versions);
        assert!(!r1.zero_frame_cost);
        assert_eq!(r1.execution_order, vec![PassId(0)]);

        // 第二次编译：历史版本 = current → 未变，且无输出消费 → 裁剪
        let r2 = compiler.compile(&passes, &versions);
        assert!(r2.zero_frame_cost);
        assert!(r2.execution_order.is_empty());

        // 模拟外部变更资源版本
        let mut bumped = versions.clone();
        bumped.insert(a, Version::initial().advance());
        let r3 = compiler.compile(&passes, &bumped);
        assert!(!r3.zero_frame_cost);
        assert_eq!(r3.execution_order, vec![PassId(0)]);
    }

    #[test]
    fn topological_order_complex() {
        // 测试更复杂的 DAG
        let mut compiler = Compiler::new();
        let a = ResourceId(0);
        let b = ResourceId(1);
        let c = ResourceId(2);

        let passes = vec![
            // Pass1 writes a
            dummy_pass(PassId(1), "pass1", vec![], vec![a]),
            // Pass2 writes b
            dummy_pass(PassId(2), "pass2", vec![], vec![b]),
            // Pass3 reads a,b writes c
            dummy_pass(PassId(3), "pass3", vec![a, b], vec![c]),
        ];
        let mut versions = HashMap::new();
        versions.insert(a, Version::initial());
        versions.insert(b, Version::initial());
        versions.insert(c, Version::initial());

        let result = compiler.compile(&passes, &versions);

        // Pass1, Pass2 可以并行（无依赖），Pass3 依赖两者
        // 所以结果中 Pass3 必须在最后
        assert!(!result.execution_order.is_empty());
        if let Some(pos3) = result.execution_order.iter().position(|&p| p == PassId(3)) {
            assert!(
                result.execution_order[..pos3].contains(&PassId(1)),
                "Pass3 should be after Pass1"
            );
            assert!(
                result.execution_order[..pos3].contains(&PassId(2)),
                "Pass3 should be after Pass2"
            );
        } else {
            panic!("Pass3 should be in execution order");
        }
    }
}
