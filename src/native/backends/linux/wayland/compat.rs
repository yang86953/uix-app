//! Small compatibility layer for the Wayland 0.31 dispatch model.
//!
//! The backend historically used `Main<T>::quick_assign` from wayland-client
//! 0.29.  Wayland 0.31 deliberately moved callback ownership into
//! `Dispatch<State>`.  This module keeps the backend's callback-oriented
//! structure while routing every callback through a typed 0.31 event queue.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::ops::Deref;
use std::os::fd::BorrowedFd;
use std::os::raw::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex};

use wayland_client::backend::ObjectId;
use wayland_client::globals::GlobalListContents;
use wayland_client::protocol::{
    wl_buffer, wl_callback, wl_compositor, wl_data_device, wl_data_device_manager, wl_data_offer,
    wl_data_source, wl_keyboard, wl_pointer, wl_region, wl_registry, wl_seat, wl_shm, wl_shm_pool,
    wl_surface,
};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_manager_v3::ZwpTextInputManagerV3;
use wayland_protocols::xdg::activation::v1::client::{
    xdg_activation_token_v1::XdgActivationTokenV1, xdg_activation_v1::XdgActivationV1,
};
use wayland_protocols::xdg::decoration::zv1::client::{
    zxdg_decoration_manager_v1::ZxdgDecorationManagerV1,
    zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1,
};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

use crate::core::{Errc, Error};
use crate::diagnostics::PendingFailureSource;

type Callback<I> =
    Box<dyn FnMut(&Main<I>, <I as Proxy>::Event, &QueueHandle<WaylandDispatchState>) + 'static>;

// 统一描述 callback registry 的稳定协议对象键。
type CallbackKey = (TypeId, u32);
// 统一保存异构 Wayland callback 的擦除后 owner。
type StoredCallback = Box<dyn Any>;

// 让一个 Component 独占 callback map 与失败传播边界。
#[derive(Clone)]
struct CallbackRegistry {
    // 仅由本 Component 访问异构 callback owner。
    callbacks: Arc<Mutex<HashMap<CallbackKey, StoredCallback>>>,
    // 把 registry 损坏转换为既有 owner-thread failure source。
    pending_failures: PendingFailureSource,
}

// 实现 callback registry 的检查式生命周期操作。
impl CallbackRegistry {
    // 建立空 registry 并绑定唯一 runtime failure source。
    fn new(pending_failures: PendingFailureSource) -> Self {
        // 返回同时拥有 callback map 与 failure adapter 的 Component。
        Self {
            // 初始化尚未登记协议 callback 的唯一 map。
            callbacks: Arc::new(Mutex::new(HashMap::new())),
            // 保存来自同一 Wayland backend 的 failure source。
            pending_failures,
        }
    }

