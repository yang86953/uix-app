//! 软件动态扩展（P1：无界面最小闭环）。
//!
//! 显式启用（Cargo feature `extensions`）后的运行时扩展宿主：装载
//! R7RS-small 受限宿主环境的扩展包，注册命令并接受类型化调用。Rust 宿主
//! 持有领域真值、授权与进程生命周期；扩展经 `(uix host)` 端口组合宿主
//! 已编译并开放的能力。完整契约见
//! [extensions](../../../../../docs/架构/app/extensions.md)。
//!
//! 本 Module 非线程安全：实例串行求值，仅 `CancelHandle` 可跨线程。

pub(crate) mod engine;
mod manifest;
mod package;

use std::collections::BTreeMap;
use std::sync::Arc;

pub use manifest::ExtensionManifest;
pub use package::ExtensionPackage;

pub use engine::CancelToken as ExtensionCancelToken;

/// 扩展宿主端口：应用注入的只读或受控业务能力。
///
/// 入参为跨边界 owned 值；闭包、环境与引擎值不离开边界。
pub type ExtensionPort =
    Arc<dyn Fn(&[ExtensionValue]) -> Result<ExtensionValue, String> + Send + Sync>;

/// 跨边界 owned 值；拒绝闭包、环境、continuation 与 record。
#[derive(Debug, Clone, PartialEq)]
pub enum ExtensionValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    Symbol(String),
    List(Vec<ExtensionValue>),
    Bytes(Vec<u8>),
}

/// 扩展宿主的可调资源边界（P1 子集；缺省值见 `Default`）。
#[derive(Debug, Clone, Copy)]
pub struct ExtensionHostConfig {
    /// 准备（入口求值）墙钟毫秒上限。
    pub prepare_wall_time_ms: u64,
    /// 单次命令调用墙钟毫秒上限。
    pub command_wall_time_ms: u64,
    /// 单次调用求值步燃料。
    pub maximum_fuel: u64,
    /// 非尾求值帧深上限。
    pub maximum_depth: u32,
    /// 单包源码字节上限。
    pub maximum_source_bytes: usize,
}

impl Default for ExtensionHostConfig {
    fn default() -> Self {
        Self {
            prepare_wall_time_ms: 5_000,
            command_wall_time_ms: 1_000,
            maximum_fuel: 5_000_000,
            maximum_depth: 8_192,
            maximum_source_bytes: 256 * 1024,
        }
    }
}

/// 扩展操作的类型化失败；脚本域错误携带 write 形式摘要。
#[derive(Debug, Clone, PartialEq)]
pub enum ExtensionError {
    /// 包结构、清单或来源校验失败。
    Package(String),
    /// 源码解析失败。
    Parse(String),
    /// 标准支持缺口（受限宿主环境拒绝项）。
    Unsupported(String),
    /// 清单声明与宿主能力不兼容。
    Incompatible(String),
    /// 能力未授权或未声明。
    CapabilityDenied(String),
    /// 调用参数不符。
    Argument(String),
    /// 配额（燃料 / 深度 / 分配 / 存活堆）超限。
    Quota(String),
    /// 协作取消。
    Cancelled,
    /// 墙钟超时。
    Timeout { milliseconds: u64 },
    /// 预期活动代不符（P3 替换协议使用）。
    StaleGeneration { expected: u64, actual: u64 },
    /// 宿主端口自身失败。
    HostFailure(String),
    /// 脚本 `error` / 未捕获 `raise`。
    ScriptFailure(String),
    /// 引擎 panic 后污染，实例不可复用。
    EnginePoisoned,
    /// 停止 / 卸载尚未完成。
    ShutdownIncomplete(String),
    /// 未知扩展（未激活或已卸载）。
    UnknownExtension(String),
    /// 已知扩展但命令不存在。
    UnknownCommand { extension: String, command: String },
}

