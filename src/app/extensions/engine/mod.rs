//! UIX 软件扩展的 Scheme（R7RS-small 受限宿主环境）自研解释器核心。
//!
//! 定位（合同见 `docs/架构/app/extensions.md` 与 P0 取证）：软件扩展的
//! 进程内执行引擎，作为 app `extensions` Module 的私有 Component。交付
//! reader、闭包、proper tail calls、卫生宏、continuation、数值塔、record、
//! 库系统、原语、燃料 / 深度 / 内存 / 墙钟配额、协作取消、可回收堆与
//! 宿主能力注入边界。
//!
//! 安全边界：解释器无默认文件 / 网络 / 进程能力；宿主函数按清单
//! `capabilities` 显式注入，未声明能力在语言层即拒绝。任何失败都以
//! `SchemeError` 返回，panic 由 `run_program` 兜底为稳定失败。
//!
//! 来源说明：以同主人小贝项目 `agent/src/scheme/`（取证提交
//! `0fa72644`）的独立化移植为基础，按 P0 矩阵补齐标准缺口；不依赖
//! 小贝 Agent 业务层。

mod cancel;
mod error;
mod eval;
mod number;
mod primitive;
mod primitive_ext;
mod reader;
mod syntax_rules;
mod value;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::panic::AssertUnwindSafe;
use std::rc::Rc;
use std::time::{Duration, Instant};

pub use cancel::CancelToken;
pub use error::SchemeError;
pub(crate) use reader::read_all;
pub use value::{HostFunction, Pair, Value};
use value::{Env, EnvNode, HeapSlot, PrimitiveFn};

/// include 来源解析器：包内声明路径 → 源码文本；未登记来源拒绝。
pub type IncludeSource = Rc<dyn Fn(&str) -> Option<String>>;

/// 宿主值登记：扩展把引擎内值（命令过程等）按命名空间登记，供宿主
/// 层调用；闭包与环境不跨 FFI 边界，调用回到引擎内完成。
#[derive(Clone)]
pub struct HostRegistration {
    pub namespace: String,
    pub name: String,
    value: Value,
}

impl HostRegistration {
    pub(crate) fn value(&self) -> &Value {
        &self.value
    }
}

/// 单次脚本执行的资源边界；配额即生命周期边界，回收由标记-清扫堆承担。
#[derive(Debug, Clone, Copy)]
pub struct SchemeLimits {
    /// 求值与读取步数的燃料上限。
    pub maximum_fuel: u64,
    /// 非尾求值递归深度上限。
    pub maximum_depth: u32,
    /// 读取器嵌套深度上限。
    pub maximum_reader_depth: u32,
    /// 源码字节上限。
    pub maximum_source_bytes: usize,
    /// pair / 向量 / 字符串分配数量配额。
    pub maximum_allocated_values: u64,
    /// 字符串累计字节配额。
    pub maximum_string_bytes: u64,
    /// 单次分配字节上限（make-vector / make-string / bytevector 等）。
    pub maximum_single_allocation_bytes: usize,
    /// 标记-清扫回收后的存活字节上限。
    pub maximum_live_bytes: u64,
    /// 触发回收的累计分配次数阈值。
    pub gc_trigger_allocations: u64,
    /// 单次求值调用的墙钟毫秒上限；超限不可捕获。
    pub maximum_wall_time_ms: u64,
}

impl Default for SchemeLimits {
    fn default() -> Self {
        Self {
            maximum_fuel: 5_000_000,
            // 显式栈机器后帧为堆分配，上限是帧数资源边界而非宿主栈保护。
            maximum_depth: 8_192,
            maximum_reader_depth: 1_024,
            maximum_source_bytes: 1 << 20,
            maximum_allocated_values: 2_000_000,
            maximum_string_bytes: 8 << 20,
            maximum_single_allocation_bytes: 1 << 20,
            maximum_live_bytes: 4 << 20,
            gc_trigger_allocations: 4_096,
            maximum_wall_time_ms: 1_000,
        }
    }
}