    // 从协议类型和对象编号生成稳定 registry 键。
    fn key<I: Proxy + 'static>(proxy: &I) -> CallbackKey {
        // 类型身份隔离不同协议接口复用的对象编号。
        (TypeId::of::<I>(), proxy.id().protocol_id())
    }

    // 检查式登记或回插一个 callback owner。
    fn insert(
        // 借用唯一 registry Component。
        &self,
        // 指定 callback 所属协议对象键。
        key: CallbackKey,
        // 转移擦除后的 callback 唯一 owner。
        callback: StoredCallback,
        // 保留本次注册生命周期阶段。
        operation: &'static str,
    ) -> bool {
        // 中毒 map 不再通过 into_inner 恢复访问。
        let Ok(mut callbacks) = self.callbacks.lock() else {
            // 将损坏事实交给 owner-thread failure queue。
            self.report_registry_lock_failure(operation);
            // 告知 adapter 本次 callback owner 未被登记。
            return false;
        };
        // 健康 registry 原子接管 callback owner。
        callbacks.insert(key, callback);
        // 告知 adapter callback owner 已经登记。
        true
    }

    // 检查式取出或注销一个 callback owner。
    fn remove(
        // 借用唯一 registry Component。
        &self,
        // 指定 callback 所属协议对象键。
        key: &CallbackKey,
        // 保留本次注册生命周期阶段。
        operation: &'static str,
    ) -> Option<StoredCallback> {
        // 中毒 map 不再通过 into_inner 恢复访问。
        let Ok(mut callbacks) = self.callbacks.lock() else {
            // 将损坏事实交给 owner-thread failure queue。
            self.report_registry_lock_failure(operation);
            // 损坏 registry 不得交付任何 callback owner。
            return None;
        };
        // 健康 registry 转移精确对象的 callback owner。
        callbacks.remove(key)
    }

    // 检查擦除 callback 是否匹配当前 Wayland 协议接口。
    fn downcast_callback<I>(&self, callback: StoredCallback) -> Option<Callback<I>>
    // 仅 Wayland 协议代理能够声明 callback Event 类型。
    where
        // 类型身份必须在 registry 生命周期内稳定。
        I: Proxy + 'static,
        // callback event 必须能够存入静态闭包。
        I::Event: 'static,
    {
        // 区分健康类型 owner 与 registry 不变量损坏。
        match callback.downcast::<Callback<I>>() {
            // 将双层 downcast owner 收敛回规范 Callback<I> trait object。
            Ok(callback) => {
                // 显式触发 FnMut trait object coercion，避免回插额外 Box 层。
                let callback: Callback<I> = callback;
                // 交付与接口匹配的唯一 callback owner。
                Some(callback)
            }
            // 类型错配表示同一 key 下的 registry 状态已经损坏。
            Err(_) => {
                // 向 owner thread 报告稳定 InvalidState，不把损坏伪装成无 callback。
                self.enqueue_failure(Error::new(
                    // registry 不变量损坏属于稳定状态错误。
                    Errc::InvalidState,
                    // 保留发生错配的协议接口名。
                    format!(
                        // 生成可定位 callback registry 阶段的诊断。
                        "Wayland {} callback registry type mismatch",
                        // 使用 wayland-client 提供的稳定接口名。
                        I::interface().name,
                    ),
                ));
                // 隔离损坏 entry，阻止其在后续事件中反复冒充合法 callback。
                None
            }
        }
    }

    // 报告 callback 执行 panic，但不越过 Wayland dispatch ABI。
    fn report_callback_panic<I>(&self)
    // 仅 Wayland 协议代理能够提供稳定接口名。
    where
        // 类型身份用于生成精确 callback 诊断。
        I: Proxy + 'static,
    {
        // 将 panic 转换为平台 typed failure。
        self.enqueue_failure(Error::new(
            // callback panic 属于平台边界失败。
            Errc::PlatformError,
            // 保留发生 panic 的协议接口名。
            format!("Wayland {} callback panicked", I::interface().name),
        ));
    }

    // 报告 callback map 锁中毒并保留精确生命周期阶段。
    fn report_registry_lock_failure(&self, operation: &'static str) {
        // 将不可恢复的 registry 状态转换为 owner-thread typed failure。
        self.enqueue_failure(Error::new(
            // 中毒 registry 无法继续安全访问。
            Errc::InvalidState,
            // 保留 register/unregister/take/reinsert 精确阶段。
            format!("Wayland callback registry mutex poisoned during {operation}"),
        ));
    }

    // 将 registry failure 送入既有 bounded owner-thread source。
    fn enqueue_failure(&self, error: Error) {
        // source 已关闭时不存在更高层 receiver，保留既有 fail-closed 语义。
        let _ = self.pending_failures.enqueue(error);
    }
}

// 允许协议代理共享同一个 callback registry owner。
#[derive(Clone)]
pub(crate) struct ProxyContext {
    // 克隆仅共享同一个 registry Component，不复制 map。
    registry: CallbackRegistry,
    queue_handle: Option<QueueHandle<WaylandDispatchState>>,
}

