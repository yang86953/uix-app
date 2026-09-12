//! 事件模型：SystemEvent / SemanticEvent / HandlerTable。

use std::any::{Any, TypeId};
use std::collections::{HashMap, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};

use crate::core::{Point, WidgetId, WindowId};

// 语义事件保留修饰键与鼠标按钮；按键值只由 system_event 子模块使用。
use crate::platform::windowing::{KeyMod, MouseButton, WindowResizeEdge};

pub(crate) mod system_event_handler;
// 将系统事件载荷与分类映射拆到同一 Event Module 的独立实现文件。
mod system_event;
// 保持公开路径 `ui::event::SystemEvent` 不变。
pub use system_event::SystemEvent;
/// 事件处理结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventResult {
    /// 事件已被某个处理器消费，终止后续传播。
    Handled,
    /// 事件未被任何处理器消费，交由上层继续处理。
    NotHandled,
    /// 事件已冒泡至当前传播链终点。
    Bubbled,
}

/// 由 UI 组件发往事件所属原生窗口的无外观动作。
///
/// 该类型只在 `ui -> app` 边界内流转；公开 API 使用
/// [`WindowControl`](crate::ui::WindowControl)
/// 描述可由应用放入自定义标题栏的控制项。
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowAction {
    BeginMoveDrag,
    BeginResizeDrag(WindowResizeEdge),
    Minimize,
    MaximizeRestore,
    ToggleMaximizeFromTitleBar,
    ShowSystemMenuFromTitleBar,
    RequestClose,
}

impl WindowAction {
    /// 原生标题栏手势不应改变客户区内已有键盘焦点。
    pub(crate) const fn preserves_keyboard_focus(self) -> bool {
        matches!(
            self,
            Self::BeginMoveDrag
                | Self::BeginResizeDrag(_)
                | Self::ToggleMaximizeFromTitleBar
                | Self::ShowSystemMenuFromTitleBar
        )
    }
}

/// 应用边界后的系统事件。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SystemEventKind {
    /// 指针按下。
    PointerDown,
    /// 指针双击。
    PointerDoubleClick,
    /// 指针抬起。
    PointerUp,
    /// 指针移动。
    PointerMove,
    /// 滚轮滚动。
    Wheel,
    /// 按键按下。
    KeyDown,
    /// 按键抬起。
    KeyUp,
    /// 复制请求。
    Copy,
    /// 剪切请求。
    Cut,
    /// 粘贴请求。
    Paste,
    /// 文本输入。
    TextInput,
    /// 输入法组合开始。
    ImeCompositionStart,
    /// 输入法组合内容更新。
    ImeCompositionUpdate,
    /// 输入法组合结束。
    ImeCompositionEnd,
    /// 获得焦点。
    FocusIn,
    /// 失去焦点。
    FocusOut,
    /// 指针进入组件区域。
    PointerEnter,
    /// 指针离开组件区域。
    PointerLeave,
    /// 窗口尺寸变化。
    Resize,
    /// 主题切换。
    ThemeChanged,
    /// 语言区域切换。
    LocaleChanged,
    /// 窗口最大化。
    WindowMaximize,
    /// 窗口最小化。
    WindowMinimize,
    /// 窗口还原。
    WindowRestore,
    /// 窗口获得焦点。
    WindowFocus,
    /// 窗口失去焦点。
    WindowBlur,
    /// 定时器到期触发。
    Timer,
    /// 文件拖放。
    FileDrop,
    /// 拖拽开始。
    DragStart,
    /// 拖拽移动。
    DragMove,
    /// 拖拽结束。
    DragEnd,
}

/// 内置语义事件种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticKind {
    /// 单击。
    Click,
    /// 值变更。
    Change,
    /// 提交。
    Submit,
    /// 文件拖放。
    FileDrop,
    /// 上下文菜单。
    ContextMenu,
    /// 复制。
    Copy,
    /// 剪切。
    Cut,
    /// 粘贴。
    Paste,
    /// 文本输入。
    TextInput,
    /// 输入法组合开始。
    ImeCompositionStart,
    /// 输入法组合更新。
    ImeCompositionUpdate,
    /// 输入法组合结束。
    ImeCompositionEnd,
    /// 自定义类型载荷（按类型标识）。
    Custom(TypeId),
}

/// Click 语义载荷。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClickEvent {
    /// 触发的鼠标按钮。
    pub button: MouseButton,
    /// 点击位置。
    pub pos: Point,
    /// 点击时持有的修饰键。
    pub modifiers: KeyMod,
}

