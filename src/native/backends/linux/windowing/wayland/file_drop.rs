// Wayland 文件拖放 Module 独占 data-offer、逐窗启用事实与 URI 传输生命周期。

// 哈希表关联协议 offer，集合保存逐窗能力开关。
use std::collections::{HashMap, HashSet};
// 文件对象独占 compositor 写入 pipe 的读端。
use std::fs::File;
// I/O 错误用于建立非阻塞传输。
use std::io;
// 协议 receive 只借用 pipe 写端。
use std::os::fd::AsFd;
// 原始描述符身份用于 event-loop poll 快照。
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
// 回调与 owner thread 共享唯一拖放 Component。
use std::sync::{Arc, Mutex};

// 核心协议类型承载 data-device 事件与复制动作协商。
use wayland_client::protocol::{
    // data-device 事件驱动拖放会话边沿。
    wl_data_device,
    // Copy 是 Upload 文件接收唯一支持的动作。
    wl_data_device_manager::DndAction,
    // offer callback 收集 MIME 与 compositor 选定动作。
    wl_data_offer,
};
// Proxy 提供协议版本与稳定对象身份，WEnum 安全处理未知枚举值。
use wayland_client::{Proxy, WEnum};

// 点与窗口身份构成 FileDrop 事件的稳定路由上下文。
use crate::core::{Errc, Error, Point, Result, WindowId};
// callback failure 统一进入 backend 既有 source。
use crate::diagnostics::PendingFailureSource;
// 完成的 URI 列表通过平台事件契约交给 Application System。
use crate::platform::windowing::event::UiEvent;
// 非阻塞累加器限制单轮读取预算并识别 EOF。
use crate::native::windowing::shared::nonblocking_read::{
    // 累加器保存尚未完成的 URI 字节。
    NonBlockingReadAccumulator,
    // 状态区分等待更多数据与完整 EOF。
    NonBlockingReadStatus,
};
// surface 路由表把协议 surface 身份解析为稳定 WindowId。
use crate::native::windowing::shared::window_target::SurfaceWindowTargets;

// Main 保留兼容 callback registry 上下文并提供显式注销端口。
use super::compat::Main;
// URI 解析 Component 只返回当前主机上的 UTF-8 文件路径。
use super::file_drop_uri::parse_uri_list;
// 纯注册表 Component 独占逐窗启用集合。
use super::file_drop_window_registry::FileDropWindowRegistry;

// freedesktop 文件拖放使用的标准 URI 列表 MIME。
const URI_LIST_MIME: &str = "text/uri-list";
// 单个拖放 FD 每轮最多读取 64 KiB，避免阻塞其他输入。
const FILE_DROP_READ_BUDGET: usize = 64 * 1024;

// 一个尚未确定为 Selection 或 DnD 的协议 offer。
struct PendingOffer {
    // 协议 owner 允许协商、接收并确定性销毁 offer。
    offer: Main<wl_data_offer::WlDataOffer>,
    // callback 按协议顺序收集 source 声明的 MIME。
    mime_types: HashSet<String>,
    // compositor 最近一次选中的动作是否为 Copy。
    selected_copy: bool,
}

// 当前指针所在 surface 的拖放会话快照。
struct ActiveDrop {
    // offer identity 关联 PendingOffer 中的协议 owner。
    offer_id: u32,
    // None 表示 surface 未注册、窗口未启用或 MIME 不受支持。
    window_id: Option<WindowId>,
    // 最后一次 Enter/Motion 的窗口局部坐标。
    position: Point,
    // accepted 明确区分协议会话与可生成业务事件的会话。
    accepted: bool,
}

// Drop 后由 owner-thread event loop 轮询的单次 URI 传输。
pub(crate) struct FileDropRead {
    // File 独占非阻塞 pipe 读端。
    file: File,
    // 累加器跨 poll 周期保存已读字节。
    bytes: NonBlockingReadAccumulator,
    // offer 必须存活到读取成功并发送 finish。
    offer: Main<wl_data_offer::WlDataOffer>,
    // FileDrop 事件定向到 Enter 时解析出的稳定窗口。
    window_id: WindowId,
    // FileDrop 事件保留 Drop 前最后坐标。
    position: Point,
    // v3+ 只有 compositor 已选择 Copy 才允许成功 finish。
    selected_copy: bool,
}