impl ProxyContext {
    pub(crate) fn new(
        queue_handle: QueueHandle<WaylandDispatchState>,
        pending_failures: PendingFailureSource,
    ) -> Self {
        Self {
            registry: CallbackRegistry::new(pending_failures),
            queue_handle: Some(queue_handle),
        }
    }

    fn queue_handle(&self) -> &QueueHandle<WaylandDispatchState> {
        self.queue_handle
            .as_ref()
            .expect("Wayland proxy context must have a queue handle")
    }

    fn with_registry(
        registry: CallbackRegistry,
        queue_handle: QueueHandle<WaylandDispatchState>,
    ) -> Self {
        Self {
            registry,
            queue_handle: Some(queue_handle),
        }
    }

    fn register<I, F>(&self, proxy: &I, callback: F)
    where
        I: Proxy + 'static,
        I::Event: 'static,
        F: FnMut(&Main<I>, I::Event, &QueueHandle<WaylandDispatchState>) + 'static,
    {
        // 把具体闭包收敛为规范的 Wayland callback trait object。
        let callback = Box::new(callback) as Callback<I>;
        // registry 接管 callback owner；失败已经同步进入 pending source。
        let _ = self.registry.insert(
            // 使用协议类型和对象编号形成稳定键。
            CallbackRegistry::key(proxy),
            // 擦除具体接口类型后转移唯一 owner。
            Box::new(callback),
            // 保留初次登记阶段。
            "registration",
        );
    }

    // 注销指定协议对象的旧式回调适配器。
    fn unregister<I>(&self, proxy: &I)
    // 仅协议代理能够形成稳定回调键。
    where
        // 类型身份参与回调表索引，因此要求静态生命周期。
        I: Proxy + 'static,
        // 开始回调注销实现。
    {
        // 检查式删除对象对应的 callback owner，重复注销保持幂等。
        let _ = self.registry.remove(
            // 使用协议类型和对象编号形成稳定键。
            &CallbackRegistry::key(proxy),
            // 保留主动注销阶段。
            "unregistration",
        );
        // 结束回调注销实现。
    }
}

/// A 0.31 event queue state used by the legacy callback adapters.
pub(crate) struct WaylandDispatchState {
    // dispatch 与 ProxyContext 共享同一个 callback registry Component。
    registry: CallbackRegistry,
}

impl WaylandDispatchState {
    pub(crate) fn from_context(context: &ProxyContext) -> Self {
        Self {
            // 克隆句柄只共享 owner，不复制 callback map 或 failure source。
            registry: context.registry.clone(),
        }
    }

    fn dispatch<I>(&mut self, proxy: &I, event: I::Event, qh: &QueueHandle<Self>)
    where
        I: Proxy + 'static,
        I::Event: 'static,
    {
        // 解析本次协议事件唯一对应的 callback registry 键。
        let key = CallbackRegistry::key(proxy);
        // 检查式取出 callback，使执行期间不持有 registry mutex。
        let callback = self.registry.remove(&key, "dispatch take");

        // 未登记 callback 或 registry 损坏时不执行协议用户闭包。
        let Some(callback) = callback else {
            // registry 损坏已经进入 pending failure source。
            return;
        };

        // 检查擦除 owner 与当前协议接口是否一致。
        let Some(mut callback) = self.registry.downcast_callback::<I>(callback) else {
            // 类型错配已经入队并隔离损坏 entry。
            return;
        };
        // 构造只共享同一 registry 与 queue handle 的 callback proxy。
        let callback_proxy = Main::new(
            // callback 期间借用同一协议代理身份。
            proxy.clone(),
            // 子 callback 注册继续进入同一个 registry Component。
            ProxyContext::with_registry(self.registry.clone(), qh.clone()),
        );
        // panic 只在本 dispatch adapter 内转换，不能越过 Wayland ABI。
        let callback_result = catch_unwind(AssertUnwindSafe(|| {
            // 执行本次协议事件对应的唯一 callback。
            callback(&callback_proxy, event, qh);
        }));
        // callback panic 必须形成 typed failure，但仍允许健康 registry 保留 callback。
        if callback_result.is_err() {
            // 把平台 panic 送到同一 owner-thread failure source。
            self.registry.report_callback_panic::<I>();
        }
        // 无论 callback 成功或 panic，都把仍存活的 owner 放回健康 registry。
        let _ = self.registry.insert(
            // 回插到本次 dispatch 取出的精确协议对象键。
            key,
            // 按规范 Callback<I> 形状擦除一次，禁止双 Box 类型漂移。
            Box::new(callback),
            // 保留回插阶段，锁中毒时生成精确诊断。
            "callback reinsert",
        );
    }
}