/// 语义事件载荷。
pub enum SemanticPayload {
    /// 无载荷。
    None,
    /// 单击载荷，携带按钮/位置/修饰键。
    Click(ClickEvent),
    /// 文本载荷（输入、粘贴、组合等场景）。
    Text(String),
    /// 文件拖放载荷，携带文件列表与落点。
    FileDrop {
        /// 拖入的文件路径列表。
        files: Vec<String>,
        /// 拖放落点位置。
        position: Point,
    },
    /// 自定义类型载荷，按类型擦除存储。
    Custom(Box<dyn Any + Send>),
}

/// 语义事件。默认冒泡；handler 可 stop / preventDefault。
pub struct SemanticEvent {
    /// 事件种类。
    pub kind: SemanticKind,
    /// 原始目标组件。
    pub target: WidgetId,
    /// 当前正在处理的组件（冒泡过程中变化）。
    pub current_target: WidgetId,
    /// 事件载荷。
    pub payload: SemanticPayload,
    propagation_stopped: bool,
    default_prevented: bool,
}

impl SemanticEvent {
    /// 构造语义事件。
    pub fn new(kind: SemanticKind, target: WidgetId, payload: SemanticPayload) -> Self {
        Self {
            kind,
            target,
            current_target: target,
            payload,
            propagation_stopped: false,
            default_prevented: false,
        }
    }

    /// 构造单击事件。
    pub fn click(target: WidgetId, payload: ClickEvent) -> Self {
        Self::new(SemanticKind::Click, target, SemanticPayload::Click(payload))
    }

    pub fn is_primary_click(&self) -> bool {
        matches!(
            &self.payload,
            SemanticPayload::Click(ClickEvent {
                button: MouseButton::Left,
                ..
            })
        )
    }

    /// 构造上下文菜单事件。
    pub fn context_menu(target: WidgetId, payload: ClickEvent) -> Self {
        Self::new(
            SemanticKind::ContextMenu,
            target,
            SemanticPayload::Click(payload),
        )
    }

    /// 构造文本输入事件。
    pub fn text_input(target: WidgetId, text: impl Into<String>) -> Self {
        Self::new(
            SemanticKind::TextInput,
            target,
            SemanticPayload::Text(text.into()),
        )
    }

    /// 构造输入法组合开始事件。
    pub fn ime_composition_start(target: WidgetId) -> Self {
        Self::new(
            SemanticKind::ImeCompositionStart,
            target,
            SemanticPayload::None,
        )
    }

    /// 构造输入法组合更新事件。
    pub fn ime_composition_update(target: WidgetId, text: impl Into<String>) -> Self {
        Self::new(
            SemanticKind::ImeCompositionUpdate,
            target,
            SemanticPayload::Text(text.into()),
        )
    }

    /// 构造输入法组合结束事件。
    pub fn ime_composition_end(target: WidgetId, text: impl Into<String>) -> Self {
        Self::new(
            SemanticKind::ImeCompositionEnd,
            target,
            SemanticPayload::Text(text.into()),
        )
    }

    /// 构造复制事件。
    pub fn copy(target: WidgetId) -> Self {
        Self::new(SemanticKind::Copy, target, SemanticPayload::None)
    }

    /// 构造剪切事件。
    pub fn cut(target: WidgetId) -> Self {
        Self::new(SemanticKind::Cut, target, SemanticPayload::None)
    }

    /// 构造粘贴事件。
    pub fn paste(target: WidgetId, text: impl Into<String>) -> Self {
        Self::new(
            SemanticKind::Paste,
            target,
            SemanticPayload::Text(text.into()),
        )
    }

    /// 构造值变更事件。
    pub fn change(target: WidgetId, value: impl Into<String>) -> Self {
        Self::new(
            SemanticKind::Change,
            target,
            SemanticPayload::Text(value.into()),
        )
    }

    /// 构造提交事件。
    pub fn submit(target: WidgetId, value: impl Into<String>) -> Self {
        Self::new(
            SemanticKind::Submit,
            target,
            SemanticPayload::Text(value.into()),
        )
    }

    /// 构造文件拖放事件。
    pub fn file_drop(target: WidgetId, files: Vec<String>, position: Point) -> Self {
        Self::new(
            SemanticKind::FileDrop,
            target,
            SemanticPayload::FileDrop { files, position },
        )
    }

    /// 构造自定义载荷事件。
    pub fn custom<T: Any + Send>(target: WidgetId, payload: T) -> Self {
        Self::new(
            SemanticKind::Custom(TypeId::of::<T>()),
            target,
            SemanticPayload::Custom(Box::new(payload)),
        )
    }