/// 一个已定义的库：命名环境与导出名集合。
#[derive(Clone)]
pub struct Library {
    env: value::Env,
    exports: std::collections::BTreeSet<String>,
}

impl Library {
    fn exports(&self) -> impl Iterator<Item = (&str, value::Value)> + '_ {
        self.exports
            .iter()
            .filter_map(|name| self.env.borrow().bindings.get(name.as_str()).cloned().map(|value| (name.as_str(), value)))
    }

    pub(crate) fn env(&self) -> &value::Env {
        &self.env
    }
}

/// R7RS-small 标准库分组；宿主环境预导入的分组以此识别。
/// `(scheme file)`、`(scheme process-context)` 等涉及文件 / 进程端口的
/// 分组不在白名单内——标准库暴露面按宿主环境合同冻结。
pub const STANDARD_LIBRARY_WHITELIST: &[&str] = &[
    "scheme/base",
    "scheme/char",
    "scheme/inexact",
    "scheme/complex",
    "scheme/lazy",
    "scheme/case-lambda",
    "scheme/cxr",
    "scheme/read",
    "scheme/write",
    "scheme/repl",
    "scheme/time",
    "scheme/load",
    "scheme/r5rs",
    // UIX 宿主库：随引擎构造全局安装，import 语义为幂等成功。
    "uix/extension",
    "uix/host",
];

/// 读取清单数据（扩展包清单解析复用引擎 reader；限制源码大小）。
pub fn read_manifest_data(
    engine: &mut SchemeEngine,
    text: &str,
) -> Result<Vec<Value>, SchemeError> {
    const MAX_MANIFEST_BYTES: usize = 16 * 1024;
    if text.len() > MAX_MANIFEST_BYTES {
        return Err(SchemeError::SourceTooLarge {
            actual: text.len(),
            maximum: MAX_MANIFEST_BYTES,
        });
    }
    read_all(engine, text)
}

/// 静态语法校验：只读取不执行（扩展装载期入口检查复用）。
///
/// 使用一次性引擎跑 reader 全量解析；源码大小与 reader 边界照常生效，
/// 分配记账覆盖字符串字面量，任何读取失败都是类型化的。
pub fn validate_syntax(source: &str) -> Result<(), SchemeError> {
    let mut engine = SchemeEngine::new(
        SchemeLimits::default(),
        BTreeSet::new(),
        Vec::new(),
        CancelToken::new(),
    )?;
    read_all(&mut engine, source)?;
    Ok(())
}

/// 扩展脚本引擎：持配额记账、全局环境、能力声明与取消信号。
///
/// panic 后引擎标记为污染并不再复用；长驻实例由扩展 Module 持有，
/// 堆回收经标记-清扫在安全点执行。
pub struct SchemeEngine {
    limits: SchemeLimits,
    fuel: u64,
    allocated_values: u64,
    string_bytes: u64,
    globals: Env,
    capabilities: BTreeSet<String>,
    hosts: BTreeMap<String, Rc<HostFunction>>,
    cancel: CancelToken,
    poisoned: bool,
    macro_expansions: u64,
    libraries: BTreeMap<String, Library>,
    wall_deadline: Option<Instant>,
    identity: Option<(String, String)>,
    registrations: Vec<HostRegistration>,
    heap: Vec<HeapSlot>,
    allocations_since_gc: u64,
    live_bytes: u64,
    include_source: Option<IncludeSource>,
    default_output: Option<Value>,
}