impl<I> Dispatch<I, ()> for WaylandDispatchState
where
    I: Proxy + 'static,
    I::Event: 'static,
{
    fn event(
        state: &mut Self,
        proxy: &I,
        event: I::Event,
        _data: &(),
        _connection: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        state.dispatch(proxy, event, qh);
    }

    fn event_created_child(
        opcode: u16,
        qhandle: &QueueHandle<Self>,
    ) -> Arc<dyn wayland_client::backend::ObjectData> {
        if TypeId::of::<I>() == TypeId::of::<wl_data_device::WlDataDevice>() && opcode == 0 {
            return qhandle.make_data::<wl_data_offer::WlDataOffer, _>(());
        }
        panic!(
            "missing event-created child dispatch for opcode {opcode} on {}",
            I::interface().name
        );
    }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for WaylandDispatchState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_registry::WlRegistry,
        _event: wl_registry::Event,
        _data: &GlobalListContents,
        _connection: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

/// Proxy wrapper that supplies the shared 0.31 queue handle to callbacks and
/// child-object constructors.
pub(crate) struct Main<I> {
    proxy: I,
    context: ProxyContext,
}

impl<I: Clone> Clone for Main<I> {
    fn clone(&self) -> Self {
        Self {
            proxy: self.proxy.clone(),
            context: self.context.clone(),
        }
    }
}

impl<I: std::fmt::Debug> std::fmt::Debug for Main<I> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_tuple("Main").field(&self.proxy).finish()
    }
}

impl<I> Main<I> {
    pub(crate) fn new(proxy: I, context: ProxyContext) -> Self {
        Self { proxy, context }
    }

    pub(crate) fn child<T>(&self, proxy: T) -> Main<T> {
        Main::new(proxy, self.context.clone())
    }

    pub(crate) fn queue_handle(&self) -> QueueHandle<WaylandDispatchState> {
        self.context.queue_handle().clone()
    }

    pub(crate) fn context(&self) -> ProxyContext {
        self.context.clone()
    }
}

impl<I: Proxy> Main<I> {
    pub(crate) fn quick_assign<F>(&self, callback: F)
    where
        I: 'static,
        I::Event: 'static,
        F: FnMut(&Main<I>, I::Event, &QueueHandle<WaylandDispatchState>) + 'static,
    {
        self.context.register(&self.proxy, callback);
    }

    pub(crate) fn id(&self) -> ObjectId {
        self.proxy.id()
    }

    pub(crate) fn c_ptr(&self) -> *mut c_void {
        self.proxy.id().as_ptr().cast()
    }

    // 在协议代理失效前删除兼容层保存的回调闭包。
    pub(crate) fn clear_callback(&self)
    // 回调键需要代理类型的静态身份。
    where
        // 限制仅作用于本次注销方法。
        I: 'static,
        // 开始回调清理实现。
    {
        // 委托共享上下文删除精确对象回调。
        self.context.unregister(&self.proxy);
        // 结束回调清理实现。
    }
}

impl<I> AsRef<I> for Main<I> {
    fn as_ref(&self) -> &I {
        &self.proxy
    }
}

impl<I> Deref for Main<I> {
    type Target = I;

    fn deref(&self) -> &Self::Target {
        &self.proxy
    }
}