    /// 停止事件继续冒泡。
    pub fn stop_propagation(&mut self) {
        self.propagation_stopped = true;
    }

    /// 阻止默认行为。
    pub fn prevent_default(&mut self) {
        self.default_prevented = true;
    }

    /// 是否已停止传播。
    pub fn propagation_stopped(&self) -> bool {
        self.propagation_stopped
    }

    /// 是否已阻止默认行为。
    pub fn default_prevented(&self) -> bool {
        self.default_prevented
    }

    /// 提取单击载荷；非单击事件返回 `None`。
    pub fn click_payload(&self) -> Option<&ClickEvent> {
        match &self.payload {
            SemanticPayload::Click(payload) => Some(payload),
            _ => None,
        }
    }

    /// 提取文本载荷；非文本事件返回 `None`。
    pub fn text_payload(&self) -> Option<&str> {
        match &self.payload {
            SemanticPayload::Text(text) => Some(text),
            _ => None,
        }
    }

    /// 提取文件拖放载荷；非拖放事件返回 `None`。
    pub fn file_drop_payload(&self) -> Option<(&[String], Point)> {
        match &self.payload {
            SemanticPayload::FileDrop { files, position } => Some((files, *position)),
            _ => None,
        }
    }

    /// 按类型提取自定义载荷；类型不符返回 `None`。
    pub fn custom_payload<T: Any>(&self) -> Option<&T> {
        match &self.payload {
            SemanticPayload::Custom(payload) => payload.downcast_ref::<T>(),
            _ => None,
        }
    }
}

type HandlerPredicate = dyn Fn(&SemanticEvent) -> bool + 'static;

#[derive(Default)]
/// 处理器选项：一次性与谓词过滤。
pub struct HandlerOptions {
    /// 是否为一次性处理器（触发后自动移除）。
    pub once: bool,
    /// 可选谓词：仅在谓词返回 `true` 时触发。
    pub when: Option<Box<HandlerPredicate>>,
}

impl HandlerOptions {
    /// 构造一次性处理器选项。
    pub fn once() -> Self {
        Self {
            once: true,
            when: None,
        }
    }

    /// 构造带谓词过滤的处理器选项。
    pub fn when(f: impl Fn(&SemanticEvent) -> bool + 'static) -> Self {
        Self {
            once: false,
            when: Some(Box::new(f)),
        }
    }
}

pub(crate) type SemanticHandler = Box<dyn FnMut(&mut SemanticEvent) + 'static>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct HandlerOptionsSignature {
    pub once: bool,
    pub when: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct HandlerSignature {
    pub kind: SemanticKind,
    pub options: HandlerOptionsSignature,
    pub generation: Option<u32>,
    pub capture_fingerprint: Option<u64>,
}

/// 语义事件处理器注册信息。
pub struct HandlerRegistration {
    /// 处理器关注的事件种类。
    pub kind: SemanticKind,
    /// 处理器选项。
    pub options: HandlerOptions,
    /// 处理器本体。
    pub handler: SemanticHandler,
    generation: Option<u32>,
    capture_fingerprints: Vec<u64>,
}

impl HandlerRegistration {
    /// 以默认选项构造注册信息。
    pub fn new(kind: SemanticKind, handler: SemanticHandler) -> Self {
        Self {
            kind,
            options: HandlerOptions::default(),
            handler,
            generation: None,
            capture_fingerprints: Vec::new(),
        }
    }

    /// 以指定选项构造注册信息。
    pub fn with_options(
        kind: SemanticKind,
        options: HandlerOptions,
        handler: SemanticHandler,
    ) -> Self {
        Self {
            kind,
            options,
            handler,
            generation: None,
            capture_fingerprints: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn with_generation(mut self, generation: u32) -> Self {
        self.generation = Some(generation);
        self
    }

    #[allow(dead_code)]
    pub(crate) fn with_capture_fingerprint(mut self, capture_fingerprint: u64) -> Self {
        self.capture_fingerprints.push(capture_fingerprint);
        self
    }

    /// Marks this handler as capturing `state` for reconcile-time reuse.
    pub fn with_state_capture<T: Clone + Send + Sync + 'static>(
        self,
        state: &crate::ui::reactive::state::State<T>,
    ) -> Self {
        crate::ui::reactive::state::capture_pending_state_bind(state);
        self.with_capture_fingerprint(state.capture_fingerprint())
    }

    /// Marks this handler as capturing `computed` for reconcile-time reuse.
    pub fn with_computed_capture<T: Clone + Send + Sync + 'static>(
        self,
        computed: &crate::ui::reactive::state::Computed<T>,
    ) -> Self {
        self.with_capture_fingerprint(computed.capture_fingerprint())
    }