impl SchemeEngine {
    /// 构造引擎：安装原语并绑定注入的宿主能力函数。
    pub fn new(
        limits: SchemeLimits,
        capabilities: BTreeSet<String>,
        hosts: Vec<HostFunction>,
        cancel: CancelToken,
    ) -> Result<Self, SchemeError> {
        let globals = EnvNode::root();
        let mut engine = Self {
            limits,
            fuel: limits.maximum_fuel,
            allocated_values: 0,
            string_bytes: 0,
            globals: globals.clone(),
            capabilities,
            hosts: BTreeMap::new(),
            cancel,
            poisoned: false,
            macro_expansions: 0,
            libraries: BTreeMap::new(),
            wall_deadline: None,
            identity: None,
            registrations: Vec::new(),
            heap: Vec::new(),
            allocations_since_gc: 0,
            live_bytes: 0,
            include_source: None,
            default_output: None,
        };
        engine.track_env(&globals);
        primitive::install(&mut engine);
        for host in hosts {
            let function = Rc::new(host);
            if engine.hosts.contains_key(function.name()) {
                return Err(SchemeError::InvalidCapabilityName {
                    name: function.name().to_string(),
                });
            }
            let name: Rc<str> = function.name().into();
            engine
                .hosts
                .insert(function.name().to_string(), function.clone());
            EnvNode::define(&globals, &name, Value::Host(function));
        }
        Ok(engine)
    }