impl Main<wl_compositor::WlCompositor> {
    pub(crate) fn create_surface(&self) -> Main<wl_surface::WlSurface> {
        let proxy = self.proxy.create_surface(&self.queue_handle(), ());
        self.child(proxy)
    }

    pub(crate) fn create_region(&self) -> Main<wl_region::WlRegion> {
        let proxy = self.proxy.create_region(&self.queue_handle(), ());
        self.child(proxy)
    }
}

impl Main<wl_shm::WlShm> {
    pub(crate) fn create_pool(&self, fd: i32, size: i32) -> Main<wl_shm_pool::WlShmPool> {
        // The request sends the descriptor immediately; the borrowed lifetime
        // only needs to cover this call.
        // SAFETY: fd 由调用方在本次同步协议请求期间保持打开，本借用不接管也不关闭该描述符。
        let fd = unsafe { BorrowedFd::borrow_raw(fd) };
        let proxy = self.proxy.create_pool(fd, size, &self.queue_handle(), ());
        self.child(proxy)
    }
}

impl Main<wl_shm_pool::WlShmPool> {
    pub(crate) fn create_buffer(
        &self,
        offset: i32,
        width: i32,
        height: i32,
        stride: i32,
        format: wl_shm::Format,
    ) -> Main<wl_buffer::WlBuffer> {
        let proxy = self.proxy.create_buffer(
            offset,
            width,
            height,
            stride,
            format,
            &self.queue_handle(),
            (),
        );
        self.child(proxy)
    }
}

impl Main<wl_surface::WlSurface> {
    pub(crate) fn frame(&self) -> Main<wl_callback::WlCallback> {
        let proxy = self.proxy.frame(&self.queue_handle(), ());
        self.child(proxy)
    }
}

impl Main<xdg_wm_base::XdgWmBase> {
    pub(crate) fn get_xdg_surface(
        &self,
        surface: &Main<wl_surface::WlSurface>,
    ) -> Main<xdg_surface::XdgSurface> {
        let proxy = self
            .proxy
            .get_xdg_surface(&surface.proxy, &self.queue_handle(), ());
        self.child(proxy)
    }
}

impl Main<xdg_surface::XdgSurface> {
    pub(crate) fn get_toplevel(&self) -> Main<xdg_toplevel::XdgToplevel> {
        let proxy = self.proxy.get_toplevel(&self.queue_handle(), ());
        self.child(proxy)
    }
}

impl Main<wl_seat::WlSeat> {
    pub(crate) fn get_pointer(&self) -> Main<wl_pointer::WlPointer> {
        let proxy = self.proxy.get_pointer(&self.queue_handle(), ());
        self.child(proxy)
    }

    pub(crate) fn get_keyboard(&self) -> Main<wl_keyboard::WlKeyboard> {
        let proxy = self.proxy.get_keyboard(&self.queue_handle(), ());
        self.child(proxy)
    }
}

impl Main<wl_data_device_manager::WlDataDeviceManager> {
    pub(crate) fn get_data_device(
        &self,
        seat: &Main<wl_seat::WlSeat>,
    ) -> Main<wl_data_device::WlDataDevice> {
        let proxy = self
            .proxy
            .get_data_device(&seat.proxy, &self.queue_handle(), ());
        self.child(proxy)
    }

    pub(crate) fn create_data_source(&self) -> Main<wl_data_source::WlDataSource> {
        let proxy = self.proxy.create_data_source(&self.queue_handle(), ());
        self.child(proxy)
    }
}

impl Main<ZwpTextInputManagerV3> {
    pub(crate) fn get_text_input(
        &self,
        seat: &Main<wl_seat::WlSeat>,
    ) -> Main<wayland_protocols::wp::text_input::zv3::client::zwp_text_input_v3::ZwpTextInputV3>
    {
        let proxy = self
            .proxy
            .get_text_input(&seat.proxy, &self.queue_handle(), ());
        self.child(proxy)
    }
}

impl Main<XdgActivationV1> {
    pub(crate) fn get_activation_token(&self) -> Main<XdgActivationTokenV1> {
        let proxy = self.proxy.get_activation_token(&self.queue_handle(), ());
        self.child(proxy)
    }
}

