//! 已编入原生构造器的公开 UI 适配；不根据官方包名猜测控件。
use super::*;
mod events;
use crate::ui::widget_runtime::{build_theme, build_viewport};
use crate::ui::{Container, ViewNode};
use events::MountedEvents;
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    sync::Arc,
};

/// 事件不在树构建闭包中执行，候选投影仍须使用最后一次挂载的窗口上下文。
#[derive(Clone)]
struct BuildEnvironment {
    provider: crate::ui::ProviderContext,
    theme: Arc<dyn crate::ui::ThemeTokens>,
    viewport: Option<f32>,
}
impl BuildEnvironment {
    fn current() -> Self {
        Self {
            provider: crate::ui::widget_runtime::provider_context::current_provider_context(),
            theme: build_theme::uix_effective_build_tokens(),
            viewport: build_viewport::current_build_viewport_width(),
        }
    }
    fn same(&self, other: &Self) -> bool {
        self.provider == other.provider
            && Arc::ptr_eq(&self.theme, &other.theme)
            && self.viewport == other.viewport
    }
    fn run<T>(&self, build: impl FnOnce() -> T) -> T {
        crate::ui::with_provider_context(&self.provider, || {
            let _theme = build_theme::BuildThemeScope::enter(self.theme.clone());
            let _viewport = self.viewport.map(build_viewport::BuildViewportScope::enter);
            build()
        })
    }
}

struct NativeCapture {
    state: Option<crate::ui::view::combinators::ScopedCaptureFrame>,
}
impl NativeCapture {
    fn begin() -> Self {
        build_viewport::begin_media_capture();
        Self {
            state: Some(crate::ui::view::combinators::ScopedCaptureFrame::begin()),
        }
    }
    fn finish(mut self, view: &mut ViewNode) {
        let output = self.state.take().unwrap().finish();
        view.captured_state_binds.extend(output.state_binds);
        view.captured_effects.extend(output.effects);
        view.captured_media_breakpoints
            .extend(build_viewport::end_media_capture());
        view.captured_viewport_width = build_viewport::current_build_viewport_width();
    }
}
impl Drop for NativeCapture {
    fn drop(&mut self) {
        if self.state.is_some() {
            let _ = build_viewport::end_media_capture();
        }
    }
}

type Factory = Rc<dyn Fn(&NativeNode, &UiBuildContext<'_>) -> RuntimeResult<ViewNode>>;
struct PreparedView {
    nodes: Vec<ViewNode>,
    dependencies: Vec<crate::ui::reactive::state::EffectDependency>,
}
impl PreparedView {
    fn is_current(&self) -> bool {
        self.dependencies
            .iter()
            .all(|dependency| dependency.source.generation() == dependency.observed_generation)
    }
}
#[derive(Clone, Default)]
pub struct UiLibrary {
    natives: NativeBindings,
    factories: BTreeMap<ExportKey, Factory>,
}
impl UiLibrary {
    pub fn new() -> Self {
        Self::default()
    }
    /// 属性声明同时决定检查接口和实际构造参数，不接收另一份手写签名。
    pub fn component<P: NativeProps>(
        mut self,
        package: &str,
        name: &str,
        build: impl Fn(P, &UiBuildContext<'_>) -> RuntimeResult<ViewNode> + 'static,
    ) -> RuntimeResult<Self> {
        let key = ExportKey::new(package, name);
        if self.natives.contains_key(&key) {
            return Err(invalid("原生导出重复"));
        }
        self.natives
            .insert(key.clone(), NativeBinding::Component(P::signature()));
        self.factories.insert(
            key,
            Rc::new(move |node, context| {
                build(P::read(&node.properties, context.events)?, context)
            }),
        );
        Ok(self)
    }
    pub fn function(
        mut self,
        key: ExportKey,
        signature: FunctionSignature,
        call: NativeCall,
    ) -> RuntimeResult<Self> {
        if self.natives.contains_key(&key) {
            return Err(invalid("原生导出重复"));
        }
        self.natives
            .insert(key, NativeBinding::Function { signature, call });
        Ok(self)
    }
    pub fn interfaces(&self) -> NativeLibraries {
        let mut libraries = NativeLibraries::new();
        for (key, native) in &self.natives {
            libraries
                .entry(key.package.clone())
                .or_default()
                .insert(key.name.clone(), native.interface());
        }
        libraries
    }
    pub fn native_bindings(&self) -> NativeBindings {
        self.natives.clone()
    }
    fn project(
        &self,
        snapshot: &Snapshot,
        events: &EventDispatcher,
    ) -> RuntimeResult<PreparedView> {
        let (nodes, dependencies) =
            crate::ui::reactive::state::collect_deps(|| self.nodes(&snapshot.roots, events));
        Ok(PreparedView {
            nodes: nodes?,
            dependencies,
        })
    }
    fn nodes(
        &self,
        nodes: &[NativeNode],
        events: &EventDispatcher,
    ) -> RuntimeResult<Vec<ViewNode>> {
        nodes
            .iter()
            .map(|node| {
                let factory = self.factories.get(&node.export).ok_or_else(|| {
                    RuntimeError::new(
                        ErrorKind::CapabilityDenied,
                        format!(
                            "原生构造器未编入：{}::{}",
                            node.export.package, node.export.name
                        ),
                    )
                    .at(&node.location)
                })?;
                let context = UiBuildContext {
                    library: self,
                    events,
                };
                let capture = NativeCapture::begin();
                let mut view = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    factory(node, &context)
                }))
                .map_err(|_| RuntimeError::new(ErrorKind::HostFailure, "原生控件构造 panic"))
                .and_then(|result| result)
                .map_err(|error| error.at(&node.location))?;
                capture.finish(&mut view);
                Ok(view.key(format!("{:?}/{:?}", node.identity, node.export)))
            })
            .collect()
    }
}
pub struct UiBuildContext<'a> {
    library: &'a UiLibrary,
    events: &'a EventDispatcher,
}
impl UiBuildContext<'_> {
    pub fn children(&self, children: Children) -> RuntimeResult<Vec<ViewNode>> {
        self.library.nodes(&children.0, self.events)
    }
}