    /// 读取并依次求值全部顶层表达式；返回最后一个值。
    ///
    /// 解释器内部 panic 在此兜底为 `PanicCaught`，之后引擎不可复用。
    /// 每次调用重置燃料与墙钟预算。
    pub fn run_program(&mut self, source: &str) -> Result<Value, SchemeError> {
        if let Err(error) = self.reset_budgets() {
            return Err(error);
        }
        let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| {
            let data = read_all(self, source)?;
            let globals = self.globals.clone();
            let mut last = Value::Unspecified;
            for datum in &data {
                last = eval::eval(self, datum, &globals)?;
            }
            Ok(last)
        }));
        match outcome {
            Ok(result) => result.map_err(|error| match error {
                // 未捕获异常在顶层转为可序列化摘要；原对象由机器丢弃。
                SchemeError::Raised(payload) => SchemeError::UncapturedRaise {
                    summary: payload.to_write_string(),
                },
                other => other,
            }),
            Err(_) => {
                self.poisoned = true;
                Err(SchemeError::PanicCaught)
            }
        }
    }

    /// 重置可复用配额（燃料、墙钟）；分配记账保留累计语义。
    fn reset_budgets(&mut self) -> Result<(), SchemeError> {
        if self.poisoned {
            return Err(SchemeError::EnginePoisoned);
        }
        self.fuel = self.limits.maximum_fuel;
        self.wall_deadline = Some(
            Instant::now() + Duration::from_millis(self.limits.maximum_wall_time_ms),
        );
        Ok(())
    }

    /// 求值检查点：扣减燃料并检查取消与墙钟；每个求值与读取步骤调用。
    pub(crate) fn tick(&mut self) -> Result<(), SchemeError> {
        if self.fuel == 0 {
            return Err(SchemeError::FuelExhausted);
        }
        self.fuel -= 1;
        if self.cancel.is_cancelled() {
            return Err(SchemeError::Cancelled);
        }
        if let Some(deadline) = self.wall_deadline
            && Instant::now() > deadline
        {
            return Err(SchemeError::WallClockExceeded {
                milliseconds: self.limits.maximum_wall_time_ms,
            });
        }
        Ok(())
    }

    pub(crate) fn limits(&self) -> &SchemeLimits {
        &self.limits
    }

    fn reserve_value(&mut self) -> Result<(), SchemeError> {
        if self.allocated_values >= self.limits.maximum_allocated_values {
            return Err(SchemeError::AllocationQuotaExceeded {
                maximum: self.limits.maximum_allocated_values,
            });
        }
        self.allocated_values += 1;
        self.allocations_since_gc += 1;
        Ok(())
    }

    fn reserve_string_bytes(&mut self, bytes: usize) -> Result<(), SchemeError> {
        let bytes = bytes as u64;
        if self.string_bytes.saturating_add(bytes) > self.limits.maximum_string_bytes {
            return Err(SchemeError::StringQuotaExceeded {
                maximum: self.limits.maximum_string_bytes,
            });
        }
        self.string_bytes += bytes;
        Ok(())
    }

    fn reserve_single(&mut self, bytes: usize) -> Result<(), SchemeError> {
        if bytes > self.limits.maximum_single_allocation_bytes {
            return Err(SchemeError::SingleAllocationTooLarge {
                requested: bytes,
                maximum: self.limits.maximum_single_allocation_bytes,
            });
        }
        Ok(())
    }

    pub(crate) fn new_pair(&mut self, car: Value, cdr: Value) -> Result<Value, SchemeError> {
        self.reserve_value()?;
        let cell = Rc::new(RefCell::new(Pair { car, cdr }));
        self.heap.push(HeapSlot::Pair(Rc::downgrade(&cell)));
        Ok(Value::Pair(cell))
    }

    pub(crate) fn new_string_from(&mut self, text: String) -> Result<Value, SchemeError> {
        self.reserve_single(text.len())?;
        self.reserve_value()?;
        self.reserve_string_bytes(text.len())?;
        let cell = Rc::new(RefCell::new(text));
        self.heap.push(HeapSlot::String(Rc::downgrade(&cell)));
        Ok(Value::String(cell))
    }

    pub(crate) fn new_string_with_fill(
        &mut self,
        size: usize,
        fill: char,
    ) -> Result<Value, SchemeError> {
        self.reserve_single(size * 4)?;
        self.reserve_value()?;
        let text: String = std::iter::repeat_n(fill, size).collect();
        self.reserve_string_bytes(text.len())?;
        let cell = Rc::new(RefCell::new(text));
        self.heap.push(HeapSlot::String(Rc::downgrade(&cell)));
        Ok(Value::String(cell))
    }

    pub(crate) fn new_vector_from(&mut self, items: Vec<Value>) -> Result<Value, SchemeError> {
        self.reserve_single(items.len() * std::mem::size_of::<Value>())?;
        self.reserve_value()?;
        let cell = Rc::new(RefCell::new(items));
        self.heap.push(HeapSlot::Vector(Rc::downgrade(&cell)));
        Ok(Value::Vector(cell))
    }

    pub(crate) fn new_bytevector_from(&mut self, bytes: Vec<u8>) -> Result<Value, SchemeError> {
        self.reserve_single(bytes.len())?;
        self.reserve_value()?;
        let cell = Rc::new(RefCell::new(bytes));
        self.heap.push(HeapSlot::Bytevector(Rc::downgrade(&cell)));
        Ok(Value::Bytevector(cell))
    }

    pub(crate) fn new_record(
        &mut self,
        type_name: std::rc::Rc<str>,
        fields: Vec<Value>,
    ) -> Result<Value, SchemeError> {
        self.reserve_single(fields.len() * std::mem::size_of::<Value>())?;
        self.reserve_value()?;
        let cell = Rc::new(RefCell::new(value::Record::new(type_name, fields)));
        self.heap.push(HeapSlot::Record(Rc::downgrade(&cell)));
        Ok(Value::Record(cell))
    }

    pub(crate) fn new_vector_with_fill(
        &mut self,
        size: usize,
        fill: Value,
    ) -> Result<Value, SchemeError> {
        self.reserve_single(size * std::mem::size_of::<Value>())?;
        self.reserve_value()?;
        let cell = Rc::new(RefCell::new(vec![fill; size]));
        self.heap.push(HeapSlot::Vector(Rc::downgrade(&cell)));
        Ok(Value::Vector(cell))
    }

    pub(crate) fn new_closure(&mut self, closure: value::Closure) -> Result<Value, SchemeError> {
        self.reserve_single(closure.body.len() * std::mem::size_of::<Value>())?;
        self.reserve_value()?;
        let cell = Rc::new(closure);
        self.heap.push(HeapSlot::Closure(Rc::downgrade(&cell)));
        Ok(Value::Closure(cell))
    }

    pub(crate) fn new_env(&mut self, parent: &Env) -> Env {
        self.allocated_values += 1;
        self.allocations_since_gc += 1;
        let env = EnvNode::child(parent);
        self.track_env(&env);
        env
    }

    fn track_env(&mut self, env: &Env) {
        self.heap.push(HeapSlot::Env(Rc::downgrade(env)));
    }

    pub(crate) fn new_continuation(
        &mut self,
        snapshot: eval::ContinuationSnapshot,
    ) -> Result<Value, SchemeError> {
        self.reserve_value()?;
        let cell = Rc::new(snapshot);
        self.heap.push(HeapSlot::Continuation(Rc::downgrade(&cell)));
        Ok(Value::Continuation(cell))
    }

    pub(crate) fn new_macro(
        &mut self,
        transformer: syntax_rules::MacroTransformer,
    ) -> Result<Value, SchemeError> {
        self.reserve_value()?;
        let cell = Rc::new(transformer);
        self.heap.push(HeapSlot::Macro(Rc::downgrade(&cell)));
        Ok(Value::Macro(cell))
    }

    pub(crate) fn new_error_object(&mut self, error: value::ErrorObject) -> Result<Value, SchemeError> {
        self.reserve_value()?;
        let cell = Rc::new(error);
        self.heap.push(HeapSlot::ErrorObject(Rc::downgrade(&cell)));
        Ok(Value::ErrorObject(cell))
    }

    pub(crate) fn new_values(&mut self, items: Vec<Value>) -> Result<Value, SchemeError> {
        self.reserve_single(items.len() * std::mem::size_of::<Value>())?;
        self.reserve_value()?;
        let cell = Rc::new(items);
        self.heap.push(HeapSlot::Values(Rc::downgrade(&cell)));
        Ok(Value::Values(cell))
    }

    pub(crate) fn new_parameter(&mut self, state: value::ParameterState) -> Result<Value, SchemeError> {
        self.reserve_value()?;
        let cell = Rc::new(RefCell::new(state));
        self.heap.push(HeapSlot::Parameter(Rc::downgrade(&cell)));
        Ok(Value::Parameter(cell))
    }

    pub(crate) fn new_promise(&mut self, state: value::PromiseState) -> Result<Value, SchemeError> {
        self.reserve_value()?;
        let cell = Rc::new(RefCell::new(state));
        self.heap.push(HeapSlot::Promise(Rc::downgrade(&cell)));
        Ok(Value::Promise(cell))
    }

    pub(crate) fn new_port(&mut self, state: value::PortState) -> Result<Value, SchemeError> {
        match &state {
            value::PortState::Input { text, .. } | value::PortState::Output { text, .. } => {
                self.reserve_single(text.len())?;
            }
        }
        self.reserve_value()?;
        let cell = Rc::new(RefCell::new(state));
        self.heap.push(HeapSlot::Port(Rc::downgrade(&cell)));
        Ok(Value::Port(cell))
    }

    /// 回收后存活字节（最近一次统计）。
    pub fn live_bytes(&self) -> u64 {
        self.live_bytes
    }

    /// 是否到达回收阈值（安全点由机器与读取循环调用方检查）。
    pub(crate) fn gc_due(&self) -> bool {
        self.allocations_since_gc >= self.limits.gc_trigger_allocations
    }

    /// 标记-清扫回收：以引擎根 + 调用方额外根（值与环境）执行。
    ///
    /// 追溯不可达的可变容器被清空以断环，随后由引用计数自然释放；
    /// 回收步数计入燃料，回收后校验存活字节配额。
    pub(crate) fn collect_garbage(
        &mut self,
        extra_values: &[Value],
        extra_envs: &[value::Env],
    ) -> Result<(), SchemeError> {
        let mut marked: HashSet<usize> = HashSet::new();
        let mut stack: Vec<Value> = Vec::new();
        // 引擎根：全局环境、库环境、登记值。
        let mut root_envs: Vec<value::Env> = vec![self.globals.clone()];
        for library in self.libraries.values() {
            root_envs.push(library.env().clone());
        }
        root_envs.extend(extra_envs.iter().cloned());
        for env in &root_envs {
            self.mark_env(env, &mut marked, &mut stack);
        }
        for registration in &self.registrations {
            stack.push(registration.value().clone());
        }
        if let Some(output) = &self.default_output {
            stack.push(output.clone());
        }
        stack.extend(extra_values.iter().cloned());
        // 标记遍历（步数计燃料，回收暂停有界）。
        while let Some(current) = stack.pop() {
            if self.fuel == 0 {
                return Err(SchemeError::FuelExhausted);
            }
            self.fuel -= 1;
            let Some(pointer) = current.heap_pointer() else {
                continue;
            };
            if !marked.insert(pointer) {
                continue;
            }
            match &current {
                Value::Pair(cell) => {
                    if let Ok(borrowed) = cell.try_borrow() {
                        stack.push(borrowed.car.clone());
                        stack.push(borrowed.cdr.clone());
                    }
                }
                Value::Vector(items) => {
                    if let Ok(borrowed) = items.try_borrow() {
                        stack.extend(borrowed.iter().cloned());
                    }
                }
                Value::Record(record) => {
                    if let Ok(borrowed) = record.try_borrow() {
                        stack.extend(borrowed.fields().iter().cloned());
                    }
                }
                Value::Closure(closure) => {
                    self.mark_env(&closure.env.clone(), &mut marked, &mut stack);
                    stack.extend(closure.body.iter().cloned());
                }
                Value::Continuation(snapshot) => {
                    for env in snapshot.environments() {
                        self.mark_env(&env, &mut marked, &mut stack);
                    }
                    stack.extend(snapshot.child_values());
                }
                Value::Macro(transformer) => {
                    transformer.push_children(&mut stack);
                }
                Value::Parameter(cell) => {
                    if let Ok(borrowed) = cell.try_borrow() {
                        if let Some(converter) = &borrowed.converter {
                            stack.push(converter.clone());
                        }
                        stack.push(borrowed.current.clone());
                    }
                }
                Value::Promise(cell) => {
                    if let Ok(borrowed) = cell.try_borrow() {
                        match &*borrowed {
                            value::PromiseState::Pending(thunk)
                            | value::PromiseState::Forced(thunk) => stack.push(thunk.clone()),
                        }
                    }
                }
                Value::ErrorObject(error) => {
                    stack.extend(error.irritants().iter().cloned());
                }
                Value::Values(items) => {
                    stack.extend(items.iter().cloned());
                }
                _ => {}
            }
        }
        // 清扫：失效槽位移除，不可达对象断环，统计存活字节。
        let mut live_bytes: u64 = 0;
        self.heap.retain(|slot| {
            let Some(pointer) = slot.pointer() else {
                return false;
            };
            if marked.contains(&pointer) {
                live_bytes += slot.approx_bytes() as u64;
                return true;
            }
            slot.drain();
            // 不可达对象保留在表中直到弱引用失效；字节不计入存活。
            true
        });
        self.live_bytes = live_bytes;
        self.allocations_since_gc = 0;
        if self.live_bytes > self.limits.maximum_live_bytes {
            return Err(SchemeError::HeapQuotaExceeded {
                live: self.live_bytes,
                maximum: self.limits.maximum_live_bytes,
            });
        }
        Ok(())
    }

    /// 标记环境节点并把绑定值压入遍历栈。
    fn mark_env(&self, env: &Env, marked: &mut HashSet<usize>, stack: &mut Vec<Value>) {
        let pointer = value::env_pointer(env);
        if !marked.insert(pointer) {
            return;
        }
        if let Ok(borrowed) = env.try_borrow() {
            stack.extend(borrowed.bindings.values().cloned());
            let parent = borrowed.parent.clone();
            drop(borrowed);
            if let Some(parent) = parent {
                self.mark_env(&parent, marked, stack);
            }
        }
    }

    /// 关闭清扫：清空全部堆对象内容（不可达断环由 drain 完成；可达对象
    /// 被一并清空以释放环引用），供实例卸载前调用。
    pub fn shutdown_collect(&mut self) {
        for slot in &self.heap {
            slot.drain();
        }
        self.heap.clear();
        self.registrations.clear();
        self.libraries.clear();
        self.live_bytes = 0;
    }

    /// 引擎内堆对象数量（诊断视图）。
    pub fn heap_slots(&self) -> usize {
        self.heap.len()
    }

    /// 把严格列表展平为向量。
    pub(crate) fn value_to_vec(
        &mut self,
        operation: &'static str,
        value: &Value,
    ) -> Result<Vec<Value>, SchemeError> {
        let mut elements = Vec::new();
        let mut current = value.clone();
        loop {
            match current {
                Value::Null => return Ok(elements),
                Value::Pair(pair) => {
                    let borrowed = pair.borrow();
                    elements.push(borrowed.car.clone());
                    current = borrowed.cdr.clone();
                }
                _ => {
                    return Err(SchemeError::WrongType {
                        operation,
                        expected: "严格列表",
                    })
                }
            }
        }
    }

    /// 由切片构建列表（消耗分配配额）。
    pub(crate) fn list_from_slice(&mut self, elements: &[Value]) -> Result<Value, SchemeError> {
        let mut list = Value::Null;
        for element in elements.iter().rev() {
            list = self.new_pair(element.clone(), list)?;
        }
        Ok(list)
    }

    /// 调用注入的宿主函数；能力未声明即在语言层拒绝。
    pub(crate) fn call_host(
        &mut self,
        function: &Rc<HostFunction>,
        arguments: &[Value],
    ) -> Result<Value, SchemeError> {
        if !self.capabilities.contains(function.name()) {
            return Err(SchemeError::CapabilityNotDeclared {
                name: function.name().to_string(),
            });
        }
        function
            .invoke(self, arguments)
            .map_err(|error| SchemeError::HostFunctionError {
                name: function.name().to_string(),
                message: error.to_string(),
            })
    }

    pub(crate) fn define_primitive(&mut self, name: &str, function: PrimitiveFn) {
        let key: Rc<str> = name.into();
        EnvNode::define(&self.globals, &key, Value::Primitive(function));
    }

    /// 登记已求值完成的库（define-library 声明体执行后调用）。
    pub(crate) fn register_library(
        &mut self,
        name: String,
        env: value::Env,
        exports: std::collections::BTreeSet<String>,
    ) {
        self.libraries.insert(name, Library { env, exports });
    }

    /// 返回库导出迭代（库名，导出名，绑定值）。
    pub(crate) fn library_exports(&self, name: &str) -> Option<Library> {
        self.libraries.get(name).cloned()
    }

    /// 库名是否属于标准库白名单（预导入分组，import 幂等）。
    pub(crate) fn is_standard_library(name: &str) -> bool {
        STANDARD_LIBRARY_WHITELIST.contains(&name)
    }

    /// 全局环境快照（库环境以它为父构造）。
    pub(crate) fn globals_snapshot(&self) -> Env {
        self.globals.clone()
    }

    /// 分配下一个宏展开号（重命名卫生的新颖性来源）。
    pub(crate) fn next_macro_expansion(&mut self) -> u64 {
        self.macro_expansions += 1;
        self.macro_expansions
    }

    /// 注册需要机器状态的控制原语（call/cc、dynamic-wind 等）。
    pub(crate) fn define_control(&mut self, name: &str, op: eval::ControlOp) {
        let key: Rc<str> = name.into();
        EnvNode::define(&self.globals, &key, Value::Control(op));
    }

    /// 设置扩展身份；`(uix extension)` 库的过程据此应答。
    pub(crate) fn set_extension_identity(&mut self, id: String, version: String) {
        self.identity = Some((id, version));
    }

    pub(crate) fn extension_identity(&self) -> Option<(&str, &str)> {
        self.identity
            .as_ref()
            .map(|(id, version)| (id.as_str(), version.as_str()))
    }

    /// 登记宿主值（命令过程等）；重复命名空间 + 名称被拒绝。
    pub(crate) fn register_host_value(
        &mut self,
        namespace: &str,
        name: &str,
        value: Value,
    ) -> Result<(), SchemeError> {
        if self
            .registrations
            .iter()
            .any(|existing| existing.namespace == namespace && existing.name == name)
        {
            return Err(SchemeError::InvalidSyntax {
                form: "register-command!",
                reason: "命令名重复",
            });
        }
        self.registrations.push(HostRegistration {
            namespace: namespace.to_string(),
            name: name.to_string(),
            value,
        });
        Ok(())
    }

    /// 已登记宿主值视图；值仍由引擎持有。
    pub(crate) fn host_registrations(&self) -> &[HostRegistration] {
        &self.registrations
    }

    /// 重绑取消令柄（候选发布为活动代时替换取消信号）。
    pub(crate) fn rebind_cancel(&mut self, cancel: CancelToken) {
        self.cancel = cancel;
    }

    /// 活动代的单次调用墙钟（prepare 与命令调用使用不同预算）。
    pub(crate) fn set_command_wall_time_ms(&mut self, milliseconds: u64) {
        self.limits.maximum_wall_time_ms = milliseconds;
        self.wall_deadline = None;
    }

    /// 注入 include 来源解析器（扩展包内声明路径）。
    pub fn set_include_source(&mut self, source: IncludeSource) {
        self.include_source = Some(source);
    }

    pub(crate) fn include_source(&self, path: &str) -> Option<String> {
        self.include_source.as_ref().and_then(|resolve| resolve(path))
    }

    /// 库是否已登记（cond-expand 的 library 测试）。
    pub(crate) fn has_library(&self, name: &str) -> bool {
        self.libraries.contains_key(name)
    }

    /// `current-output-port` 的实例级内存端口（惰性创建）。
    pub(crate) fn default_output_port(&mut self) -> Result<Value, SchemeError> {
        if let Some(port) = &self.default_output {
            return Ok(port.clone());
        }
        let port = self.new_port(value::PortState::Output {
            text: String::new(),
            closed: false,
        })?;
        self.default_output = Some(port.clone());
        Ok(port)
    }

    /// 默认输出端口当前累积的文本（诊断出口）。
    pub fn output_text(&self) -> String {
        self.default_output
            .as_ref()
            .and_then(|port| match port {
                Value::Port(cell) => match &*cell.borrow() {
                    value::PortState::Output { text, .. } => Some(text.clone()),
                    _ => None,
                },
                _ => None,
            })
            .unwrap_or_default()
    }

    /// 应用一个已登记值（命令过程）；宿主调用的引擎侧入口。
    pub(crate) fn apply_registration(
        &mut self,
        namespace: &str,
        name: &str,
        arguments: &[Value],
    ) -> Result<Value, SchemeError> {
        self.call_registration(namespace, name, arguments)
    }

    pub(crate) fn call_registration(
        &mut self,
        namespace: &str,
        name: &str,
        arguments: &[Value],
    ) -> Result<Value, SchemeError> {
        let Some(index) = self
            .registrations
            .iter()
            .position(|existing| existing.namespace == namespace && existing.name == name)
        else {
            return Err(SchemeError::NotCallable);
        };
        self.reset_budgets()?;
        let procedure = self.registrations[index].value().clone();
        eval::apply_procedure(self, &procedure, arguments).map_err(|error| match error {
            SchemeError::Raised(payload) => SchemeError::UncapturedRaise {
                summary: payload.to_write_string(),
            },
            other => other,
        })
    }
}
