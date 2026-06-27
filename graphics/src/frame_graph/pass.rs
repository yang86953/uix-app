//! Frame Graph — Pass 声明式定义子系统。
//!
//! 每个 Pass 节点明确声明其 `reads` 和 `writes` 的资源依赖，
//! 并持有可执行的渲染函数。编译器据此进行拓扑排序与自动裁剪。

use super::resource::{PassId, ResourceId};
use uix_platform::Result;

use super::super::GraphicsEngine;

// ════════════════════════════════════════════════════════════════════════════
// PassContext —— Pass 执行时的上下文
// ════════════════════════════════════════════════════════════════════════════

/// Pass 执行时获得的渲染上下文。
///
/// - `engine`：低级 2D 渲染引擎（SoftwareEngine / NullEngine）。
/// - `resources`：帧级资源数据存储（实际像素缓冲）。
/// - `dirty_rect`：该 Pass 负责的脏区域（由上层 UI 树计算）。
pub struct PassContext<'a> {
    pub engine: &'a mut dyn GraphicsEngine,
    pub resources: &'a mut FrameResources,
    pub dirty_rect: Option<uix_platform::Rect>,
}

// ════════════════════════════════════════════════════════════════════════════
// FrameResources —— 帧级资源数据存储
// ════════════════════════════════════════════════════════════════════════════

/// 帧级存储，持有每个逻辑资源对应的实际数据。
///
/// Texture 存储为 `Vec<u32>`（BGRA 预乘像素），
/// Buffer 存储为 `Vec<u8>`（原始字节）。
#[derive(Default)]
pub struct FrameResources {
    textures: std::collections::HashMap<ResourceId, Vec<u32>>,
    texture_meta: std::collections::HashMap<ResourceId, (i32, i32)>,
    buffers: std::collections::HashMap<ResourceId, Vec<u8>>,
}

impl FrameResources {
    pub fn new() -> Self {
        Self::default()
    }

    /// 分配或替换 Texture 数据。返回可变引用供写入。
    pub fn allocate_texture(
        &mut self,
        id: ResourceId,
        width: i32,
        height: i32,
        fill: u32,
    ) -> &mut [u32] {
        let len = (width * height) as usize;
        let vec = self.textures.entry(id).or_insert_with(|| vec![fill; len]);
        vec.resize(len, fill);
        self.texture_meta.insert(id, (width, height));
        vec.as_mut_slice()
    }

    /// 获取 Texture 像素切片（只读）。
    pub fn texture_data(&self, id: ResourceId) -> Option<&[u32]> {
        self.textures.get(&id).map(|v| v.as_slice())
    }

    /// 获取 Texture 尺寸。
    pub fn texture_size(&self, id: ResourceId) -> Option<(i32, i32)> {
        self.texture_meta.get(&id).copied()
    }

    /// 获取 Texture 像素切片（可变，用于写入）。
    pub fn texture_data_mut(&mut self, id: ResourceId) -> Option<&mut [u32]> {
        self.textures.get_mut(&id).map(|v| v.as_mut_slice())
    }

    /// 分配或替换 Buffer 数据。返回可变引用。
    pub fn allocate_buffer(&mut self, id: ResourceId, size: usize, fill: u8) -> &mut [u8] {
        let buf = self.buffers.entry(id).or_insert_with(|| vec![fill; size]);
        buf.resize(size, fill);
        buf.as_mut_slice()
    }

    /// 获取 Buffer 数据（只读）。
    pub fn buffer_data(&self, id: ResourceId) -> Option<&[u8]> {
        self.buffers.get(&id).map(|v| v.as_slice())
    }

    /// 获取 Buffer 数据（可变）。
    pub fn buffer_data_mut(&mut self, id: ResourceId) -> Option<&mut [u8]> {
        self.buffers.get_mut(&id).map(|v| v.as_mut_slice())
    }
}

// ════════════════════════════════════════════════════════════════════════════
// PassFn —— Pass 执行函数签名
// ════════════════════════════════════════════════════════════════════════════

