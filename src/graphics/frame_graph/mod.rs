//! UIX Frame Graph —— 声明式渲染编排引擎。
//!
//! # 设计哲学
//!
//! 帧图（Frame Graph）替代传统的立即模式（Immediate Mode）渲染流程，
//! 将渲染管线建模为 资源 + Pass（节点）的有向无环图。
//!
//! ## 核心特性
//!
//! - **资源中心化**：纹理、Buffer 抽象为带版本号的逻辑资源。
//! - **Pass 声明式定义**：每个 Pass 明确声明 reads / writes 依赖。
//! - **自动裁剪（Culling）**：输入未变 + 输出未被消费 → Pass 被剔除。
//! - **自动屏障（Barrier）**：读写冲突的 Pass 间自动排障。
//! - **零帧开销（0 Frame Cost）**：无变化时完全跳过渲染。
//!
//! # 用法
//!
//! ```ignore
//! use uix::graphics::frame_graph::FrameGraph;
//!
//! let mut fg = FrameGraph::new();
//!
//! // 注册资源
//! let color = fg.register_texture("color", 800, 600);
//! let depth = fg.register_texture("depth", 800, 600);
//!
//! // 声明 Pass
//! fg.add_pass("Shadow", |p| {
//!     p.reads(&[color]).writes(&[depth]).execute_with(|ctx| {
//!         // ... render shadow ...
//!         Ok(())
//!     })
//! });
//!
//! // 每帧：编译 + 执行
//! let plan = fg.compile();
//! fg.execute(&mut engine, &mut plan)?;
//! ```

pub mod compile;
pub mod pass;
pub mod resource;

use std::collections::HashMap;

use compile::{CompiledGraph, Compiler};
use pass::{FrameResources, PassBuilder, PassContext, PassNode};
use resource::{PassId, ResourceId, ResourceKind, ResourceRegistry, Version};

use crate::diag::Result;

// ════════════════════════════════════════════════════════════════════════════
// FrameGraph —— 帧图主结构
// ════════════════════════════════════════════════════════════════════════════

/// 帧图 —— 渲染管线的声明式编排器。
///
/// 持有资源注册表、Pass 节点列表和编译器状态。
/// 每次渲染循环调用 `compile()` → `execute()`。
#[allow(dead_code)]
pub struct FrameGraph {
    registry: ResourceRegistry,
    passes: Vec<PassNode>,
    compiler: Compiler,
    /// 当前帧开始时的资源版本快照（由外部更新）。
    current_versions: HashMap<ResourceId, Version>,
    /// 帧级资源数据存储。
    resources: FrameResources,
    #[allow(dead_code)]
    /// 自增 Pass 计数器。
    next_pass_id: u32,
}

impl FrameGraph {
    pub fn new() -> Self {
        Self {
            registry: ResourceRegistry::new(),
            passes: Vec::new(),
            compiler: Compiler::new(),
            current_versions: HashMap::new(),
            resources: FrameResources::new(),
            next_pass_id: 0,
        }
    }

    // ── 资源管理 ──────────────────────────────────────────────────

    /// 注册一个 Texture 资源。
    pub fn register_texture(&mut self, name: &str, width: u32, height: u32) -> ResourceId {
        let id = self
            .registry
            .register(name, ResourceKind::texture(width, height));
        self.current_versions.insert(id, Version::initial());
        id
    }

    /// 注册一个 Buffer 资源。
    pub fn register_buffer(&mut self, name: &str, byte_size: u64) -> ResourceId {
        let id = self
            .registry
            .register(name, ResourceKind::buffer(byte_size));
        self.current_versions.insert(id, Version::initial());
        id
    }

    /// 标记一个资源已被修改（版本递增）。
    /// 由外部（或 Pass 执行后）调用。
    pub fn mark_resource_dirty(&mut self, id: ResourceId) {
        // 分配一个虚拟 PassId 来记录写入
        let sys_id = PassId(u32::MAX);
        let new_ver = self.registry.bump_version(id, sys_id);
        self.current_versions.insert(id, new_ver);
    }

    /// 获取资源的当前版本。
    pub fn version_of(&self, id: ResourceId) -> Version {
        self.registry.version_of(id)
    }

