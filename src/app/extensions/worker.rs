//! 扩展执行线程：UI 应用模式下的串行执行器。
//!
//! 引擎（非 Send 的 `Rc` 值图）整体存活在本线程：宿主由本线程从提取的
//! Send 配置构造，准备 / 激活 / 调用 / 事件 / 异步终态都经命令通道投递，
//! UI 声明更新经 sink 回调交付（owned 数据）。UI 线程不执行任何 Lisp；
//! layout、paint 与命中路径无解释器参与。

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

use super::engine::Value as EngineValue;
use super::ui_declare::UiNode;
use super::ui_project::{UiEvent, UiEventPayload, UiUpdate};
use super::{
    ActivationReceipt, ExtensionError, ExtensionHost, ExtensionHostConfig, ExtensionPackage,
    ExtensionPort, ExtensionValue, PreparedExtension, ReplacementReceipt, TeardownReceipt,
    extension_value_from_engine,
};

/// 异步端口：宿主实现的非阻塞业务能力；终态经 `AsyncCompletion` 回投。
pub type ExtensionAsyncPort =
    Arc<dyn Fn(u64, &[ExtensionValue], AsyncCompletion) -> Result<(), String> + Send + Sync>;

/// 异步完成句柄：携带扩展身份、代与请求号；同一请求号只交付一次终态。
pub struct AsyncCompletion {
    commands: Sender<WorkerCommand>,
    extension: String,
    generation: u64,
    request: u64,
}

impl AsyncCompletion {
    pub fn complete(self, result: Result<ExtensionValue, String>) {
        let _ = self.commands.send(WorkerCommand::Complete {
            extension: self.extension,
            generation: self.generation,
            request: self.request,
            result,
        });
    }
}

/// UI 更新交付：worker 线程调用，应用负责回投到目标窗口 owner thread。
pub type ExtensionUiSink = Arc<dyn Fn(UiUpdate) + Send + Sync>;

/// 线程内准备完成的候选引用（候选持引擎，不可跨线程移动）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedRef(pub(crate) u64);

pub(crate) enum WorkerCommand {
    Prepare {
        package: ExtensionPackage,
        reply: Sender<Result<PreparedRef, ExtensionError>>,
    },
    Activate {
        prepared: PreparedRef,
        reply: Sender<Result<ActivationReceipt, ExtensionError>>,
    },
    Call {
        extension: String,
        command: String,
        namespace: &'static str,
        arguments: Vec<ExtensionValue>,
        reply: Sender<Result<ExtensionValue, ExtensionError>>,
    },
    Event(UiEvent),
    Replace {
        prepared: PreparedRef,
        expected_generation: u64,
        reply: Sender<Result<ReplacementReceipt, ExtensionError>>,
    },
    Revoke {
        extension: String,
        reply: Sender<Result<(), ExtensionError>>,
    },
    List {
        reply: Sender<Vec<super::ExtensionStatus>>,
    },
    Deactivate {
        extension: String,
        reply: Sender<Result<TeardownReceipt, ExtensionError>>,
    },
    Complete {
        extension: String,
        generation: u64,
        request: u64,
        result: Result<ExtensionValue, String>,
    },
    Shutdown(Sender<Result<(), ExtensionError>>),
}

/// 扩展宿主的线程安全句柄。
#[derive(Clone)]
pub struct ExtensionUiHandle {
    commands: Sender<WorkerCommand>,
    async_port_names: Arc<Vec<String>>,
}

impl ExtensionUiHandle {
    /// 在执行线程准备包（候选求值与暂存注册）。
    pub fn prepare(&self, package: &ExtensionPackage) -> Result<PreparedRef, ExtensionError> {
        let (reply, receiver) = std::sync::mpsc::channel();
        self.commands
            .send(WorkerCommand::Prepare {
                package: package.clone(),
                reply,
            })
            .map_err(closed())?;
        receiver.recv().map_err(closed())?
    }

    /// 发布候选为活动代。
    pub fn activate(&self, prepared: PreparedRef) -> Result<ActivationReceipt, ExtensionError> {
        let (reply, receiver) = std::sync::mpsc::channel();
        self.commands
            .send(WorkerCommand::Activate { prepared, reply })
            .map_err(closed())?;
        receiver.recv().map_err(closed())?
    }

    /// 类型化命令调用（阻塞至 worker 回执；语义与同步宿主一致）。
    pub fn call_command(
        &self,
        extension_id: &str,
        command: &str,
        arguments: &[ExtensionValue],
    ) -> Result<ExtensionValue, ExtensionError> {
        self.call(extension_id, command, "command", arguments)
    }

