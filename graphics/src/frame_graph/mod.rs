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

use uix_platform::Result;

// ════════════════════════════════════════════════════════════════════════════
// FrameGraph —— 帧图主结构
// ════════════════════════════════════════════════════════════════════════════

/// 帧图 —— 渲染管线的声明式编排器。
///
/// 持有资源注册表、Pass 节点列表和编译器状态。
/// 每次渲染循环调用 `compile()` → `execute()`。
pub struct FrameGraph {
    registry: ResourceRegistry,
    passes: Vec<PassNode>,
    pass_index: HashMap<PassId, usize>,
    compiler: Compiler,
    /// 当前帧开始时的资源版本快照（由外部更新）。
    current_versions: HashMap<ResourceId, Version>,
    /// 帧级资源数据存储。
    resources: FrameResources,
}

impl FrameGraph {
    pub fn new() -> Self {
        Self {
            registry: ResourceRegistry::new(),
            passes: Vec::new(),
            pass_index: HashMap::new(),
            compiler: Compiler::new(),
            current_versions: HashMap::new(),
            resources: FrameResources::new(),
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

    /// 添加一个 Pass 到帧图。返回该 Pass 的 PassId。
    ///
    /// `build` 参数接收一个 `PassBuilder`，通过链式调用配置依赖和函数。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let pid = fg.add_pass("Clear", |b| {
    ///     b.writes(&[color]).execute_with(Box::new(|ctx| {
    ///         ctx.engine.begin_frame(&DirtyRegion::full());
    ///         Ok(())
    ///     }))
    /// });
    /// ```
    pub fn add_pass<F>(&mut self, name: &str, build: F) -> PassId
    where
        F: FnOnce(PassBuilder) -> PassBuilder,
    {
        let id = self.registry.allocate_pass_id();
        let builder = PassBuilder::new(id, name);
        let node = build(builder).build();
        self.pass_index.insert(id, self.passes.len());
        self.passes.push(node);
        id
    }

    /// 显式添加一个已构建的 PassNode。
    pub fn add_pass_node(&mut self, node: PassNode) {
        self.pass_index.insert(node.id, self.passes.len());
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
    /// 返回 `CompiledGraph`，传递给 `execute()` / `execute_with()` 使用。
    pub fn compile(&mut self) -> CompiledGraph {
        self.compiler.compile(&self.passes, &self.current_versions)
    }

    /// 执行编译后的帧图（使用 Pass 内建执行函数）。
    ///
    /// 按 `plan.execution_order` 顺序执行每个 Pass 的 `execute` 闭包。
    /// 如果 Pass 的 execute 为 None 则直接跳过（由 `execute_with` 处理）。
    ///
    /// # 参数
    /// - `engine`：实现了 `GraphicsEngine` 的低级渲染引擎。
    /// - `plan`：由 `compile()` 生成的执行计划。
    pub fn execute(
        &mut self,
        engine: &mut dyn crate::GraphicsEngine,
        plan: &CompiledGraph,
    ) -> Result<()> {
        for &pid in &plan.execution_order {
            let idx = self.pass_index.get(&pid).copied().ok_or_else(|| {
                uix_platform::Error::new(
                    uix_platform::Errc::InvalidState,
                    format!("FrameGraph::execute: unknown PassId {:?}", pid),
                )
            })?;
            let pass = &mut self.passes[idx];

            // 构造 Pass 上下文
            let mut ctx = PassContext {
                engine,
                resources: &mut self.resources,
                dirty_rect: None,
            };

            // 执行（跳过 None —— 配合 execute_with 使用）
            if let Some(ref mut exec) = pass.execute {
                (exec)(&mut ctx)?;
            }

            // 写入资源版本提升
            for &res in &pass.writes {
                let new_ver = self.registry.bump_version(res, pass.id);
                self.current_versions.insert(res, new_ver);
            }
        }

        Ok(())
    }

    /// 执行编译后的帧图（使用外部渲染回调）。
    ///
    /// 与 `execute()` 不同，此方法忽略 Pass 内建的 `execute` 闭包，
    /// 改为对每个执行的 Pass 调用 `render_fn` 回调。
    /// 回调可以捕获非 'static 引用（如 `&mut WidgetTree`），
    /// 不受 `PassFn` 的 `'static` 限制。
    ///
    /// # 参数
    /// - `engine`：低级渲染引擎。
    /// - `plan`：由 `compile()` 生成的执行计划。
    /// - `render_fn`：外部渲染回调，接收 (PassId, engine, frame_resources)。
    pub fn execute_with(
        &mut self,
        engine: &mut dyn crate::GraphicsEngine,
        plan: &CompiledGraph,
        render_fn: &mut dyn FnMut(PassId, &mut dyn crate::GraphicsEngine, &mut FrameResources) -> Result<()>,
    ) -> Result<()> {
        for &pid in &plan.execution_order {
            // 调用外部渲染回调（取代 PassNode::execute）
            render_fn(pid, engine, &mut self.resources)?;

            // 写入资源版本提升
            if let Some(&idx) = self.pass_index.get(&pid) {
                for &res in &self.passes[idx].writes {
                    let new_ver = self.registry.bump_version(res, pid);
                    self.current_versions.insert(res, new_ver);
                }
            }
        }

        Ok(())
    }

    /// 编译并执行 —— 便捷的一次性调用。
    pub fn compile_and_execute(
        &mut self,
        engine: &mut dyn crate::GraphicsEngine,
    ) -> Result<CompiledGraph> {
        let plan = self.compile();
        self.execute(engine, &plan)?;
        Ok(plan)
    }

    /// 清空 Pass 列表（保留资源注册表和版本历史）。
    pub fn clear_passes(&mut self) {
        self.passes.clear();
        self.pass_index.clear();
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
                    .begin_frame(crate::traits::UpdateStrategy::FullRedraw);
                ctx.engine.end_frame();
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