impl FileDropRead {
    // 创建一次 data-offer receive 使用的非阻塞 pipe。
    fn create(
        // 协议 offer owner 转移到 read 生命周期。
        offer: Main<wl_data_offer::WlDataOffer>,
        // 目标窗口在传输期间保持稳定。
        window_id: WindowId,
        // 坐标快照不再受后续 pointer motion 影响。
        position: Point,
        // 保存最终动作判定供 completion finish。
        selected_copy: bool,
        // 返回 read owner 与仅借给协议请求的写端。
    ) -> io::Result<(Self, OwnedFd)> {
        // pipe2 一次建立 CLOEXEC 的两端描述符。
        let mut fds = [0_i32; 2];
        // SAFETY: fds 指向两个连续可写 i32，pipe2 只在本调用中初始化它们。
        if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
            // 保留操作系统创建失败原因。
            return Err(io::Error::last_os_error());
        }
        // SAFETY: pipe2 成功后读端由本函数唯一接管一次。
        let read_fd = unsafe { OwnedFd::from_raw_fd(fds[0]) };
        // SAFETY: pipe2 成功后写端由本函数唯一接管一次。
        let write_fd = unsafe { OwnedFd::from_raw_fd(fds[1]) };
        // 读取端必须非阻塞，保持 UI owner thread 可调度。
        set_nonblocking(read_fd.as_raw_fd())?;
        // 返回完整 read owner 与临时写端 owner。
        Ok((
            // read owner 聚合 FD、数据与协议完成上下文。
            Self {
                // File 接管 OwnedFd 并负责最终关闭。
                file: File::from(read_fd),
                // 初始尚未读取任何 URI 字节。
                bytes: NonBlockingReadAccumulator::default(),
                // 保存 offer 直到 completion。
                offer,
                // 保存稳定 WindowId。
                window_id,
                // 保存最后坐标。
                position,
                // 保存动作结果。
                selected_copy,
            },
            // 写端在 receive 请求返回后即可 Drop。
            write_fd,
        ))
    }

    // 暴露只读 FD identity 供 poll 快照使用。
    pub(crate) fn fd(&self) -> RawFd {
        // 不转移 File 所有权。
        self.file.as_raw_fd()
    }

    // 在单轮预算内推进 URI 读取。
    fn read_available(&mut self) -> io::Result<NonBlockingReadStatus> {
        // 复用共享 EOF/WouldBlock 语义。
        self.bytes
            // 单个 FD 不得独占 event-loop 周期。
            .read_available(&mut self.file, FILE_DROP_READ_BUDGET)
    }
}

// Wayland 文件拖放 Component 的唯一可变状态。
#[derive(Default)]
pub(crate) struct WaylandFileDropState {
    // 只有显式启用的窗口可以接受 URI offer。
    enabled_windows: FileDropWindowRegistry<WindowId>,
    // DataOffer 到 Enter/Selection 之间保存未分类 offer。
    offers: HashMap<u32, PendingOffer>,
    // seat 同一时刻最多存在一个活动拖放焦点。
    active: Option<ActiveDrop>,
    // 已 Drop 的传输允许与后续新拖放并行完成。
    reads: Vec<FileDropRead>,
}

impl WaylandFileDropState {
    // 幂等更新逐窗能力，并在禁用时撤销该窗全部在途状态。
    pub(crate) fn set_window_enabled(&mut self, window_id: WindowId, enable: bool) {
        // 启用只发布窗口能力事实。
        if enable {
            // 集合天然保持幂等。
            self.enabled_windows.insert(window_id);
            // 启用不影响其他窗口或当前不相关会话。
            return;
        }
        // 禁用先删除未来 Enter 的接受资格。
        self.enabled_windows.remove(&window_id);
        // 当前会话属于该窗时立即拒绝并释放 offer。
        if self
            // 查看当前可选会话。
            .active
            // 只匹配稳定 WindowId。
            .as_ref()
            // 未接受的会话 window_id 为 None，不受本窗禁用影响。
            .is_some_and(|active| active.window_id == Some(window_id))
        {
            // take 后 cleanup 不会留下悬空 active identity。
            if let Some(active) = self.active.take() {
                // 拒绝并销毁尚未 Drop 的协议 offer。
                self.discard_offer(active.offer_id);
            }
        }
        // 已 Drop 但尚未读完的数据不得投递到已禁用窗口。
        let mut index = 0;
        // swap_remove 需要显式索引循环。
        while index < self.reads.len() {
            // 只撤销属于该窗口的 transfer owner。
            if self.reads[index].window_id == window_id {
                // 取出 owner 后关闭 FD 并销毁协议 offer。
                let read = self.reads.swap_remove(index);
                // 失败完成不得发送 finish。
                discard_read(read);
            } else {
                // 保留其他窗口 transfer 的相对无关性。
                index += 1;
            }
        }
    }