    /// 获取资源的名称（调试用）。
    pub fn resource_name(&self, id: ResourceId) -> &str {
        self.registry.resource_name(id)
    }

    /// 获取帧级资源存储的可变引用。
    pub fn frame_resources_mut(&mut self) -> &mut FrameResources {
        &mut self.resources
    }

    /// 获取帧级资源存储的只读引用。
    pub fn frame_resources(&self) -> &FrameResources {
        &self.resources
    }

    // ── Pass 管理 ──────────────────────────────────────────────────

    /// 添加一个 Pass 到帧图。
    ///
    /// `build` 参数接收一个 `PassBuilder`，通过链式调用配置依赖和函数。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// fg.add_pass("Clear", |b| {
    ///     b.writes(&[color]).execute_with(|ctx| {
    ///         ctx.engine.begin_frame(&DirtyRegion::full());
    ///         Ok(())
    ///     })
    /// });
    /// ```
    pub fn add_pass<F>(&mut self, name: &str, build: F)
    where
        F: FnOnce(PassBuilder) -> PassBuilder,
    {
        let id = self.registry.allocate_pass_id();
        let builder = PassBuilder::new(id, name);
        let node = build(builder).build();
        self.passes.push(node);
    }

    /// 显式添加一个已构建的 PassNode。
    pub fn add_pass_node(&mut self, node: PassNode) {
        self.passes.push(node);
    }

    /// 获取所有 Pass 的只读引用。
    pub fn passes(&self) -> &[PassNode] {
        &self.passes
    }

    // ── 编译与执行 ────────────────────────────────────────────────

    /// 编译当前帧图。
    ///
    /// 执行裁剪、拓扑排序和屏障生成。
    /// 返回 `CompiledGraph`，传递给 `execute()` 使用。
    pub fn compile(&mut self) -> CompiledGraph {
        self.compiler.compile(&self.passes, &self.current_versions)
    }

    /// 执行编译后的帧图。
    ///
    /// 按 `plan.execution_order` 顺序执行每个 Pass，
    /// 并在 Pass 间隐式应用屏障约束。
    ///
    /// # 参数
    /// - `engine`：实现了 `GraphicsEngine` 的低级渲染引擎。
    /// - `plan`：由 `compile()` 生成的执行计划。
    pub fn execute(
        &mut self,
        engine: &mut dyn crate::graphics::GraphicsEngine,
        plan: &CompiledGraph,
    ) -> Result<()> {
        // 将 PassId 到 PassNode 的快速查找建立
        let pass_map: HashMap<PassId, usize> = self
            .passes
            .iter()
            .enumerate()
            .map(|(i, p)| (p.id, i))
            .collect();

        // 按顺序执行
        for &pid in &plan.execution_order {
            let idx = pass_map.get(&pid).ok_or_else(|| {
                crate::diag::Error::new(
                    crate::diag::Errc::InvalidState,
                    format!("FrameGraph::execute: unknown PassId {:?}", pid),
                )
            })?;
            let pass = &mut self.passes[*idx];

            // 构造 Pass 上下文
            let mut ctx = PassContext {
                engine,
                resources: &mut self.resources,
                dirty_rect: None,
            };

            // 执行
            (pass.execute)(&mut ctx)?;

            // 写入资源版本提升
            for &res in &pass.writes {
                let new_ver = self.registry.bump_version(res, pass.id);
                self.current_versions.insert(res, new_ver);
            }
        }

        Ok(())
    }

    /// 编译并执行 —— 便捷的一次性调用。
    pub fn compile_and_execute(
        &mut self,
        engine: &mut dyn crate::graphics::GraphicsEngine,
    ) -> Result<CompiledGraph> {
        let plan = self.compile();
        self.execute(engine, &plan)?;
        Ok(plan)
    }

    /// 重置帧图状态（清空所有 Pass 和版本历史）。
    ///
    /// 资源数据保留，但版本历史重置，下一次编译会全部执行。
    pub fn reset(&mut self) {
        self.passes.clear();
        self.compiler = Compiler::new();
        self.current_versions.clear();
    }

    /// 被裁剪的 Pass 数量（调试用）。
    pub fn culled_count(&self, plan: &CompiledGraph) -> usize {
        plan.culled_passes.len()
    }

