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
mod ui_declare;
pub(crate) mod ui_project;
mod worker;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub use manifest::ExtensionManifest;
pub use package::ExtensionPackage;
pub use ui_declare::UiNode;
pub use ui_project::{UiEvent, UiEventPayload, UiEventSender, UiProjector, UiUpdate};
pub use worker::{
    AsyncCompletion, ExtensionAsyncPort, ExtensionUiHandle, ExtensionUiSink,
};

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

/// 热替换终态：新代已提交，状态按合同迁移。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementReceipt {
    pub extension_id: String,
    pub version: String,
    pub generation: u64,
    pub commands: Vec<String>,
    /// 是否执行了状态导出与迁移（双方注册且 schema 兼容）。
    pub migrated: bool,
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
    /// 撤权后新调用与事件拒绝，实例保留至停用。
    revoked: bool,
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
    async_ports: BTreeMap<String, ExtensionAsyncPort>,
    /// 应用显式开放的挂载位白名单（能力名 `mount-<name>`）。
    mounts: BTreeSet<String>,
    ui_sink: Option<ExtensionUiSink>,
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
            async_ports: BTreeMap::new(),
            mounts: BTreeSet::new(),
            ui_sink: None,
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

    /// 开放挂载位：扩展清单以 `mount-<name>` 能力声明后可提交该区域声明。
    pub fn with_mount(mut self, name: &str) -> Self {
        self.mounts.insert(name.to_string());
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
            // 挂载位与异步端口不注入同步函数，但计入能力集。
            if let Some(name) = declared.strip_prefix("mount-") {
                if !self.mounts.contains(name) {
                    return Err(ExtensionError::Incompatible(format!(
                        "清单声明挂载位 {name}，宿主未开放"
                    )));
                }
                capabilities.insert(declared.clone());
                continue;
            }
            if self.async_ports.contains_key(declared) {
                capabilities.insert(declared.clone());
                continue;
            }
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
                revoked: false,
            },
        );
        Ok(receipt)
    }

    /// 热替换：候选升级活动代；提交前失败保留旧代并恢复接收。
    ///
    /// 状态迁移合同：双方 `state-schema-version` 一致且都注册状态过程时
    /// 导出快照交给候选导入；schema 不一致或导入失败则替换被拒绝，
    /// 旧代不受影响。在途异步终态经代际校验自动丢弃，不写入新代。
    pub fn replace(
        &mut self,
        prepared: PreparedExtension,
        expected_generation: u64,
    ) -> Result<ReplacementReceipt, ExtensionError> {
        let manifest = prepared.package.manifest()?;
        let Some(instance) = self.instances.get(&manifest.id) else {
            return Err(ExtensionError::UnknownExtension(manifest.id.clone()));
        };
        if instance.revoked {
            return Err(ExtensionError::CapabilityDenied(format!(
                "扩展 {} 已撤权",
                manifest.id
            )));
        }
        if instance.generation != expected_generation {
            return Err(ExtensionError::StaleGeneration {
                expected: expected_generation,
                actual: instance.generation,
            });
        }
        // 1) 静止点导出旧状态（同步宿主无在途调用；worker 模式由命令
        //    串行化门控）。
        let old_schema = instance.manifest.state_schema_version;
        let has_old_export = instance
            .engine
            .host_registrations()
            .iter()
            .any(|registration| registration.namespace == "state-export");
        let has_new_import = prepared
            .engine
            .host_registrations()
            .iter()
            .any(|registration| registration.namespace == "state-import");
        let snapshot = if old_schema == 0 && manifest.state_schema_version == 0 {
            None
        } else if old_schema != manifest.state_schema_version {
            return Err(ExtensionError::Incompatible(format!(
                "状态 schema 版本不一致：活动代 {old_schema}，候选 {}；不兼容且无迁移时拒绝替换",
                manifest.state_schema_version
            )));
        } else if has_old_export && has_new_import {
            Some(self.export_state(&manifest.id)?)
        } else {
            // schema > 0 但缺少状态过程：静默清空被合同禁止，拒绝替换。
            return Err(ExtensionError::Incompatible(format!(
                "状态 schema {old_schema} 非零但双方未同时注册状态过程"
            )));
        };
        // 2) 候选导入（失败丢弃候选，旧代保留并继续接收）。
        //    迁移路径下丢弃准备期暂存的初始声明：迁移过程拥有状态
        //    解释权，负责以导入后状态重新提交（外部效果合同不变）。
        let mut candidate = prepared.engine;
        let migrated = snapshot.is_some();
        if let Some(snapshot) = snapshot {
            candidate.discard_pending_ui();
            import_state_into(&mut candidate, snapshot)?;
        }
        // 3) 提交：释放旧代，发布新代。
        let commands: Vec<String> = candidate
            .host_registrations()
            .iter()
            .filter(|registration| registration.namespace == "command")
            .map(|registration| registration.name.clone())
            .collect();
        if let Some(mut old) = self.instances.remove(&manifest.id) {
            old.engine.shutdown_collect();
        }
        self.generation_counter += 1;
        let generation = self.generation_counter;
        let receipt = ReplacementReceipt {
            extension_id: manifest.id.clone(),
            version: manifest.version.clone(),
            generation,
            commands: commands.clone(),
            migrated,
        };
        let cancel = engine::CancelToken::new();
        candidate.rebind_cancel(cancel.clone());
        candidate.set_command_wall_time_ms(self.config.command_wall_time_ms);
        self.instances.insert(
            manifest.id.clone(),
            ActiveInstance {
                generation,
                manifest,
                engine: candidate,
                commands,
                cancel,
                revoked: false,
            },
        );
        Ok(receipt)
    }

    /// 导出扩展私有状态快照（owned 值）。
    fn export_state(&mut self, extension_id: &str) -> Result<ExtensionValue, ExtensionError> {
        let Some(instance) = self.instances.get_mut(extension_id) else {
            return Err(ExtensionError::UnknownExtension(extension_id.to_string()));
        };
        let value = instance
            .engine
            .call_registration("state-export", "state", &[])
            .map_err(ExtensionError::from_engine)?;
        extension_value_from_engine(&value)
    }

    /// 撤权：阻止新调用、事件与声明提交；已发生或不能取消的效果由
    /// 宿主负责收尾。实例保留至停用（可观察状态）。
    pub fn revoke(&mut self, extension_id: &str) -> Result<(), ExtensionError> {
        let Some(instance) = self.instances.get_mut(extension_id) else {
            return Err(ExtensionError::UnknownExtension(extension_id.to_string()));
        };
        instance.revoked = true;
        instance.cancel.cancel();
        Ok(())
    }

    /// 类型化命令调用；实例串行复用，扩展状态在调用间保留。
    pub fn call_command(
        &mut self,
        extension_id: &str,
        command: &str,
        arguments: &[ExtensionValue],
    ) -> Result<ExtensionValue, ExtensionError> {
        self.call_internal(extension_id, command, "command", arguments)
    }

    /// 服务扩展点调用（`register-service!` 登记的实现）。
    pub fn call_service(
        &mut self,
        extension_id: &str,
        service: &str,
        arguments: &[ExtensionValue],
    ) -> Result<ExtensionValue, ExtensionError> {
        self.call_internal(extension_id, service, "service", arguments)
    }

    /// 内部统一调用入口（worker 与同步门面共用）。
    fn call_internal(
        &mut self,
        extension_id: &str,
        name: &str,
        namespace: &'static str,
        arguments: &[ExtensionValue],
    ) -> Result<ExtensionValue, ExtensionError> {
        let Some(instance) = self.instances.get_mut(extension_id) else {
            return Err(ExtensionError::UnknownExtension(extension_id.to_string()));
        };
        if instance.revoked {
            return Err(ExtensionError::CapabilityDenied(format!(
                "扩展 {extension_id} 已撤权"
            )));
        }
        let known = match namespace {
            "command" => instance.commands.iter().any(|existing| existing == name),
            "service" => instance
                .engine
                .host_registrations()
                .iter()
                .any(|registration| {
                    registration.namespace == "service" && registration.name == name
                }),
            _ => false,
        };
        if !known {
            return Err(ExtensionError::UnknownCommand {
                extension: extension_id.to_string(),
                command: name.to_string(),
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
            .call_registration(namespace, name, &engine_arguments);
        outcome
            .map_err(ExtensionError::from_engine)
            .and_then(|value| extension_value_from_engine(&value))
    }

    /// 活动实例的可变视图（worker 事件与终态派发）。
    fn instance_mut(&mut self, extension_id: &str) -> Option<&mut ActiveInstance> {
        self.instances.get_mut(extension_id)
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

/// 候选导入状态快照：值经候选引擎构造器登记后交给导入过程。
fn import_state_into(
    engine: &mut engine::SchemeEngine,
    snapshot: ExtensionValue,
) -> Result<(), ExtensionError> {
    let value = extension_value_into_engine(engine, &snapshot)?;
    engine
        .apply_registration("state-import", "state", &[value])
        .map_err(ExtensionError::from_engine)?;
    Ok(())
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