    // DataOffer callback 发布一个新的未分类 offer owner。
    fn insert_offer(&mut self, offer: Main<wl_data_offer::WlDataOffer>) -> Option<PendingOffer> {
        // 协议对象编号在同一连接生命周期中标识 offer。
        let offer_id = offer.id().protocol_id();
        // 替换异常重复编号时把旧 owner 交给调用方清理。
        self.offers.insert(
            // 哈希键使用稳定协议编号。
            offer_id,
            // 新 offer 初始没有 MIME 或动作事件。
            PendingOffer {
                // 保存兼容协议 owner。
                offer,
                // MIME 由随后 Offer 事件追加。
                mime_types: HashSet::new(),
                // 未收到 Action 前不得对 v3+ 宣称 Copy 成功。
                selected_copy: false,
            },
        )
    }

    // offer callback 追加一个 MIME 声明。
    fn advertise_mime(&mut self, offer_id: u32, mime_type: String) {
        // 迟到事件只允许命中仍活动的协议 owner。
        if let Some(record) = self.offers.get_mut(&offer_id) {
            // 集合去除重复 MIME，不改变协商结果。
            record.mime_types.insert(mime_type);
        }
    }

    // offer callback 保存 compositor 最近选择的动作。
    fn select_action(&mut self, offer_id: u32, action: WEnum<DndAction>) {
        // 迟到 Action 不得复活已经清理的 offer。
        if let Some(record) = self.offers.get_mut(&offer_id) {
            // 未知枚举与 None/Move/Ask 均不是支持的 Copy 结果。
            record.selected_copy = matches!(action, WEnum::Value(DndAction::Copy));
        }
    }

    // Enter 根据 MIME、surface 路由与逐窗开关一次提交接受结果。
    fn enter(
        &mut self,
        // Enter serial 授权本次 accept 请求。
        serial: u32,
        // 协议 offer identity 关联先前 DataOffer。
        offer_id: u32,
        // None 表示未知 surface 或路由 owner 不可用。
        window_id: Option<WindowId>,
        // Enter 坐标建立初始 Drop 位置。
        position: Point,
    ) {
        // 异常缺失 Leave 时先确定性释放旧活动会话。
        if let Some(previous) = self.active.take() {
            // 旧会话尚未 Drop，必须销毁它的 offer。
            self.discard_offer(previous.offer_id);
        }
        // 只有已启用窗口且 offer 包含 URI MIME 才可接受。
        let accepted = window_id.is_some_and(|window_id| {
            // 窗口能力与 MIME 必须同时成立。
            self.enabled_windows.contains(&window_id)
                // offer 必须仍由当前 Component 拥有。
                && self.offers.get(&offer_id).is_some_and(|record| {
                    // MIME 名称按协议精确匹配。
                    record.mime_types.contains(URI_LIST_MIME)
                })
        });
        // 对存在的 offer 提交明确接受或拒绝反馈。
        if let Some(record) = self.offers.get(&offer_id) {
            // v3+ 同时协商唯一支持的 Copy 动作。
            if record.offer.version() >= 3 {
                // 拒绝时提交空动作集合，接受时只提供 Copy。
                let actions = if accepted {
                    // Upload 不执行源文件移动或询问策略。
                    DndAction::Copy
                } else {
                    // 空位掩码明确表示目的端不接受。
                    DndAction::empty()
                };
                // preferred 与 supported 保持同一 Copy/None 事实。
                record.offer.set_actions(actions, actions);
            }
            // accept 的 MIME 与 accepted 事实完全一致。
            record.offer.accept(
                // serial 必须来自本次 Enter。
                serial,
                // 拒绝使用 None，接受使用标准 URI MIME。
                accepted.then(|| URI_LIST_MIME.to_string()),
            );
        }
        // 即使拒绝也保存会话，以便 Leave/Drop 销毁协议 offer。
        self.active = Some(ActiveDrop {
            // 保存本次 offer identity。
            offer_id,
            // 只有接受会话保存业务 WindowId。
            window_id: accepted.then_some(window_id).flatten(),
            // 保存窗口局部坐标。
            position,
            // 保存明确接受结果。
            accepted,
        });
    }