    /// 服务扩展点调用（`register-service!` 登记的实现）。
    pub fn call_service(
        &self,
        extension_id: &str,
        service: &str,
        arguments: &[ExtensionValue],
    ) -> Result<ExtensionValue, ExtensionError> {
        self.call(extension_id, service, "service", arguments)
    }

    fn call(
        &self,
        extension_id: &str,
        name: &str,
        namespace: &'static str,
        arguments: &[ExtensionValue],
    ) -> Result<ExtensionValue, ExtensionError> {
        let (reply, receiver) = std::sync::mpsc::channel();
        self.commands
            .send(WorkerCommand::Call {
                extension: extension_id.to_string(),
                command: name.to_string(),
                namespace,
                arguments: arguments.to_vec(),
                reply,
            })
            .map_err(closed())?;
        receiver.recv().map_err(closed())?
    }

    /// 热替换：以预期活动代升级（提交前失败保留旧代）。
    pub fn replace(
        &self,
        prepared: PreparedRef,
        expected_generation: u64,
    ) -> Result<ReplacementReceipt, ExtensionError> {
        let (reply, receiver) = std::sync::mpsc::channel();
        self.commands
            .send(WorkerCommand::Replace {
                prepared,
                expected_generation,
                reply,
            })
            .map_err(closed())?;
        receiver.recv().map_err(closed())?
    }

    /// 撤权：阻止新调用与事件；效果收尾责任在宿主。
    pub fn revoke(&self, extension_id: &str) -> Result<(), ExtensionError> {
        let (reply, receiver) = std::sync::mpsc::channel();
        self.commands
            .send(WorkerCommand::Revoke {
                extension: extension_id.to_string(),
                reply,
            })
            .map_err(closed())?;
        receiver.recv().map_err(closed())?
    }

    /// 活动实例列表（含撤权状态；worker 模式的状态观察入口）。
    pub fn list(&self) -> Result<Vec<super::ExtensionStatus>, ExtensionError> {
        let (reply, receiver) = std::sync::mpsc::channel();
        self.commands
            .send(WorkerCommand::List { reply })
            .map_err(closed())?;
        receiver.recv().map_err(closed())
    }

    /// 停止单个实例：撤销注册、排空并释放；重复停用返回 `UnknownExtension`。
    /// 挂载位子树的清空由应用在收到终态后执行。
    pub fn deactivate(&self, extension_id: &str) -> Result<TeardownReceipt, ExtensionError> {
        let (reply, receiver) = std::sync::mpsc::channel();
        self.commands
            .send(WorkerCommand::Deactivate {
                extension: extension_id.to_string(),
                reply,
            })
            .map_err(closed())?;
        receiver.recv().map_err(closed())?
    }

    /// 面板事件出口：投影闭包经此投递事件（只发送，不执行脚本）。
    pub fn event_sender(&self) -> super::ui_project::UiEventSender {
        super::ui_project::UiEventSender {
            inner: self.commands.clone(),
        }
    }

    /// 已注册的异步端口名（诊断视图）。
    pub fn async_port_names(&self) -> &[String] {
        &self.async_port_names
    }

    /// 停止全部实例并结束 worker 线程。
    pub fn shutdown(self) -> Result<(), ExtensionError> {
        let (reply, receiver) = std::sync::mpsc::channel();
        if self.commands.send(WorkerCommand::Shutdown(reply)).is_err() {
            return Ok(());
        }
        receiver.recv().unwrap_or(Ok(()))
    }
}

fn closed<E>() -> impl Fn(E) -> ExtensionError {
    |_| ExtensionError::ShutdownIncomplete("扩展执行线程已退出".to_string())
}

struct Worker {
    host: ExtensionHost,
    commands: Sender<WorkerCommand>,
    async_ports: Arc<BTreeMap<String, ExtensionAsyncPort>>,
    ui_sink: ExtensionUiSink,
    prepared: BTreeMap<u64, PreparedExtension>,
    next_prepared: u64,
    revisions: BTreeMap<String, u64>,
    next_request: u64,
}