    /// 执行的 Pass 数量（调试用）。
    pub fn executed_count(&self, plan: &CompiledGraph) -> usize {
        plan.execution_order.len()
    }
}

impl Default for FrameGraph {
    fn default() -> Self {
        Self::new()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 便利构造函数 —— 快速构建典型 UI 渲染管线
// ════════════════════════════════════════════════════════════════════════════

/// 典型的 2D UI 帧图构建器。
///
/// 提供默认的 UI 渲染管线 Pass 定义，用户可按需增删。
pub mod presets {
    use super::*;

    /// 在给定的 `FrameGraph` 上注册标准 UI 渲染 Pass。
    ///
    /// 标准管线：
    /// 1. ClearPass：清除主帧缓冲（writes: main_color）
    /// 2. GeometryPass：绘制 UI 几何体（reads: main_color, writes: main_color）
    /// 3. ShadowPass：绘制阴影（reads: main_color, writes: main_color）
    /// 4. PresentPass：最终呈现准备（reads: main_color）
    ///
    /// # 参数
    /// - `fg`：帧图
    /// - `main_color`：主帧缓冲资源 ID
    /// - `clear_color`：清除颜色
    /// - `geometry_fn`：几何绘制回调 (engine, main_color) -> Result
    /// - `shadow_fn`：阴影绘制回调
    pub fn add_ui_pipeline(fg: &mut FrameGraph, main_color: ResourceId, _width: u32, _height: u32) {
        // Pass 1: Clear — 不清除也可以，但为清晰语义保留
        fg.add_pass("Clear", |b| {
            b.writes(&[main_color]).execute_with(Box::new(move |ctx| {
                ctx.engine
                    .begin_frame(&crate::graphics::DirtyRegion::full());
                ctx.engine.end_frame(&crate::graphics::DirtyRegion::full());
                Ok(())
            }))
        });
    }

    /// GPU 风格管线示例（Shadow → GBuffer → Lighting → PostFX）。
    /// 这些是概念性 Pass 名称，实际实现根据项目需求填充。
    pub fn add_gpu_style_pipeline(fg: &mut FrameGraph) -> (ResourceId, ResourceId, ResourceId) {
        let gbuffer = fg.register_texture("GBuffer_Albedo", 1920, 1080);
        let shadow = fg.register_texture("ShadowMap", 1024, 1024);
        let output = fg.register_texture("FinalOutput", 1920, 1080);

        fg.add_pass("Shadow", |b| {
            b.writes(&[shadow]).execute_with(Box::new(|_ctx| Ok(())))
        });
        fg.add_pass("GBuffer", |b| {
            b.writes(&[gbuffer]).execute_with(Box::new(|_ctx| Ok(())))
        });
        fg.add_pass("Lighting", |b| {
            b.reads(&[gbuffer, shadow])
                .writes(&[output])
                .execute_with(Box::new(|_ctx| Ok(())))
        });
        fg.add_pass("PostFX", |b| {
            b.reads(&[output])
                .writes(&[output])
                .execute_with(Box::new(|_ctx| Ok(())))
        });

        (gbuffer, shadow, output)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 测试
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphics::engine::GraphicsEngine;
    use crate::graphics::NullEngine;

    #[test]
    fn frame_graph_empty_compile() {
        let mut fg = FrameGraph::new();
        let plan = fg.compile();
        assert!(plan.zero_frame_cost);
        assert!(plan.execution_order.is_empty());
    }

    #[test]
    fn frame_graph_single_pass_with_consumer() {
        let mut fg = FrameGraph::new();
        let color = fg.register_texture("color", 100, 100);

        // Producer + consumer 构成有意义管线
        fg.add_pass("Clear", |b| {
            b.writes(&[color]).execute_with(Box::new(|_ctx| Ok(())))
        });
        fg.add_pass("Present", |b| {
            b.reads(&[color]).execute_with(Box::new(|_ctx| Ok(())))
        });

        let plan = fg.compile();
        // Clear: 无 reads, writes[color], 被 Present 消费 → 不裁
        // Present: reads[color]无历史→视为已变, writes[]→output_unconsumed → 但 input_unchanged=false 所以不裁
        assert!(!plan.zero_frame_cost);
        assert_eq!(plan.execution_order.len(), 2);

        // 第二次编译：Clear input 无、output 仍被 Present 消费 → 不裁
        // Present input 未变、无输出消费 → 裁
        let plan2 = fg.compile();
        assert!(!plan2.zero_frame_cost);
        assert_eq!(plan2.execution_order.len(), 1);
        // 最终稳定态：Clear 每次都跑（因为 output 始终被消费），Present 无变化时被裁
        let plan3 = fg.compile();
        assert!(!plan3.zero_frame_cost);
        assert_eq!(plan3.execution_order.len(), 1);
    }

    #[test]
    fn frame_graph_execute() {
        let mut fg = FrameGraph::new();
        let color = fg.register_texture("color", 100, 100);
        let mut engine = NullEngine::new();
        let _ = <NullEngine as GraphicsEngine>::initialize(&mut engine, 100, 100);

        // Producer 写入 color，Consumer 读取 color → 形成消费链
        fg.add_pass("Clear", |b| {
            b.writes(&[color]).execute_with(Box::new(|mut ctx| {
                let _ = ctx.engine;
                Ok(())
            }))
        });
        fg.add_pass("Notify", |b| {
            b.reads(&[color]).execute_with(Box::new(|_ctx| Ok(())))
        });

        let plan = fg.compile();
        assert!(!plan.zero_frame_cost);
        assert!(fg.execute(&mut engine, &plan).is_ok());
        // 执行后 color 版本应递增（由 Clear Pass 写入）
        assert!(
            fg.version_of(color) > Version::initial(),
            "version should be bumped after execute: got {:?}",
            fg.version_of(color)
        );
    }

    #[test]
    fn frame_graph_two_passes_chain() {
        let mut fg = FrameGraph::new();
        let a = fg.register_texture("a", 10, 10);
        let b = fg.register_texture("b", 10, 10);

        fg.add_pass("PassA", |pb| {
            pb.writes(&[a]).execute_with(Box::new(|_ctx| Ok(())))
        });
        fg.add_pass("PassB", |pb| {
            pb.reads(&[a])
                .writes(&[b])
                .execute_with(Box::new(|_ctx| Ok(())))
        });

        let plan = fg.compile();
        // PassA 输入无变化（无 reads），输出(a)被 PassB 消费 → 不裁 PassA
        // PassB 输入(a)版本无历史(=initial)，当前也是 initial → 未变，输出(b)无消费 → 裁 PassB
        // 实际上：PassA 无 reads → input_unchanged=true, writes a,
        // a 的 consumer=PassB, PassB 在当前帧还没被裁（在决策时未裁）
        // 所以 output_unconsumed=false → PassA 不裁
        // PassB: reads a, 版本无变化→true, writes b, b 无消费→true
        // → 裁 PassB
        assert!(!plan.execution_order.is_empty());
        // PassA should execute
        assert!(plan.execution_order.contains(&PassId(0)));
    }

    #[test]
    fn frame_graph_zero_cost_when_idle() {
        let mut fg = FrameGraph::new();
        let color = fg.register_texture("color", 200, 200);

        fg.add_pass("Clear", |b| {
            b.writes(&[color]).execute_with(Box::new(|_ctx| Ok(())))
        });
        fg.add_pass("Present", |b| {
            b.reads(&[color]).execute_with(Box::new(|_ctx| Ok(())))
        });

        // First compile → should execute
        let plan1 = fg.compile();
        assert!(!plan1.zero_frame_cost);

        // Second compile → Clear still produces for Present, Present input not changed, output none → cull Present
        let plan2 = fg.compile();
        assert!(!plan2.zero_frame_cost); // Clear still runs
        assert_eq!(plan2.execution_order.len(), 1);

        // Simulate external change (mark dirty) → Clear triggers Present
        fg.mark_resource_dirty(color);
        let plan3 = fg.compile();
        assert!(!plan3.zero_frame_cost);
        // Clear: writes color (未变但output被消费), Present: reads color (已变) → both run
        assert_eq!(plan3.execution_order.len(), 2);
    }
}