    // Motion 只更新当前会话的最后窗口局部坐标。
    fn motion(&mut self, position: Point) {
        // 无活动会话的迟到 Motion 保持幂等忽略。
        if let Some(active) = self.active.as_mut() {
            // 新坐标覆盖 Enter 或上一 Motion。
            active.position = position;
        }
    }

    // Leave 销毁尚未 Drop 的 offer 并断开 callback owner。
    fn leave(&mut self) {
        // take 先清除活动身份，避免 cleanup 期间重入观察旧状态。
        if let Some(active) = self.active.take() {
            // Leave 前没有 Drop，因此不能 finish。
            self.discard_offer(active.offer_id);
        }
    }

    // Drop 把已接受会话转换为非阻塞 read owner。
    fn drop_performed(&mut self) -> Result<()> {
        // 没有活动会话的迟到 Drop 不产生伪事件。
        let Some(active) = self.active.take() else {
            // 幂等忽略协议竞态。
            return Ok(());
        };
        // offer 必须仍由 Component 保存。
        let Some(record) = self.offers.remove(&active.offer_id) else {
            // 缺失 owner 表示 callback/协议状态不一致。
            return Err(Error::new(
                // 本地共享生命周期错误使用 InvalidState。
                Errc::InvalidState,
                // 诊断保留 Drop 与 offer identity 阶段。
                "Wayland file drop active offer is unavailable during Drop",
            ));
        };
        // 拒绝会话或缺失目标窗口只负责协议清理。
        let Some(window_id) = active.window_id.filter(|_| active.accepted) else {
            // 先断开 compat callback 强引用。
            record.offer.clear_callback();
            // 未接受 offer 不得 receive 或 finish。
            record.offer.destroy();
            // 明确拒绝不是业务错误。
            return Ok(());
        };
        // v3+ 必须已经收到 compositor 选择 Copy 的 Action。
        if record.offer.version() >= 3 && !record.selected_copy {
            // 断开 callback owner 后销毁未协商成功的 offer。
            record.offer.clear_callback();
            // 没有 Copy 动作时不接收数据。
            record.offer.destroy();
            // 协商失败属于正常拒绝。
            return Ok(());
        }
        // pipe 创建成功前保持 offer 局部 owner。
        let (read, write_fd) = match FileDropRead::create(
            // read 生命周期接管协议 owner 的 clone。
            record.offer.clone(),
            // 保存稳定目标窗口。
            window_id,
            // 保存最后坐标。
            active.position,
            // 保存完成阶段动作判定。
            record.selected_copy,
        ) {
            // 健康 pipe 把两端交给后续 receive 编排。
            Ok(pipe) => pipe,
            // setup 失败必须同步释放已经移出状态表的协议 owner。
            Err(error) => {
                // 先断开 offer callback 对 Component 的强引用。
                record.offer.clear_callback();
                // 未接收数据的 offer 直接销毁，不发送 finish。
                record.offer.destroy();
                // 返回可定位的 typed I/O failure。
                return Err(Error::new(
                    // 系统 pipe 失败属于 I/O 错误。
                    Errc::IoError,
                    // 保留底层 cause。
                    format!("Wayland file drop pipe creation failed: {error}"),
                ));
            }
        };
        // 向 source 请求 URI 列表并只借用写端。
        record
            // 协议请求与后续 finish 使用同一 offer。
            .offer
            // MIME 必须与 Enter 接受值一致。
            .receive(URI_LIST_MIME.to_string(), write_fd.as_fd());
        // Drop 后不再需要 Action/MIME callback，立即断开强引用环。
        record.offer.clear_callback();
        // read owner 加入 event-loop 可轮询队列。
        self.reads.push(read);
        // 写端在请求编码完成后由局部 owner 关闭。
        Ok(())
    }

    // Selection 取得 offer 后从 DnD owner 脱离但不销毁协议对象。
    fn detach_selection_offer(&mut self, offer_id: u32) -> Option<PendingOffer> {
        // clipboard 会立即发送 receive，因此这里只转移 callback 生命周期。
        self.offers.remove(&offer_id)
    }

    // 清理指定未 Drop offer；Enter 已用真实 serial 提交过接受或拒绝。
    fn discard_offer(&mut self, offer_id: u32) {
        // 已由 Selection 或 Drop 消费的 offer 保持幂等。
        if let Some(record) = self.offers.remove(&offer_id) {
            // 先断开 callback registry 的强引用。
            record.offer.clear_callback();
            // 再向 compositor 销毁协议 offer。
            record.offer.destroy();
        }
    }