impl std::fmt::Display for ExtensionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtensionError::Package(message)
            | ExtensionError::Parse(message)
            | ExtensionError::Unsupported(message)
            | ExtensionError::Incompatible(message)
            | ExtensionError::CapabilityDenied(message)
            | ExtensionError::Argument(message)
            | ExtensionError::Quota(message)
            | ExtensionError::HostFailure(message)
            | ExtensionError::ScriptFailure(message)
            | ExtensionError::ShutdownIncomplete(message) => formatter.write_str(message),
            ExtensionError::Cancelled => formatter.write_str("扩展执行已取消"),
            ExtensionError::Timeout { milliseconds } => {
                write!(formatter, "扩展执行超过 {milliseconds}ms 墙钟上限")
            }
            ExtensionError::StaleGeneration { expected, actual } => {
                write!(formatter, "预期活动代 {expected}，实际 {actual}")
            }
            ExtensionError::EnginePoisoned => {
                formatter.write_str("扩展引擎已污染，实例不可复用")
            }
            ExtensionError::UnknownExtension(id) => {
                write!(formatter, "扩展 {id} 未激活或已卸载")
            }
            ExtensionError::UnknownCommand { extension, command } => {
                write!(formatter, "扩展 {extension} 没有命令 {command}")
            }
        }
    }
}

impl std::error::Error for ExtensionError {}

impl ExtensionError {
    fn internal(stage: &str, error: &engine::SchemeError) -> Self {
        match error {
            engine::SchemeError::EnginePoisoned => ExtensionError::EnginePoisoned,
            other => ExtensionError::Package(format!("{stage} 失败：{other}")),
        }
    }

    fn from_engine(error: engine::SchemeError) -> Self {
        use engine::SchemeError as Source;
        match error {
            Source::FuelExhausted => {
                ExtensionError::Quota("求值燃料耗尽".to_string())
            }
            Source::DepthLimitExceeded { maximum } => {
                ExtensionError::Quota(format!("求值深度超过 {maximum} 上限"))
            }
            Source::AllocationQuotaExceeded { maximum } => {
                ExtensionError::Quota(format!("分配值数量超过 {maximum} 配额"))
            }
            Source::StringQuotaExceeded { maximum } => {
                ExtensionError::Quota(format!("字符串字节超过 {maximum} 配额"))
            }
            Source::SingleAllocationTooLarge { requested, maximum } => ExtensionError::Quota(
                format!("单次分配 {requested} 字节超过 {maximum} 上限"),
            ),
            Source::HeapQuotaExceeded { live, maximum } => {
                ExtensionError::Quota(format!("回收后存活堆 {live} 超过 {maximum} 配额"))
            }
            Source::Cancelled => ExtensionError::Cancelled,
            Source::WallClockExceeded { milliseconds } => {
                ExtensionError::Timeout { milliseconds }
            }
            Source::EnginePoisoned | Source::PanicCaught => ExtensionError::EnginePoisoned,
            Source::UserError { message } | Source::UncapturedRaise { summary: message } => {
                ExtensionError::ScriptFailure(message)
            }
            Source::CapabilityNotDeclared { name } => {
                ExtensionError::CapabilityDenied(format!("能力 {name} 未在清单声明"))
            }
            Source::InvalidCapabilityName { name } => {
                ExtensionError::Package(format!("能力名 {name} 无效"))
            }
            Source::HostFunctionError { name, message } => {
                ExtensionError::HostFailure(format!("端口 {name} 失败：{message}"))
            }
            Source::SourceTooLarge { actual, maximum } => ExtensionError::Package(format!(
                "源码 {actual} 字节超过 {maximum} 上限"
            )),
            Source::NotImplemented { feature } => {
                ExtensionError::Unsupported(feature.to_string())
            }
            Source::LibraryNotFound { name } => {
                ExtensionError::Unsupported(format!("库 {name} 未定义或不在白名单"))
            }
            other => ExtensionError::Argument(other.to_string()),
        }
    }
}

/// 准备完成、尚未发布的扩展候选。
///
/// 候选持有独立引擎与暂存命令表；`activate` 消费候选并发布为活动代。
pub struct PreparedExtension {
    package: ExtensionPackage,
    engine: engine::SchemeEngine,
    commands: Vec<String>,
}

impl PreparedExtension {
    /// 候选的命令表（暂存，未发布）。
    pub fn commands(&self) -> &[String] {
        &self.commands
    }

    /// 候选清单。
    pub fn manifest(&self) -> Result<ExtensionManifest, ExtensionError> {
        self.package.manifest()
    }
}

impl std::fmt::Debug for PreparedExtension {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedExtension")
            .field("commands", &self.commands)
            .finish_non_exhaustive()
    }
}

/// 激活终态：候选已成为活动代，命令集对新调用可见。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivationReceipt {
    pub extension_id: String,
    pub version: String,
    pub generation: u64,
    pub commands: Vec<String>,
}