impl Main<ZxdgDecorationManagerV1> {
    pub(crate) fn get_toplevel_decoration(
        &self,
        toplevel: &Main<xdg_toplevel::XdgToplevel>,
    ) -> Main<ZxdgToplevelDecorationV1> {
        let proxy = self
            .proxy
            .get_toplevel_decoration(&toplevel.proxy, &self.queue_handle(), ());
        self.child(proxy)
    }
}

// 验证 callback registry 的检查式失败传播与 owner 生命周期。
#[cfg(test)]
mod tests {
    // 复用被测 registry Component 与稳定键类型。
    use super::{CallbackKey, CallbackRegistry};
    // 读取稳定错误分类。
    use crate::core::Errc;
    // 构造与生产 backend 相同的 bounded pending failure source。
    use crate::diagnostics::{PendingFailureQueue, PendingFailureSource};
    // 使用协议接口类型验证类型错配和 panic 诊断。
    use wayland_client::protocol::wl_surface;
    // 生成不依赖真实 Wayland connection 的测试类型身份。
    use std::any::TypeId;
    // 捕获测试主动制造的 registry mutex poison。
    use std::panic::{AssertUnwindSafe, catch_unwind};

    // 建立一个可独立观察 failure 的 registry fixture。
    fn registry_fixture() -> (CallbackRegistry, PendingFailureSource) {
        // 创建单测试专用 bounded queue。
        let queue = PendingFailureQueue::new();
        // 为 registry 创建唯一 failure source。
        let source = queue.source();
        // 返回共享同一 source 的 registry 与 owner 观察句柄。
        (CallbackRegistry::new(source.clone()), source)
    }

    // 生成不依赖协议连接的稳定 callback map 键。
    fn test_key(sequence: u32) -> CallbackKey {
        // 用测试类型身份和显式序号模拟协议对象键。
        (TypeId::of::<u32>(), sequence)
    }

    // 健康 registry 必须支持 take 后按同一规范形状回插。
    #[test]
    fn healthy_registry_preserves_owner_across_reinsert() {
        // 构造健康 registry 和本测试不需要读取的 failure source。
        let (registry, _source) = registry_fixture();
        // 建立可观察的稳定测试键。
        let key = test_key(1);
        // 初次 registration 必须接管 owner。
        assert!(registry.insert(key, Box::new(7_u32), "registration"));
        // dispatch take 必须转移同一 owner。
        let callback = registry
            // 从健康 registry 取出唯一值。
            .remove(&key, "dispatch take")
            // 初次登记后必须存在。
            .expect("registered callback owner must be present");
        // callback reinsert 必须重新接管同一 owner。
        assert!(registry.insert(key, callback, "callback reinsert"));
        // 再次取出验证 owner 没有在回插中丢失。
        let callback = registry
            // 从健康 registry 取出回插后的唯一值。
            .remove(&key, "dispatch take")
            // 回插后必须仍然存在。
            .expect("reinserted callback owner must be present");
        // 恢复测试值的具体类型。
        let value = callback
            // 测试 owner 应保持原始 u32 类型。
            .downcast::<u32>()
            // 健康回插不得改变擦除值类型。
            .expect("callback owner type must remain stable");
        // 验证同一 owner 的可观察值未改变。
        assert_eq!(*value, 7);
    }