    // backend shutdown 确定性释放全部窗口、offer 与 read owners。
    pub(crate) fn shutdown(&mut self) {
        // 清除未来 Enter 的所有接受资格。
        self.enabled_windows.clear();
        // 活动身份先失效。
        self.active = None;
        // drain 取得全部未分类/活动 offer owners。
        for (_, record) in self.offers.drain() {
            // 注销 callback 防止 backend 关闭后迟到写入。
            record.offer.clear_callback();
            // 销毁未完成协议对象。
            record.offer.destroy();
        }
        // 逐个释放 read FD 与已 Drop offer。
        for read in self.reads.drain(..) {
            // shutdown 不得发送成功 finish。
            discard_read(read);
        }
    }
}

// 把 data-device 新建的 offer 注册为显式 callback owner。
pub(crate) fn register_data_offer(
    // data-device Main 提供同一 ProxyContext 的 child 包装。
    data_device: &Main<wl_data_device::WlDataDevice>,
    // event-created child proxy 来自 DataOffer 事件。
    offer_proxy: wl_data_offer::WlDataOffer,
    // 所有 callbacks 共享唯一文件拖放 Component。
    state: &Arc<Mutex<WaylandFileDropState>>,
    // 状态失败进入 backend 既有 source。
    pending_failures: &PendingFailureSource,
) {
    // child 复用 data-device 的 callback registry 上下文。
    let offer = data_device.child(offer_proxy);
    // 稳定协议编号供 callback 定位记录。
    let offer_id = offer.id().protocol_id();
    // 任一重复旧 owner 在锁外清理。
    let replaced = match state.lock() {
        // 健康 Component 接管新 offer。
        Ok(mut state) => state.insert_offer(offer.clone()),
        // 中毒状态不得注册 callback 或伪造接受能力。
        Err(_) => {
            // 上报稳定 owner failure。
            enqueue_state_failure(pending_failures, "DataOffer registration");
            // 无 callback 的新 offer仍需协议销毁。
            offer.destroy();
            // 本次事件处理结束。
            return;
        }
    };
    // 异常复用编号时释放旧协议 owner。
    if let Some(replaced) = replaced {
        // 先注销旧 callback。
        replaced.offer.clear_callback();
        // 再销毁旧对象。
        replaced.offer.destroy();
    }
    // callback 只克隆共享状态与 failure source。
    let callback_state = Arc::clone(state);
    // failure source 是 runtime-scoped 廉价 clone。
    let callback_failures = pending_failures.clone();
    // 为 MIME 与 Action 事件注册持久 callback。
    offer.quick_assign(move |_, event, _| {
        // Component 损坏时不再解释协议事件。
        let Ok(mut state) = callback_state.lock() else {
            // 迟到 callback 仍报告稳定 owner failure。
            enqueue_state_failure(&callback_failures, "data-offer callback");
            // 停止本次事件。
            return;
        };
        // 只处理目的端需要的 offer 事件。
        match event {
            // MIME 声明追加到对应 offer。
            wl_data_offer::Event::Offer { mime_type } => {
                // 保持协议发送顺序。
                state.advertise_mime(offer_id, mime_type);
            }
            // compositor Action 决定 Drop 是否可成功完成。
            wl_data_offer::Event::Action { dnd_action } => {
                // 未知动作安全降级为非 Copy。
                state.select_action(offer_id, dnd_action);
            }
            // SourceActions 只影响 compositor 协商，目的端仅支持 Copy。
            wl_data_offer::Event::SourceActions { .. } => {}
            // 为未来协议版本保留 fail-closed 忽略。
            _ => {}
        }
    });
}