/// Pass 执行函数的类型别名。
pub type PassFn = Box<dyn FnMut(&mut PassContext) -> Result<()>>;

/// 外部渲染回调 —— 用于 execute_with()，在编译后由调用者提供渲染函数。
/// 避免了 PassFn 的 'static 要求，可以捕获非 'static 引用。
pub type RenderCallback<'a> =
    dyn FnMut(PassId, &mut dyn GraphicsEngine, &mut FrameResources) -> Result<()> + 'a;

// ════════════════════════════════════════════════════════════════════════════
// PassNode —— 帧图中的 Pass 节点
// ════════════════════════════════════════════════════════════════════════════

/// 帧图中的单个 Pass 节点。
///
/// 声明式定义：`name` 用于调试，`reads` / `writes` 声明资源依赖，
/// `execute` 是渲染函数的载体（可选 —— 使用 execute_with 时可留空）。
pub struct PassNode {
    pub id: PassId,
    pub name: String,
    pub reads: Vec<ResourceId>,
    pub writes: Vec<ResourceId>,
    /// 内部执行函数。为 `None` 时表示该 Pass 由 `execute_with` 的外部回调驱动。
    pub execute: Option<PassFn>,
}

impl PassNode {
    /// 创建一个带执行函数的 Pass。
    pub fn new(
        id: PassId,
        name: impl Into<String>,
        reads: Vec<ResourceId>,
        writes: Vec<ResourceId>,
        execute: PassFn,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            reads,
            writes,
            execute: Some(execute),
        }
    }

    /// 创建一个仅声明依赖的 Pass（无执行函数，配合 `execute_with` 使用）。
    pub fn new_structural(
        id: PassId,
        name: impl Into<String>,
        reads: Vec<ResourceId>,
        writes: Vec<ResourceId>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            reads,
            writes,
            execute: None,
        }
    }

    /// 该 Pass 是否写入了指定资源。
    pub fn writes_to(&self, res: ResourceId) -> bool {
        self.writes.contains(&res)
    }

    /// 该 Pass 是否读取了指定资源。
    pub fn reads_from(&self, res: ResourceId) -> bool {
        self.reads.contains(&res)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// PassBuilder —— Pass 构造器（用于 FrameGraph::add_pass）
// ════════════════════════════════════════════════════════════════════════════

/// 链式构造 Pass 的构建器。
pub struct PassBuilder {
    pub(crate) id: PassId,
    pub(crate) name: String,
    pub(crate) reads: Vec<ResourceId>,
    pub(crate) writes: Vec<ResourceId>,
    pub(crate) execute: Option<PassFn>,
}

impl PassBuilder {
    pub fn new(id: PassId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            reads: Vec::new(),
            writes: Vec::new(),
            execute: None,
        }
    }

    /// 声明 Pass 读取的资源。
    pub fn reads(mut self, resources: &[ResourceId]) -> Self {
        self.reads.extend_from_slice(resources);
        self
    }

    /// 声明 Pass 写入的资源。
    pub fn writes(mut self, resources: &[ResourceId]) -> Self {
        self.writes.extend_from_slice(resources);
        self
    }

    /// 设置 Pass 的执行函数。
    pub fn execute_with(mut self, f: PassFn) -> Self {
        self.execute = Some(f);
        self
    }

    /// 构建 PassNode。
    ///
    /// 如果未设置 `execute_with()`，则 `execute` 字段为 `None`，
    /// 该 Pass 仅声明依赖（供 `execute_with` 外部回调使用）。
    pub fn build(self) -> PassNode {
        PassNode {
            id: self.id,
            name: self.name,
            reads: self.reads,
            writes: self.writes,
            execute: self.execute,
        }
    }

    /// 直接构建为结构化的 Pass（不带执行函数，always None）。
    pub fn build_structural(self) -> PassNode {
        PassNode {
            id: self.id,
            name: self.name,
            reads: self.reads,
            writes: self.writes,
            execute: None,
        }
    }
}