    /// Marks this handler as capturing a window-scoped app handle.
    pub fn with_window_capture(self, window_id: WindowId) -> Self {
        self.with_capture_fingerprint(window_capture_fingerprint(window_id))
    }

    pub(crate) fn signature(&self) -> HandlerSignature {
        HandlerSignature {
            kind: self.kind,
            options: self.options.signature(),
            generation: self.generation,
            capture_fingerprint: combined_capture_fingerprint(&self.capture_fingerprints),
        }
    }

    pub(crate) fn authored_signature(&self) -> HandlerSignature {
        let mut signature = self.signature();
        if signature.generation.is_none() && signature.capture_fingerprint.is_some() {
            signature.generation = Some(0);
        }
        signature
    }
}

impl HandlerOptions {
    fn signature(&self) -> HandlerOptionsSignature {
        HandlerOptionsSignature {
            once: self.once,
            when: self.when.is_some(),
        }
    }
}

fn combined_capture_fingerprint(captures: &[u64]) -> Option<u64> {
    match captures {
        [] => None,
        [single] => Some(*single),
        _ => {
            let mut sorted = captures.to_vec();
            sorted.sort_unstable();
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            sorted.hash(&mut hasher);
            Some(hasher.finish())
        }
    }
}

fn window_capture_fingerprint(window_id: WindowId) -> u64 {
    let mut hasher = DefaultHasher::new();
    TypeId::of::<WindowId>().hash(&mut hasher);
    window_id.hash(&mut hasher);
    hasher.finish()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// 处理器唯一句柄。
pub struct HandlerId(usize);

struct HandlerEntry {
    id: HandlerId,
    kind: SemanticKind,
    options: HandlerOptions,
    handler: SemanticHandler,
}

#[derive(Default)]
/// 按组件维度组织的事件分发表。
pub struct HandlerTable {
    handlers: HashMap<WidgetId, Vec<HandlerEntry>>,
    next_id: usize,
}

impl HandlerTable {
    /// 构造空的分发表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册处理器并返回句柄。
    pub fn register(&mut self, widget: WidgetId, registration: HandlerRegistration) -> HandlerId {
        let id = HandlerId(self.next_id);
        self.next_id += 1;
        self.handlers.entry(widget).or_default().push(HandlerEntry {
            id,
            kind: registration.kind,
            options: registration.options,
            handler: registration.handler,
        });
        id
    }

    /// 注册原始语义事件处理器。
    pub fn on(
        &mut self,
        widget: WidgetId,
        kind: SemanticKind,
        handler: impl FnMut(&mut SemanticEvent) + 'static,
    ) -> HandlerId {
        self.register(widget, HandlerRegistration::new(kind, Box::new(handler)))
    }

    /// 注册单击处理器，自动解包 `ClickEvent` 载荷。
    pub fn on_click(
        &mut self,
        widget: WidgetId,
        mut handler: impl FnMut(&ClickEvent) + 'static,
    ) -> HandlerId {
        self.on(widget, SemanticKind::Click, move |event| {
            if let Some(payload) = event.click_payload() {
                handler(payload);
            }
        })
    }

    /// 注册文本输入处理器，自动解包文本载荷。
    pub fn on_text_input(
        &mut self,
        widget: WidgetId,
        mut handler: impl FnMut(&str) + 'static,
    ) -> HandlerId {
        self.on(widget, SemanticKind::TextInput, move |event| {
            if let Some(text) = event.text_payload() {
                handler(text);
            }
        })
    }

    /// 注册输入法组合开始处理器。
    pub fn on_ime_composition_start(
        &mut self,
        widget: WidgetId,
        mut handler: impl FnMut() + 'static,
    ) -> HandlerId {
        self.on(widget, SemanticKind::ImeCompositionStart, move |_event| {
            handler();
        })
    }

    /// 注册输入法组合更新处理器。
    pub fn on_ime_composition_update(
        &mut self,
        widget: WidgetId,
        mut handler: impl FnMut(&str) + 'static,
    ) -> HandlerId {
        self.on(widget, SemanticKind::ImeCompositionUpdate, move |event| {
            if let Some(text) = event.text_payload() {
                handler(text);
            }
        })
    }

    /// 注册输入法组合结束处理器。
    pub fn on_ime_composition_end(
        &mut self,
        widget: WidgetId,
        mut handler: impl FnMut(&str) + 'static,
    ) -> HandlerId {
        self.on(widget, SemanticKind::ImeCompositionEnd, move |event| {
            if let Some(text) = event.text_payload() {
                handler(text);
            }
        })
    }

    /// 注册复制处理器。
    pub fn on_copy(&mut self, widget: WidgetId, mut handler: impl FnMut() + 'static) -> HandlerId {
        self.on(widget, SemanticKind::Copy, move |_event| {
            handler();
        })
    }

    /// 注册剪切处理器。
    pub fn on_cut(&mut self, widget: WidgetId, mut handler: impl FnMut() + 'static) -> HandlerId {
        self.on(widget, SemanticKind::Cut, move |_event| {
            handler();
        })
    }

    /// 注册粘贴处理器。
    pub fn on_paste(
        &mut self,
        widget: WidgetId,
        mut handler: impl FnMut(&str) + 'static,
    ) -> HandlerId {
        self.on(widget, SemanticKind::Paste, move |event| {
            if let Some(text) = event.text_payload() {
                handler(text);
            }
        })
    }

    /// 注册值变更处理器。
    pub fn on_change(
        &mut self,
        widget: WidgetId,
        mut handler: impl FnMut(&str) + 'static,
    ) -> HandlerId {
        self.on(widget, SemanticKind::Change, move |event| {
            if let Some(value) = event.text_payload() {
                handler(value);
            }
        })
    }

    /// 注册提交处理器。
    pub fn on_submit(
        &mut self,
        widget: WidgetId,
        mut handler: impl FnMut(&str) + 'static,
    ) -> HandlerId {
        self.on(widget, SemanticKind::Submit, move |event| {
            if let Some(value) = event.text_payload() {
                handler(value);
            }
        })
    }

    /// 注册文件拖放处理器。
    pub fn on_file_drop(
        &mut self,
        widget: WidgetId,
        mut handler: impl FnMut(&[String], Point) + 'static,
    ) -> HandlerId {
        self.on(widget, SemanticKind::FileDrop, move |event| {
            if let Some((files, position)) = event.file_drop_payload() {
                handler(files, position);
            }
        })
    }

    /// 注册自定义类型处理器。
    pub fn on_custom<T: Any>(
        &mut self,
        widget: WidgetId,
        mut handler: impl FnMut(&T) + 'static,
    ) -> HandlerId {
        self.on(
            widget,
            SemanticKind::Custom(TypeId::of::<T>()),
            move |event| {
                if let Some(payload) = event.custom_payload::<T>() {
                    handler(payload);
                }
            },
        )
    }

    /// 移除指定组件上由 `handler_id` 标识的处理器。
    pub fn remove(&mut self, widget: WidgetId, handler_id: HandlerId) {
        if let Some(entries) = self.handlers.get_mut(&widget) {
            entries.retain(|entry| entry.id != handler_id);
        }
    }

    /// 清空指定组件上的全部处理器。
    pub fn clear_widget(&mut self, widget: WidgetId) {
        self.handlers.remove(&widget);
    }

    /// 清空全部分发表。
    pub fn clear(&mut self) {
        self.handlers.clear();
    }

    /// 沿组件路径自目标向根分发事件；遇停止传播立即中断。
    pub fn dispatch_path(&mut self, path: &[WidgetId], event: &mut SemanticEvent) -> EventResult {
        let mut handled = false;
        for &widget in path {
            event.current_target = widget;
            if self.dispatch_widget(widget, event) {
                handled = true;
            }
            if event.propagation_stopped() {
                break;
            }
        }
        if handled {
            EventResult::Handled
        } else {
            EventResult::NotHandled
        }
    }

    fn dispatch_widget(&mut self, widget: WidgetId, event: &mut SemanticEvent) -> bool {
        let Some(entries) = self.handlers.get_mut(&widget) else {
            return false;
        };

        let mut handled = false;
        let mut idx = 0;
        while idx < entries.len() {
            let matches = entries[idx].kind == event.kind;
            let enabled = entries[idx]
                .options
                .when
                .as_ref()
                .map(|predicate| predicate(event))
                .unwrap_or(true);

            if matches && enabled {
                handled = true;
                (entries[idx].handler)(event);
                if entries[idx].options.once {
                    entries.remove(idx);
                } else {
                    idx += 1;
                }
                if event.propagation_stopped() {
                    break;
                }
            } else {
                idx += 1;
            }
        }
        handled
    }
}

#[macro_export]
/// 将自定义载荷类型登记为语义事件种类（供 `on_custom` 使用）。
macro_rules! register_semantic {
    ($payload:ty) => {
        $crate::ui::event::SemanticKind::Custom(std::any::TypeId::of::<$payload>())
    };
}

// 事件模型专项测试：SystemEvent 映射、SemanticEvent 载荷与 HandlerTable 分派。