// Selection 事件把 offer callback owner 从文件拖放 Component 移交剪贴板。
pub(crate) fn detach_selection_offer(
    // Some offer 才存在待移交 identity。
    offer: Option<&wl_data_offer::WlDataOffer>,
    // 共享拖放 Component。
    state: &Arc<Mutex<WaylandFileDropState>>,
    // 状态损坏进入 backend source。
    pending_failures: &PendingFailureSource,
) {
    // 空 Selection 不携带新 offer。
    let Some(offer) = offer else {
        // 保持无操作语义。
        return;
    };
    // 提取协议编号后不持有 proxy 引用。
    let offer_id = offer.id().protocol_id();
    // 从拖放 owner 中移除记录。
    let detached = match state.lock() {
        // 健康状态允许分类为 Selection。
        Ok(mut state) => state.detach_selection_offer(offer_id),
        // 损坏状态无法安全访问记录。
        Err(_) => {
            // 记录稳定 failure，clipboard 仍可尝试读取原始 proxy。
            enqueue_state_failure(pending_failures, "Selection detach");
            // 不恢复 poisoned state。
            return;
        }
    };
    // Selection 不再需要 MIME callback，因为其 Offer 事件已先发送完毕。
    if let Some(detached) = detached {
        // 只清理本地 callback，不销毁 clipboard 即将 receive 的 offer。
        detached.offer.clear_callback();
    }
}

// 处理除 DataOffer 与 Selection 外的 data-device DnD 事件。
pub(crate) fn handle_data_device_event(
    // seat callback 转交完整协议事件。
    event: wl_data_device::Event,
    // surface 到窗口路由 owner。
    surface_windows: &Arc<Mutex<SurfaceWindowTargets>>,
    // 唯一文件拖放状态 owner。
    state: &Arc<Mutex<WaylandFileDropState>>,
    // callback failure source。
    pending_failures: &PendingFailureSource,
) {
    // Enter 需要先解析 surface，避免与 file-drop state 形成反向锁序。
    let event = match event {
        // Enter 建立新的协议会话。
        wl_data_device::Event::Enter {
            // serial 授权 accept。
            serial,
            // surface 解析稳定窗口。
            surface,
            // x 是窗口局部坐标。
            x,
            // y 是窗口局部坐标。
            y,
            // Some offer 才能接收外部文件。
            id,
        } => {
            // surface 编号只在当前连接内解释。
            let surface_id = surface.id().protocol_id();
            // 路由锁损坏时拒绝本次会话。
            let window_id = match surface_windows.lock() {
                // 健康表只复制 WindowId。
                Ok(targets) => targets.window_for_surface(surface_id),
                // 中毒表不得路由到猜测窗口。
                Err(_) => {
                    // 上报 surface owner failure。
                    enqueue_state_failure(pending_failures, "Enter surface routing");
                    // None 强制后续拒绝。
                    None
                }
            };
            // 无 offer 的内部拖动不属于文件接收能力。
            let Some(offer) = id else {
                // 清除异常残留 active 会话。
                if let Ok(mut state) = state.lock() {
                    // Leave 语义确定性释放旧 owner。
                    state.leave();
                } else {
                    // 报告拖放状态损坏。
                    enqueue_state_failure(pending_failures, "Enter without offer");
                }
                // 本次事件已处理。
                return;
            };
            // 提取 DataOffer 建立的稳定 identity。
            let offer_id = offer.id().protocol_id();
            // 锁定拖放 Component 提交协商。
            let Ok(mut state) = state.lock() else {
                // 状态损坏时无法安全 accept。
                enqueue_state_failure(pending_failures, "Enter state");
                // 停止处理。
                return;
            };
            // 接受结果由 Component 内部三项条件共同决定。
            state.enter(
                // 使用本次协议 serial。
                serial,
                // 关联 offer owner。
                offer_id,
                // 传入可选稳定窗口。
                window_id,
                // 固定点转换为跨平台 f32 坐标。
                Point::new(x as f32, y as f32),
            );
            // Enter 已完全处理，不再进入通用分支。
            return;
        }
        // 其他事件不需要先锁 surface 路由。
        event => event,
    };
    // 所有非 Enter 状态变更只需要单一 Component guard。
    let Ok(mut state) = state.lock() else {
        // 共享状态损坏统一上报。
        enqueue_state_failure(pending_failures, "data-device callback");
        // 停止本次事件。
        return;
    };
    // 提交会话其余生命周期边沿。
    match event {
        // Motion 更新最后坐标。
        wl_data_device::Event::Motion { x, y, .. } => {
            // 坐标保持 surface-local 语义。
            state.motion(Point::new(x as f32, y as f32));
        }
        // Leave 取消尚未 Drop 的会话。
        wl_data_device::Event::Leave => state.leave(),
        // Drop 建立非阻塞 URI read owner。
        wl_data_device::Event::Drop => {
            // setup failure 在释放 guard 后进入 failure source。
            if let Err(error) = state.drop_performed() {
                // 避免持锁调用 pending source。
                drop(state);
                // 转交 typed error。
                let _ = pending_failures.enqueue(error);
            }
        }
        // DataOffer 与 Selection 由 seat callback 的分类入口处理。
        _ => {}
    }
}