/// 卸载终态：实例已排空并释放。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeardownReceipt {
    pub extension_id: String,
    pub generation: u64,
}

/// 扩展运行状态视图。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionStatus {
    pub extension_id: String,
    pub version: String,
    pub generation: u64,
    pub commands: Vec<String>,
}

struct ActiveInstance {
    generation: u64,
    manifest: ExtensionManifest,
    engine: engine::SchemeEngine,
    commands: Vec<String>,
    cancel: engine::CancelToken,
}

impl ActiveInstance {
    fn construct_owned(&mut self, value: &ExtensionValue) -> Result<engine::Value, ExtensionError> {
        extension_value_into_engine(&mut self.engine, value)
    }
}

/// 扩展宿主：持活动实例表与授权端口；显式创建，关闭前须逐一停用。
pub struct ExtensionHost {
    config: ExtensionHostConfig,
    ports: BTreeMap<String, ExtensionPort>,
    instances: BTreeMap<String, ActiveInstance>,
    generation_counter: u64,
}

impl Default for ExtensionHost {
    fn default() -> Self {
        Self::new()
    }
}

impl ExtensionHost {
    /// 以默认资源边界与空端口表构造。
    pub fn new() -> Self {
        Self {
            config: ExtensionHostConfig::default(),
            ports: BTreeMap::new(),
            instances: BTreeMap::new(),
            generation_counter: 0,
        }
    }

    /// 覆盖资源边界。
    pub fn with_config(mut self, config: ExtensionHostConfig) -> Self {
        self.config = config;
        self
    }

    /// 注册宿主端口；能力名与清单声明、引擎能力集三方一致才可调用。
    pub fn with_port(mut self, name: &str, port: ExtensionPort) -> Self {
        self.ports.insert(name.to_string(), port);
        self
    }

    /// 冻结包、解析入口、候选求值与暂存注册；不发布任何注册。
    pub fn prepare(&self, package: &ExtensionPackage) -> Result<PreparedExtension, ExtensionError> {
        let manifest = package.manifest()?;
        let mut limits = engine::SchemeLimits::default();
        limits.maximum_fuel = self.config.maximum_fuel;
        limits.maximum_depth = self.config.maximum_depth;
        limits.maximum_source_bytes = self.config.maximum_source_bytes;
        limits.maximum_wall_time_ms = self.config.prepare_wall_time_ms;
        let mut capabilities = std::collections::BTreeSet::new();
        let mut hosts = Vec::new();
        for declared in &manifest.capabilities {
            let Some(port) = self.ports.get(declared) else {
                return Err(ExtensionError::Incompatible(format!(
                    "清单声明能力 {declared}，宿主未提供对应端口"
                )));
            };
            let bridged = Arc::clone(port);
            let function = engine::HostFunction::new(
                declared.clone(),
                Arc::new(
                    move |constructor: &mut engine::SchemeEngine,
                          values: &[engine::Value]|
                          -> Result<engine::Value, engine::SchemeError> {
                        // 端口边界：engine 值即转 owned，回调返回后不再被持有；
                        // 返回值经引擎构造器登记，参与配额与回收。
                        let arguments: Vec<ExtensionValue> = values
                            .iter()
                            .map(extension_value_from_engine)
                            .collect::<Result<_, _>>()
                            .map_err(|error| engine::SchemeError::HostFunctionError {
                                name: "port".to_string(),
                                message: error.to_string(),
                            })?;
                        let outcome = bridged(&arguments)
                            .map_err(|message| engine::SchemeError::HostFunctionError {
                                name: "port".to_string(),
                                message,
                            })?;
                        extension_value_into_engine(constructor, &outcome)
                            .map_err(|error| engine::SchemeError::HostFunctionError {
                                name: "port".to_string(),
                                message: error.to_string(),
                            })
                    },
                ),
            )
            .map_err(|error| ExtensionError::internal("端口注入", &error))?;
            hosts.push(function);
            capabilities.insert(declared.clone());
        }
        let cancel = engine::CancelToken::new();
        let mut instance_engine = engine::SchemeEngine::new(limits, capabilities, hosts, cancel.clone())
            .map_err(|error| ExtensionError::internal("引擎构造", &error))?;
        instance_engine.set_extension_identity(manifest.id.clone(), manifest.version.clone());
        let source = package.entry_source()?;
        instance_engine
            .run_program(source)
            .map_err(ExtensionError::from_engine)?;
        let commands = instance_engine
            .host_registrations()
            .iter()
            .filter(|registration| registration.namespace == "command")
            .map(|registration| registration.name.clone())
            .collect();
        Ok(PreparedExtension {
            package: package.clone(),
            engine: instance_engine,
            commands,
        })
    }