impl Worker {
    fn run(mut self, inbox: Receiver<WorkerCommand>) {
        while let Ok(command) = inbox.recv() {
            match command {
                WorkerCommand::Prepare { package, reply } => {
                    let reply_value = match self.host.prepare(&package) {
                        Ok(prepared) => {
                            self.next_prepared += 1;
                            let reference = PreparedRef(self.next_prepared);
                            self.prepared.insert(self.next_prepared, prepared);
                            Ok(reference)
                        }
                        Err(error) => Err(error),
                    };
                    let _ = reply.send(reply_value);
                }
                WorkerCommand::Activate { prepared, reply } => {
                    let outcome = match self.prepared.remove(&prepared.0) {
                        Some(candidate) => self.host.activate(candidate),
                        None => Err(ExtensionError::ShutdownIncomplete(
                            "候选不存在或已激活".to_string(),
                        )),
                    };
                    // 入口顶层 submit-ui! 的初始声明在激活后交付。
                    if let Ok(receipt) = &outcome {
                        let id = receipt.extension_id.clone();
                        self.drain_side_effects(&id);
                    }
                    let _ = reply.send(outcome);
                }
                WorkerCommand::Call {
                    extension,
                    command,
                    namespace,
                    arguments,
                    reply,
                } => {
                    let outcome = self
                        .host
                        .call_internal(&extension, &command, namespace, &arguments);
                    self.drain_side_effects(&extension);
                    let _ = reply.send(outcome);
                }
                WorkerCommand::Event(event) => {
                    self.dispatch_event(event);
                }
                WorkerCommand::Replace {
                    prepared,
                    expected_generation,
                    reply,
                } => {
                    let candidate = self.prepared.remove(&prepared.0);
                    let outcome = match candidate {
                        Some(candidate) => {
                            let id = candidate
                                .manifest()
                                .map(|manifest| manifest.id.clone())
                                .unwrap_or_default();
                            let outcome = self.host.replace(candidate, expected_generation);
                            // 新代入口顶层 submit 的初始声明在提交后交付。
                            if let Ok(receipt) = &outcome {
                                let id = receipt.extension_id.clone();
                                self.drain_side_effects(&id);
                            }
                            let _ = id;
                            outcome
                        }
                        None => Err(ExtensionError::ShutdownIncomplete(
                            "候选不存在或已激活".to_string(),
                        )),
                    };
                    let _ = reply.send(outcome);
                }
                WorkerCommand::Revoke { extension, reply } => {
                    let outcome = self.host.revoke(&extension);
                    let _ = reply.send(outcome);
                }
                WorkerCommand::List { reply } => {
                    let _ = reply.send(self.host.list());
                }
                WorkerCommand::Deactivate { extension, reply } => {
                    // 单实例停用：撤销注册、排空并释放；挂载位清空由应用
                    // 在收到终态后执行（本 worker 不拥有任何窗口）。
                    let outcome = self.host.deactivate(&extension);
                    let _ = reply.send(outcome);
                }
                WorkerCommand::Complete {
                    extension,
                    generation,
                    request,
                    result,
                } => {
                    self.dispatch_completion(extension, generation, request, result);
                }
                WorkerCommand::Shutdown(reply) => {
                    let outcome = self.host.shutdown();
                    let _ = reply.send(outcome);
                    return;
                }
            }
        }
    }

    fn dispatch_event(&mut self, event: UiEvent) {
        let generation = match self.host.instance_mut(&event.extension) {
            Some(instance) => {
                if instance.revoked {
                    return;
                }
                instance.generation
            }
            None => return,
        };
        if generation != event.generation {
            // 陈旧代事件：稳定拒绝，不解释为新命令。
            return;
        }
        let payload: Vec<ExtensionValue> = match &event.payload {
            UiEventPayload::Click => Vec::new(),
            UiEventPayload::Change { text } | UiEventPayload::Submit { text } => {
                vec![ExtensionValue::Text(text.clone())]
            }
        };
        let arguments = match self.host.instance_mut(&event.extension) {
            Some(instance) => payload
                .iter()
                .filter_map(|value| instance.construct_owned(value).ok())
                .collect::<Vec<_>>(),
            None => return,
        };
        if let Some(instance) = self.host.instance_mut(&event.extension) {
            let _ = instance
                .engine
                .apply_registration("ui-event", &event.handler, &arguments);
        }
        self.drain_side_effects(&event.extension);
    }

    fn dispatch_completion(
        &mut self,
        extension: String,
        generation: u64,
        request: u64,
        result: Result<ExtensionValue, String>,
    ) {
        let Some(instance) = self.host.instance_mut(&extension) else {
            return;
        };
        if instance.revoked || instance.generation != generation {
            instance.engine.discard_async_handler(request);
            return;
        }
        let engine_result = match result {
            Ok(value) => match instance.construct_owned(&value) {
                Ok(engine_value) => Ok(engine_value),
                Err(error) => Err(engine_host_error(&error)),
            },
            Err(message) => Err(engine_host_error(&ExtensionError::HostFailure(message))),
        };
        let _ = instance.engine.complete_async(request, engine_result);
        self.drain_side_effects(&extension);
    }