// 从健康 Component 构造本轮 poll 的 FD identity 快照。
pub(crate) fn read_fds(
    // 共享拖放状态 owner。
    state: &Arc<Mutex<WaylandFileDropState>>,
    // owner failure source。
    pending_failures: &PendingFailureSource,
) -> Option<Vec<RawFd>> {
    // 锁中毒时用 None 区分健康空队列。
    let state = match state.lock() {
        // 健康 guard 只用于复制 FD。
        Ok(state) => state,
        // 中毒 state 不得把其中 FD 交给 poll。
        Err(_) => {
            // 上报精确 snapshot failure。
            enqueue_state_failure(pending_failures, "poll snapshot");
            // 调用方必须终止本轮 dispatch。
            return None;
        }
    };
    // Vec 快照不接管任何 File 生命周期。
    Some(state.reads.iter().map(FileDropRead::fd).collect())
}

// 处理单个 file-drop FD 的 poll completion。
pub(crate) fn complete_polled_read(
    // 本轮 poll 的稳定 FD identity。
    polled_fd: RawFd,
    // readiness/error 位来自同一 poll 快照。
    revents: i16,
    // 唯一拖放状态 owner。
    state: &Arc<Mutex<WaylandFileDropState>>,
    // 完成事件队列 owner。
    events: &Arc<Mutex<std::collections::VecDeque<UiEvent>>>,
    // completion failure source。
    pending_failures: &PendingFailureSource,
) -> bool {
    // 状态 guard 只覆盖 identity 校验与非阻塞读取。
    let outcome = {
        // 损坏状态不得读取或释放内部 FD。
        let mut state = match state.lock() {
            // 健康 Component 允许定位 read。
            Ok(state) => state,
            // 中毒 state 转成 typed failure。
            Err(_) => {
                // guard 不存在，可直接入队。
                enqueue_state_failure(pending_failures, "poll completion");
                // 调用方停止 dispatch。
                return false;
            }
        };
        // 旧 poll 快照可能对应已禁用或已完成的 FD。
        let Some(index) = state.reads.iter().position(|read| read.fd() == polled_fd) else {
            // 健康陈旧事件保持幂等。
            return true;
        };
        // FD 错误先移除 owner，锁外再报告与销毁协议对象。
        if (revents & (libc::POLLERR | libc::POLLNVAL)) != 0 {
            // swap_remove 原子释放 state 对该 read 的所有权。
            let read = state.reads.swap_remove(index);
            // 返回结构化失败结果。
            ReadOutcome::Failed(
                // 保留 read owner 供锁外 cleanup。
                read,
                // 使用稳定 poll 错误文本。
                Error::new(
                    // FD poll 错误属于 I/O 失败。
                    Errc::IoError,
                    // 诊断区分 file-drop 来源。
                    "Wayland file drop read fd reported an error",
                ),
            )
        } else if (revents & (libc::POLLIN | libc::POLLHUP)) == 0 {
            // 无 readiness 时保留 owner。
            ReadOutcome::Pending
        } else {
            // 有限预算读取当前 FD。
            match state.reads[index].read_available() {
                // WouldBlock 或预算耗尽保留 owner。
                Ok(NonBlockingReadStatus::Pending) => ReadOutcome::Pending,
                // EOF 后取出完整 owner 与字节。
                Ok(NonBlockingReadStatus::Complete(bytes)) => {
                    // read owner 从共享状态转移到完成阶段。
                    let read = state.reads.swap_remove(index);
                    // 锁外解析 URI 并提交事件。
                    ReadOutcome::Complete(read, bytes)
                }
                // syscall 错误释放 read owner。
                Err(error) => {
                    // 取出失败 owner。
                    let read = state.reads.swap_remove(index);
                    // 包装底层 I/O cause。
                    ReadOutcome::Failed(
                        // 传递协议/FD owner。
                        read,
                        // 生成 typed error。
                        Error::new(
                            // read syscall 失败属于 I/O 错误。
                            Errc::IoError,
                            // 保留底层 cause。
                            format!("Wayland file drop read failed: {error}"),
                        ),
                    )
                }
            }
        }
    };
    // 锁外完成协议与事件交付。
    match outcome {
        // Pending 不修改任何 owner。
        ReadOutcome::Pending => true,
        // 失败路径不得发送 finish。
        ReadOutcome::Failed(read, error) => {
            // 先关闭 FD 并销毁 offer。
            discard_read(read);
            // 再把失败交给 backend source。
            let _ = pending_failures.enqueue(error);
            // owner 本身仍健康，允许上层读取 failure 后决定关闭。
            true
        }
        // 完整字节需要解析与定向事件。
        ReadOutcome::Complete(read, bytes) => {
            // 解析只保留本地 file URI。
            let files = parse_uri_list(&bytes);
            // 非空文件列表才产生 FileDrop 业务事件。
            if !files.is_empty() {
                // 事件队列必须健康后才可向 source 宣称成功。
                let mut events = match events.lock() {
                    // 健康 guard 接受定向事件。
                    Ok(events) => events,
                    // 队列损坏时取消协议成功。
                    Err(_) => {
                        // 释放失败 transfer。
                        discard_read(read);
                        // 上报稳定 owner failure。
                        enqueue_state_failure(pending_failures, "event delivery");
                        // 调用方停止本轮 dispatch。
                        return false;
                    }
                };
                // 事件携带 Enter 时的 WindowId 与最终 Motion 坐标。
                events.push_back(
                    // 构造平台文件拖放事件。
                    UiEvent::file_drop(files, read.position)
                        // 明确定向，禁止多窗广播。
                        .for_window(read.window_id),
                );
            }
            // 数据读取与可选事件投递成功后完成协议会话。
            finish_read(read);
            // completion 成功。
            true
        }
    }
}