/// 调用线程拥有的挂载控制器。只通知所属 State 订阅，不创建 worker 或轮询。
#[derive(Clone)]
pub struct UiHost(Rc<Host>);
struct Host {
    engine: RefCell<Option<Engine>>,
    library: RefCell<Option<Rc<UiLibrary>>>,
    pending: RefCell<Option<PreparedView>>,
    mounted_events: RefCell<MountedEvents>,
    environment: RefCell<Option<BuildEnvironment>>,
    changed: crate::ui::reactive::state::State<u64>,
    error: RefCell<Option<RuntimeError>>,
    closing: Cell<bool>,
    busy: Cell<bool>,
    cancellation: Cancellation,
    lease: RefCell<Weak<MountLease>>,
}
struct Busy<'a>(&'a Cell<bool>);
impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}
struct MountLease(Rc<Host>);
impl Drop for MountLease {
    fn drop(&mut self) {
        self.0.request_close();
    }
}
impl Host {
    fn request_close(&self) {
        self.closing.set(true);
        self.cancellation.cancel();
        self.finish_close();
    }
    fn finish_close(&self) {
        if !self.closing.get() {
            return;
        }
        if let Ok(mut engine) = self.engine.try_borrow_mut() {
            if let Some(engine) = engine.as_mut() {
                engine.close();
            }
            *engine = None;
            self.pending.borrow_mut().take();
            self.mounted_events.borrow_mut().clear();
            self.library.borrow_mut().take();
            self.environment.borrow_mut().take();
        }
    }
    fn closed(&self) -> RuntimeError {
        RuntimeError::new(ErrorKind::Closed, "组件 UI owner 已关闭或尚未初始化")
    }
    fn enter(&self) -> RuntimeResult<Busy<'_>> {
        if self.closing.get() {
            return Err(self.closed());
        }
        if self.busy.replace(true) {
            return Err(RuntimeError::new(
                ErrorKind::Conflict,
                "组件 UI 构建与事件不能重入同一 owner",
            ));
        }
        Ok(Busy(&self.busy))
    }
}
impl UiHost {
    /// 初始化语言 owner；原生控件在首次 view 的实际窗口上下文中构建。
    pub fn new(
        program: Program,
        library: UiLibrary,
        inputs: BTreeMap<String, Value>,
        limits: ComponentLimits,
    ) -> RuntimeResult<Self> {
        let library = Rc::new(library);
        let host = Self(Rc::new(Host {
            engine: RefCell::new(None),
            library: RefCell::new(Some(library.clone())),
            pending: RefCell::new(None),
            mounted_events: RefCell::new(MountedEvents::default()),
            environment: RefCell::new(None),
            changed: crate::ui::reactive::state::State::new(0),
            error: RefCell::new(None),
            closing: Cell::new(false),
            busy: Cell::new(false),
            cancellation: Cancellation::default(),
            lease: RefCell::new(Weak::new()),
        }));
        let engine = Engine::new(program, library.native_bindings(), inputs, limits)?;
        if host.0.closing.get() {
            return Err(host.0.closed());
        }
        *host.0.engine.borrow_mut() = Some(engine);
        Ok(host)
    }
    fn events(&self) -> EventDispatcher {
        let weak = Rc::downgrade(&self.0);
        EventDispatcher::new(move |token, arguments| {
            let host = weak
                .upgrade()
                .ok_or_else(|| RuntimeError::new(ErrorKind::Closed, "组件挂载面已释放"))?;
            UiHost(host)
                .dispatch_mounted(token, arguments)
                .map(|result| result.value)
        })
    }
    pub fn is_closed(&self) -> bool {
        self.0.closing.get()
    }
    pub fn close(&self) {
        self.0.request_close();
    }
    pub fn error(&self) -> Option<RuntimeError> {
        self.0.error.borrow().clone()
    }
    pub fn snapshot(&self) -> RuntimeResult<Snapshot> {
        let engine = self
            .0
            .engine
            .try_borrow()
            .map_err(|_| invalid("UI owner 正在执行，不允许重入读取"))?;
        engine
            .as_ref()
            .map(|engine| engine.snapshot().clone())
            .ok_or_else(|| self.0.closed())
    }
    fn operation<T>(
        &self,
        run: impl FnOnce(&mut Engine, &UiLibrary, &EventDispatcher) -> RuntimeResult<(T, PreparedView)>,
    ) -> RuntimeResult<T> {
        let _busy = self.0.enter()?;
        let result = (|| {
            let library = self
                .0
                .library
                .borrow()
                .clone()
                .ok_or_else(|| self.0.closed())?;
            let mut engine = self.0.engine.try_borrow_mut().map_err(|_| {
                RuntimeError::new(ErrorKind::Conflict, "组件 UI 事件不能重入同一 owner")
            })?;
            let engine = engine.as_mut().ok_or_else(|| self.0.closed())?;
            let environment = self
                .0
                .environment
                .borrow()
                .clone()
                .unwrap_or_else(BuildEnvironment::current);
            let (result, view) = environment.run(|| run(engine, &library, &self.events()))?;
            self.0
                .mounted_events
                .borrow_mut()
                .retain_live(engine.snapshot());
            *self.0.pending.borrow_mut() = Some(view);
            *self.0.environment.borrow_mut() = Some(environment);
            Ok(result)
        })();
        self.0.finish_close();
        if self.0.closing.get() {
            return Err(self.0.closed());
        }
        *self.0.error.borrow_mut() = result.as_ref().err().cloned();
        // 失败也重投影原权威值，恢复可能已被原生输入控件修改的临时草稿。
        self.0
            .changed
            .update(|revision| *revision = revision.wrapping_add(1));
        result
    }
    pub fn update(&self, inputs: BTreeMap<String, Value>) -> RuntimeResult<UpdateStats> {
        self.operation(|engine, library, events| {
            engine.update_with(inputs, &self.0.cancellation, |snapshot| {
                library.project(snapshot, events)
            })
        })
    }
    pub fn dispatch(
        &self,
        token: EventToken,
        arguments: Vec<Value>,
    ) -> RuntimeResult<DispatchResult> {
        self.operation(|engine, library, events| {
            engine.dispatch_with(token, arguments, &self.0.cancellation, |snapshot| {
                library.project(snapshot, events)
            })
        })
    }
    fn dispatch_mounted(
        &self,
        token: EventToken,
        arguments: Vec<Value>,
    ) -> RuntimeResult<DispatchResult> {
        self.operation(|engine, library, events| {
            let callback = self.0.mounted_events.borrow().get(token)?;
            engine.dispatch_mounted_with(callback, arguments, &self.0.cancellation, |snapshot| {
                library.project(snapshot, events)
            })
        })
    }
    /// 在所属 UI 捕获工厂内调用。挂载回执保活 owner，最后一个挂载面销毁时自动关闭。
    pub fn view(&self) -> RuntimeResult<ViewNode> {
        let _busy = self.0.enter()?;
        self.0.changed.get();
        let environment = BuildEnvironment::current();
        let pending = self.0.pending.borrow_mut().take().filter(|view| {
            view.is_current()
                && self
                    .0
                    .environment
                    .borrow()
                    .as_ref()
                    .is_some_and(|old| old.same(&environment))
        });
        let children = if let Some(children) = pending {
            children.nodes
        } else {
            let snapshot = self.snapshot()?;
            let library = self
                .0
                .library
                .borrow()
                .clone()
                .ok_or_else(|| self.0.closed())?;
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                library.project(&snapshot, &self.events())
            }))
            .map_err(|_| RuntimeError::new(ErrorKind::HostFailure, "原生控件重建 panic"))??
            .nodes
        };
        if self.0.closing.get() {
            return Err(self.0.closed());
        }
        *self.0.environment.borrow_mut() = Some(environment);
        {
            let engine = self.0.engine.borrow();
            *self.0.mounted_events.borrow_mut() =
                MountedEvents::capture(engine.as_ref().ok_or_else(|| self.0.closed())?)?;
        }
        let lease = {
            let mut weak = self.0.lease.borrow_mut();
            if let Some(lease) = weak.upgrade() {
                lease
            } else {
                let lease = Rc::new(MountLease(self.0.clone()));
                *weak = Rc::downgrade(&lease);
                lease
            }
        };
        let mut view = ViewNode::new(Container::new(), children);
        view.add_render_handlers([crate::ui::render_handler::RenderHandlerRegistration::new(
            lease,
        )]);
        Ok(view)
    }
}