    /// 处理 UI 提交与异步请求，直到本轮无新副作用。
    fn drain_side_effects(&mut self, extension: &str) {
        loop {
            let submissions = match self.host.instance_mut(extension) {
                Some(instance) => instance.engine.take_ui_submissions(),
                None => return,
            };
            if submissions.is_empty() {
                break;
            }
            let generation = self
                .host
                .instance_mut(extension)
                .map(|instance| instance.generation)
                .unwrap_or(0);
            for (mount, declaration) in submissions {
                let revision = self.revisions.entry(mount.clone()).or_insert(0);
                *revision += 1;
                let update = match UiNode::parse(&declaration) {
                    Ok(node) => UiUpdate::Applied {
                        mount,
                        extension: extension.to_string(),
                        generation,
                        revision: *revision,
                        node,
                    },
                    Err(error) => UiUpdate::Rejected {
                        mount,
                        extension: extension.to_string(),
                        reason: error.to_string(),
                    },
                };
                (self.ui_sink)(update);
            }
        }
        loop {
            let requests = match self.host.instance_mut(extension) {
                Some(instance) => instance.engine.take_async_requests(),
                None => return,
            };
            if requests.is_empty() {
                break;
            }
            let generation = self
                .host
                .instance_mut(extension)
                .map(|instance| instance.generation)
                .unwrap_or(0);
            for (port, arguments, handler) in requests {
                self.next_request += 1;
                let request = self.next_request;
                if let Some(instance) = self.host.instance_mut(extension) {
                    instance.engine.store_async_handler(request, handler);
                }
                let completion = AsyncCompletion {
                    commands: self.commands.clone(),
                    extension: extension.to_string(),
                    generation,
                    request,
                };
                let outcome: Result<(), String> = match self.async_ports.get(&port) {
                    Some(implementation) => arguments
                        .iter()
                        .map(|value| extension_value_from_engine(value))
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|error| error.to_string())
                        .and_then(|owned| implementation(request, &owned, completion)),
                    None => Err("异步端口不可用".to_string()),
                };
                if let Err(message) = outcome {
                    // 发起失败即终态：同步完成错误并继续排空。
                    self.dispatch_completion(
                        extension.to_string(),
                        generation,
                        request,
                        Err(message),
                    );
                }
            }
        }
    }
}

fn engine_host_error(error: &ExtensionError) -> super::engine::SchemeError {
    super::engine::SchemeError::HostFunctionError {
        name: "async-port".to_string(),
        message: error.to_string(),
    }
}

impl ExtensionHost {
    /// 注册异步端口（与同步端口同名的授权面）。
    pub fn with_async_port(mut self, name: &str, port: ExtensionAsyncPort) -> Self {
        self.async_ports.insert(name.to_string(), port);
        self
    }

    /// 注册 UI 更新交付回调。
    pub fn with_ui_sink(mut self, sink: ExtensionUiSink) -> Self {
        self.ui_sink = Some(sink);
        self
    }

    /// 进入 UI 应用模式：提取 Send 配置并在独立执行线程重建宿主。
    ///
    /// 要求尚未装载实例（引擎值图不可跨线程移动）；返回线程安全句柄。
    pub fn spawn_worker(self) -> ExtensionUiHandle {
        assert!(
            self.instances.is_empty(),
            "spawn_worker 要求宿主尚未装载实例"
        );
        let ExtensionHost {
            config,
            ports,
            async_ports,
            mounts,
            ui_sink,
            instances: _,
            generation_counter: _,
        } = self;
        let ui_sink = ui_sink.unwrap_or_else(|| Arc::new(|_| {}));
        let host_async_ports = async_ports.clone();
        let async_ports = Arc::new(async_ports);
        let port_names: Arc<Vec<String>> = Arc::new(async_ports.keys().cloned().collect());
        let (commands, inbox) = std::sync::mpsc::channel();
        let worker_commands = commands.clone();
        let worker_ports = Arc::clone(&async_ports);
        let worker_sink = Arc::clone(&ui_sink);
        std::thread::Builder::new()
            .name("uix-extension-worker".to_string())
            .spawn(move || {
                let mut host = ExtensionHost::new().with_config(config);
                for (name, port) in ports {
                    host = host.with_port(&name, port);
                }
                for name in mounts {
                    host = host.with_mount(&name);
                }
                for (name, port) in host_async_ports {
                    host = host.with_async_port(&name, port);
                }
                let worker = Worker {
                    host,
                    commands: worker_commands,
                    async_ports: worker_ports,
                    ui_sink: worker_sink,
                    prepared: BTreeMap::new(),
                    next_prepared: 0,
                    revisions: BTreeMap::new(),
                    next_request: 0,
                };
                worker.run(inbox);
            })
            // 契约：线程创建仅因系统资源耗尽失败，属宿主启动边界。
            .expect("扩展执行线程创建失败");
        ExtensionUiHandle {
            commands,
            async_port_names: port_names,
        }
    }
}