    /// 发布候选为活动代；同 ID 已存在活动实例时拒绝（替换属 P3 协议）。
    pub fn activate(
        &mut self,
        prepared: PreparedExtension,
    ) -> Result<ActivationReceipt, ExtensionError> {
        let manifest = prepared.package.manifest()?;
        if self.instances.contains_key(&manifest.id) {
            return Err(ExtensionError::Incompatible(format!(
                "扩展 {} 已有活动实例；替换须走热替换协议",
                manifest.id
            )));
        }
        self.generation_counter += 1;
        let generation = self.generation_counter;
        let receipt = ActivationReceipt {
            extension_id: manifest.id.clone(),
            version: manifest.version.clone(),
            generation,
            commands: prepared.commands.clone(),
        };
        let cancel = engine::CancelToken::new();
        // 引擎内的取消令柄由实例统一持有；候选取消信号不带入活动代；
        // 墙钟预算从准备切换为单次命令调用口径。
        let mut engine = prepared.engine;
        engine.rebind_cancel(cancel.clone());
        engine.set_command_wall_time_ms(self.config.command_wall_time_ms);
        self.instances.insert(
            manifest.id.clone(),
            ActiveInstance {
                generation,
                manifest,
                engine,
                commands: prepared.commands,
                cancel,
            },
        );
        Ok(receipt)
    }

    /// 类型化命令调用；实例串行复用，扩展状态在调用间保留。
    pub fn call_command(
        &mut self,
        extension_id: &str,
        command: &str,
        arguments: &[ExtensionValue],
    ) -> Result<ExtensionValue, ExtensionError> {
        let Some(instance) = self.instances.get_mut(extension_id) else {
            return Err(ExtensionError::UnknownExtension(extension_id.to_string()));
        };
        if !instance.commands.iter().any(|existing| existing == command) {
            return Err(ExtensionError::UnknownCommand {
                extension: extension_id.to_string(),
                command: command.to_string(),
            });
        }
        if arguments.len() > 16 {
            return Err(ExtensionError::Argument(
                "单次调用参数超过 16 个上限".to_string(),
            ));
        }
        let mut engine_arguments = Vec::with_capacity(arguments.len());
        for argument in arguments {
            engine_arguments.push(instance.construct_owned(argument)?);
        }
        let outcome = instance
            .engine
            .call_registration("command", command, &engine_arguments);
        outcome
            .map_err(ExtensionError::from_engine)
            .and_then(|value| extension_value_from_engine(&value))
    }

    /// 活动实例的取消令柄；跨线程安全，下一次检查点生效。
    pub fn cancel_handle(&self, extension_id: &str) -> Option<engine::CancelToken> {
        self.instances
            .get(extension_id)
            .map(|instance| instance.cancel.clone())
    }

    /// 活动实例列表。
    pub fn list(&self) -> Vec<ExtensionStatus> {
        self.instances
            .values()
            .map(|instance| ExtensionStatus {
                extension_id: instance.manifest.id.clone(),
                version: instance.manifest.version.clone(),
                generation: instance.generation,
                commands: instance.commands.clone(),
            })
            .collect()
    }

    /// 基础停止：撤销注册、排空并释放实例（单线程串行下在途调用即已结束）。
    pub fn deactivate(&mut self, extension_id: &str) -> Result<TeardownReceipt, ExtensionError> {
        let Some(mut instance) = self.instances.remove(extension_id) else {
            return Err(ExtensionError::UnknownExtension(extension_id.to_string()));
        };
        instance.engine.shutdown_collect();
        Ok(TeardownReceipt {
            extension_id: instance.manifest.id.clone(),
            generation: instance.generation,
        })
    }

    /// 全部停止（宿主关闭路径）；逐一释放并报告未完成项。
    pub fn shutdown(&mut self) -> Result<(), ExtensionError> {
        let mut incomplete = Vec::new();
        let ids: Vec<String> = self.instances.keys().cloned().collect();
        for id in ids {
            if let Err(ExtensionError::ShutdownIncomplete(message)) = self.deactivate(&id) {
                incomplete.push(message);
            }
        }
        if incomplete.is_empty() {
            Ok(())
        } else {
            Err(ExtensionError::ShutdownIncomplete(incomplete.join("; ")))
        }
    }
}

