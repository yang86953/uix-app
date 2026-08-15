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

#[derive(Clone)]
pub(crate) struct ProxyContext {
    callbacks: Arc<Mutex<HashMap<(TypeId, u32), Box<dyn Any>>>>,
    queue_handle: Option<QueueHandle<WaylandDispatchState>>,
    pending_failures: PendingFailureSource,
}

impl ProxyContext {
    pub(crate) fn new(
        queue_handle: QueueHandle<WaylandDispatchState>,
        pending_failures: PendingFailureSource,
    ) -> Self {
        Self {
            callbacks: Arc::new(Mutex::new(HashMap::new())),
            queue_handle: Some(queue_handle),
            pending_failures,
        }
    }

    fn queue_handle(&self) -> &QueueHandle<WaylandDispatchState> {
        self.queue_handle
            .as_ref()
            .expect("Wayland proxy context must have a queue handle")
    }

    fn with_callbacks(
        callbacks: Arc<Mutex<HashMap<(TypeId, u32), Box<dyn Any>>>>,
        queue_handle: QueueHandle<WaylandDispatchState>,
        pending_failures: PendingFailureSource,
    ) -> Self {
        Self {
            callbacks,
            queue_handle: Some(queue_handle),
            pending_failures,
        }
    }

    fn callback_key<I: Proxy + 'static>(proxy: &I) -> (TypeId, u32) {
        (TypeId::of::<I>(), proxy.id().protocol_id())
    }

    fn register<I, F>(&self, proxy: &I, callback: F)
    where
        I: Proxy + 'static,
        I::Event: 'static,
        F: FnMut(&Main<I>, I::Event, &QueueHandle<WaylandDispatchState>) + 'static,
    {
        self.callbacks
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(
                Self::callback_key(proxy),
                Box::new(Box::new(callback) as Callback<I>),
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
        // 按协议类型与对象编号删除唯一回调项。
        self.callbacks
            // 短时锁定兼容层回调表。
            .lock()
            // 中毒时仍由 owner thread 完成确定性清理。
            .unwrap_or_else(|error| error.into_inner())
            // 删除对象对应的回调，重复注销保持幂等。
            .remove(&Self::callback_key(proxy));
        // 结束回调注销实现。
    }
}

/// A 0.31 event queue state used by the legacy callback adapters.
pub(crate) struct WaylandDispatchState {
    callbacks: Arc<Mutex<HashMap<(TypeId, u32), Box<dyn Any>>>>,
    pending_failures: PendingFailureSource,
}

impl WaylandDispatchState {
    pub(crate) fn from_context(context: &ProxyContext) -> Self {
        Self {
            callbacks: Arc::clone(&context.callbacks),
            pending_failures: context.pending_failures.clone(),
        }
    }

    fn dispatch<I>(&mut self, proxy: &I, event: I::Event, qh: &QueueHandle<Self>)
    where
        I: Proxy + 'static,
        I::Event: 'static,
    {
        let key = ProxyContext::callback_key(proxy);
        let callback = self
            .callbacks
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(&key);

        let Some(callback) = callback else {
            return;
        };

        if let Ok(mut callback) = callback.downcast::<Callback<I>>() {
            let callback_proxy = Main::new(
                proxy.clone(),
                ProxyContext::with_callbacks(
                    Arc::clone(&self.callbacks),
                    qh.clone(),
                    self.pending_failures.clone(),
                ),
            );
            let callback_result = catch_unwind(AssertUnwindSafe(|| {
                callback(&callback_proxy, event, qh);
            }));
            if callback_result.is_err() {
                let _ = self.pending_failures.enqueue(Error::new(
                    Errc::PlatformError,
                    format!("Wayland {} callback panicked", I::interface().name),
                ));
            }
            // 按注册时的存储类型（Callback<I>）放回回调表：downcast 返回的是
            // Box<Callback<I>>，直接再装箱会让每次事件给回调类型多包一层，
            // 导致同一代理的第二个事件起 downcast 全部失败、回调永久丢失
            // （表现为输入无响应、装饰协商不生效）。
            let callback: Callback<I> = callback;
            self.callbacks
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .insert(key, Box::new(callback));
        }
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