// completion 在锁外传递 read owner 与结果。
enum ReadOutcome {
    // 本轮无完整结果。
    Pending,
    // EOF 携带完整 URI 字节。
    Complete(FileDropRead, Vec<u8>),
    // 失败携带待清理 owner 与 typed error。
    Failed(FileDropRead, Error),
}

// 成功 transfer 按协议版本发送 finish 并销毁 offer。
fn finish_read(read: FileDropRead) {
    // v3+ 只有 Copy 动作会到达该成功路径。
    if read.offer.version() >= 3 && read.selected_copy {
        // receive 已完成后通知 source 可以结束 DnD。
        read.offer.finish();
    }
    // finish 后不再发送除 destroy 外的请求。
    read.offer.destroy();
    // File 字段随 read Drop 关闭 pipe 读端。
}

// 失败或取消 transfer 只销毁 offer，不发送 finish。
fn discard_read(read: FileDropRead) {
    // callback 已在 Drop 转为 read 时清除。
    read.offer.destroy();
    // File 字段随 read Drop 关闭 FD。
}

// 把共享状态锁失败统一投递为稳定诊断。
fn enqueue_state_failure(pending_failures: &PendingFailureSource, operation: &'static str) {
    // 迟到 callback 在 source 已关闭时允许静默丢弃 enqueue 结果。
    let _ = pending_failures.enqueue(Error::new(
        // poisoned Component 属于 InvalidState。
        Errc::InvalidState,
        // 操作名定位具体生命周期边沿。
        format!("Wayland file drop state mutex poisoned during {operation}"),
    ));
}

// 为 pipe 读端追加 O_NONBLOCK 标志。
fn set_nonblocking(fd: RawFd) -> io::Result<()> {
    // SAFETY: fd 在同步 fcntl 调用期间保持打开，F_GETFL 不访问 Rust 内存。
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    // 负值表示底层描述符状态读取失败。
    if flags < 0 {
        // 读取线程本地 errno。
        return Err(io::Error::last_os_error());
    }
    // SAFETY: fd 仍由当前 read owner 持有，flags 来自同一描述符。
    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        // 设置失败保留系统原因。
        return Err(io::Error::last_os_error());
    }
    // 描述符现为非阻塞。
    Ok(())
}

// backend owner-thread 提供确定性的文件拖放关闭端口。
impl super::WaylandBackend {
    // 在 data-device callback 注销前释放全部 offer 与 pipe owners。
    pub(crate) fn shutdown_file_drop(&mut self) {
        // shutdown 恢复 poisoned guard 只用于不可再观察的最终释放。
        self.file_drop_state
            // 获取 Component 的唯一可变访问。
            .lock()
            // 中毒不应阻止 owner-thread 关闭 FD 与 callback owners。
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            // 执行幂等状态清理。
            .shutdown();
    }
}