// ---------- 跨边界值转换 ----------
//
// 进入引擎的值一律经引擎构造器登记（配额与回收参与）；离开引擎的值
// 浅拷贝为 owned 数据，不再被宿主持有。

fn extension_value_into_engine(
    engine: &mut engine::SchemeEngine,
    value: &ExtensionValue,
) -> Result<engine::Value, ExtensionError> {
    use engine::Value as EngineValue;
    match value {
        ExtensionValue::Null => Ok(EngineValue::Null),
        ExtensionValue::Bool(flag) => Ok(EngineValue::Bool(*flag)),
        ExtensionValue::Int(number) => Ok(EngineValue::Fixnum(*number)),
        ExtensionValue::Float(number) => {
            if number.is_finite() {
                Ok(EngineValue::Flonum(*number))
            } else {
                Err(ExtensionError::Argument(
                    "跨边界浮点值必须有限（拒绝 NaN/Inf）".to_string(),
                ))
            }
        }
        ExtensionValue::Text(text) => engine.new_string_from(text.clone())
            .map_err(|error| ExtensionError::from_engine(error)),
        ExtensionValue::Symbol(name) => Ok(EngineValue::Symbol(name.as_str().into())),
        ExtensionValue::Bytes(bytes) => engine.new_bytevector_from(bytes.clone())
            .map_err(|error| ExtensionError::from_engine(error)),
        ExtensionValue::List(items) => {
            let mut list = EngineValue::Null;
            for item in items.iter().rev() {
                let head = extension_value_into_engine(engine, item)?;
                list = engine
                    .new_pair(head, list)
                    .map_err(ExtensionError::from_engine)?;
            }
            Ok(list)
        }
    }
}

fn extension_value_from_engine(value: &engine::Value) -> Result<ExtensionValue, ExtensionError> {
    use engine::Value as EngineValue;
    match value {
        EngineValue::Null => Ok(ExtensionValue::Null),
        EngineValue::Bool(flag) => Ok(ExtensionValue::Bool(*flag)),
        EngineValue::Fixnum(number) => Ok(ExtensionValue::Int(*number)),
        EngineValue::Bignum(number) => Err(ExtensionError::Argument(format!(
            "精确整数 {number} 超出跨边界 i64 范围"
        ))),
        EngineValue::Rational(_) => Err(ExtensionError::Argument(
            "有理数不支持跨边界传递；先用宿主端口或数值转换".to_string(),
        )),
        EngineValue::Flonum(number) => {
            if number.is_finite() {
                Ok(ExtensionValue::Float(*number))
            } else {
                Err(ExtensionError::Argument(
                    "跨边界浮点值必须有限（拒绝 NaN/Inf）".to_string(),
                ))
            }
        }
        EngineValue::Char(character) => Ok(ExtensionValue::Text(character.to_string())),
        EngineValue::Symbol(name) => Ok(ExtensionValue::Symbol(name.to_string())),
        EngineValue::String(cell) => Ok(ExtensionValue::Text(cell.borrow().clone())),
        EngineValue::Bytevector(cell) => Ok(ExtensionValue::Bytes(cell.borrow().clone())),
        EngineValue::Pair(_) => {
            let mut items = Vec::new();
            let mut current = value.clone();
            loop {
                match current {
                    EngineValue::Null => return Ok(ExtensionValue::List(items)),
                    EngineValue::Pair(pair) => {
                        let borrowed = pair.borrow();
                        items.push(extension_value_from_engine(&borrowed.car)?);
                        current = borrowed.cdr.clone();
                    }
                    improper => {
                        let _ = improper;
                        return Err(ExtensionError::Argument(
                            "跨边界列表必须是严格列表".to_string(),
                        ));
                    }
                }
            }
        }
        EngineValue::Vector(items) => {
            let items = items.borrow();
            let mut converted = Vec::with_capacity(items.len());
            for item in items.iter() {
                converted.push(extension_value_from_engine(item)?);
            }
            Ok(ExtensionValue::List(converted))
        }
        EngineValue::Unspecified => Ok(ExtensionValue::Null),
        other => Err(ExtensionError::Argument(format!(
            "值 {} 不支持跨边界传递（闭包、环境、continuation、record、参数对象与端口不外泄）",
            other.to_write_string()
        ))),
    }
}