    // registry 锁中毒必须为每个生命周期阶段产生稳定 typed failure。
    #[test]
    fn poisoned_registry_reports_each_lifecycle_operation() {
        // 构造待中毒 registry 与 owner-thread failure 观察句柄。
        let (registry, source) = registry_fixture();
        // 在同线程持锁 panic，避免 StoredCallback 的非 Send 约束影响测试。
        let poison = catch_unwind(AssertUnwindSafe(|| {
            // 取得健康 registry 的唯一 map guard。
            let _guard = registry
                // 仅测试模块直接访问 Component 内部 mutex。
                .callbacks
                // 初次锁定必须成功。
                .lock()
                // fixture 刚创建，不应已经中毒。
                .expect("fresh callback registry must lock");
            // 主动制造标准 Mutex poison 状态。
            panic!("poison Wayland callback registry for test");
        }));
        // 测试必须成功捕获主动 panic。
        assert!(poison.is_err());
        // registration 失败不得恢复访问中毒 map。
        assert!(!registry.insert(test_key(2), Box::new(1_u32), "registration"));
        // unregistration 失败不得恢复访问中毒 map。
        assert!(registry.remove(&test_key(2), "unregistration").is_none());
        // dispatch take 失败不得恢复访问中毒 map。
        assert!(registry.remove(&test_key(2), "dispatch take").is_none());
        // callback reinsert 失败不得恢复访问中毒 map。
        assert!(!registry.insert(test_key(2), Box::new(1_u32), "callback reinsert"));
        // 按调用顺序验证每个生命周期阶段都进入同一 source。
        for operation in [
            // 初次 callback 登记阶段。
            "registration",
            // 主动 callback 注销阶段。
            "unregistration",
            // dispatch 取出 callback 阶段。
            "dispatch take",
            // callback 执行完成后的回插阶段。
            "callback reinsert",
        ] {
            // owner thread 按 FIFO 取出下一条 registry failure。
            let error = source
                // 读取同一 registry source 的下一条失败。
                .take()
                // 每个损坏操作都必须产生一条失败。
                .expect("poisoned registry operation must enqueue failure");
            // registry mutex poison 必须稳定分类为 InvalidState。
            assert_eq!(error.code(), Errc::InvalidState);
            // 诊断必须保留精确注册生命周期阶段。
            assert!(error.what().contains(operation));
        }
        // 四个操作之后不得生成额外重复 failure。
        assert!(source.take().is_none());
    }

    // 擦除 callback 类型错配不得继续伪装为未登记 callback。
    #[test]
    fn callback_type_mismatch_is_reported_to_owner_thread() {
        // 构造健康 registry 与 owner-thread failure 观察句柄。
        let (registry, source) = registry_fixture();
        // 用错误的 u32 owner 模拟同键 callback 类型漂移。
        let callback = registry.downcast_callback::<wl_surface::WlSurface>(Box::new(9_u32));
        // 损坏 owner 必须被隔离而不是交付执行。
        assert!(callback.is_none());
        // owner thread 必须观察到类型错配失败。
        let error = source
            // 读取 registry 产生的唯一失败。
            .take()
            // 类型错配不能静默消失。
            .expect("callback type mismatch must enqueue failure");
        // registry 不变量损坏必须稳定分类为 InvalidState。
        assert_eq!(error.code(), Errc::InvalidState);
        // 诊断必须保留 Wayland 接口与错配阶段。
        assert!(
            // 读取稳定诊断文本。
            error
                // 借用 typed Error 的公开消息。
                .what()
                // 匹配协议接口与 registry 错配阶段。
                .contains("wl_surface callback registry type mismatch")
        );
    }

    // callback panic 转换必须保持既有 PlatformError 分类。
    #[test]
    fn callback_panic_is_reported_to_owner_thread() {
        // 构造健康 registry 与 owner-thread failure 观察句柄。
        let (registry, source) = registry_fixture();
        // 模拟 dispatch catch_unwind 已捕获 wl_surface callback panic。
        registry.report_callback_panic::<wl_surface::WlSurface>();
        // owner thread 必须观察到 panic 转换后的唯一失败。
        let error = source
            // 读取 registry 产生的 panic failure。
            .take()
            // callback panic 不能静默消失。
            .expect("callback panic must enqueue failure");
        // callback panic 必须继续分类为 PlatformError。
        assert_eq!(error.code(), Errc::PlatformError);
        // 诊断必须保留 Wayland 接口与 panic 阶段。
        assert!(error.what().contains("wl_surface callback panicked"));
    }
}
