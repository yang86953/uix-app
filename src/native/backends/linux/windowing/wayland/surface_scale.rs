// Wayland output scale 与逐窗 surface buffer scale 的私有协调 Component。

// 有序映射让多 output 有效 scale 选择保持确定性。
use std::collections::BTreeMap;
// registry 只持有逐窗状态的弱引用，不延长窗口生命周期。
use std::sync::{Arc, Mutex, Weak};

// Wayland surface 回调需要识别 Enter 与 Leave 目标 output。
use wayland_client::protocol::wl_surface;
// Proxy trait 暴露 output 的稳定协议 identity。
use wayland_client::Proxy;

// 引入目标窗口身份和 typed 错误分类。
use crate::core::{Errc, Error, Result, WindowId};
// callback 失败只写入 runtime-scoped 有界 failure source。
use crate::diagnostics::PendingFailureSource;
// graphics 与 windowing 共享同一份逐窗 surface 元数据。
use crate::native::presentation::graphics::platform::linux::WaylandSurfaceMetrics;
// scale 变化通过既有逐窗 resize 事实进入唯一 WindowDriver 管线。
use crate::platform::windowing::event::UiEvent;

// 兼容代理保持 Wayland 回调注册与 teardown 语义一致。
use super::compat::Main;

// backend 级 output scale registry 的锁内状态。
struct OutputScaleRegistryState {
    // 首个 output 是无 Enter 事实时的稳定默认输出。
    default_output: Option<u32>,
    // 按协议 identity 保存每个 output 的最新整数 scale。
    scales: BTreeMap<u32, i32>,
    // 弱引用列表用于把后续 wl_output::Scale 变化广播给仍存活窗口。
    surfaces: Vec<Weak<WaylandWindowScaleState>>,
}

// Wayland backend 唯一的 output scale 事实所有者。
pub(crate) struct WaylandOutputScaleRegistry {
    // 单锁原子维护 output 事实与订阅窗口集合。
    state: Mutex<OutputScaleRegistryState>,
    // registry 回调错误复用 backend 已有 failure source。
    pending_failures: PendingFailureSource,
}

impl WaylandOutputScaleRegistry {
    // 创建尚未登记 output 或窗口的 backend 私有 registry。
    pub(crate) fn new(pending_failures: PendingFailureSource) -> Self {
        // 发布空 registry。
        Self {
            // 初始化所有锁内集合。
            state: Mutex::new(OutputScaleRegistryState {
                // 首个 output 尚未出现。
                default_output: None,
                // 尚无 output scale 事实。
                scales: BTreeMap::new(),
                // 尚无逐窗订阅者。
                surfaces: Vec::new(),
            }),
            // 保存同一 backend failure source 的廉价 clone。
            pending_failures,
        }
    }

    // 登记一个已绑定 output，并以 scale=1 等待 compositor 的 Scale 事件。
    pub(crate) fn register_output(&self, output_id: u32, is_default: bool) {
        // 中毒 registry 不能继续改写，转换为 owner-thread failure。
        let Ok(mut state) = self.state.lock() else {
            // 发布稳定错误后放弃本次登记。
            self.report_lock_failure("register output");
            // 不恢复损坏锁中的业务状态。
            return;
        };
        // 首个 output 或显式默认 output 建立 fallback 身份。
        if state.default_output.is_none() || is_default {
            // 保存默认协议 identity。
            state.default_output = Some(output_id);
        }
        // 重复登记保持既有 scale，首次登记从 1 开始。
        state.scales.entry(output_id).or_insert(1);
    }

    // 登记逐窗弱引用，使 output scale 更新不建立反向生命周期所有权。
    pub(crate) fn register_surface(&self, surface: &Arc<WaylandWindowScaleState>) {
        // 中毒 registry 只报告失败，不把窗口放入损坏集合。
        let Ok(mut state) = self.state.lock() else {
            // 记录精确登记阶段。
            self.report_lock_failure("register surface");
            // 停止本次登记。
            return;
        };
        // 顺便清理已经 teardown 的窗口弱引用。
        state.surfaces.retain(|entry| entry.strong_count() > 0);
        // 保存当前窗口的弱引用。
        state.surfaces.push(Arc::downgrade(surface));
    }

    // backend teardown 后清空 output 与弱订阅事实，迟到 callback 已由上层先注销。
    pub(crate) fn shutdown(&self) {
        // teardown 阶段恢复锁只用于释放已不可再观察的 registry 值。
        let mut state = self
            // 等待取得 registry 唯一写权限。
            .state
            // 锁住完整 registry。
            .lock()
            // 关闭路径不因旧 callback 中毒跳过资源释放。
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // 移除默认 output identity。
        state.default_output = None;
        // 清空全部 output scale 事实。
        state.scales.clear();
        // 清空全部窗口弱订阅。
        state.surfaces.clear();
    }

    // 返回指定 output 的当前 scale；未知 output 采用协议默认 1。
    pub(crate) fn scale_for(&self, output_id: u32) -> i32 {
        // 中毒时报告并使用不放大的安全 fallback。
        let Ok(state) = self.state.lock() else {
            // 记录精确查询阶段。
            self.report_lock_failure("read output scale");
            // 协议默认 buffer scale 为 1。
            return 1;
        };
        // 所有已登记值都再次收敛为正整数。
        state.scales.get(&output_id).copied().unwrap_or(1).max(1)
    }

    // 返回无 surface Enter 事实时使用的默认 output scale。
    pub(crate) fn preferred_scale(&self) -> i32 {
        // 中毒 registry 不能提供可信默认 output。
        let Ok(state) = self.state.lock() else {
            // 记录精确查询阶段。
            self.report_lock_failure("read preferred output scale");
            // 使用 Wayland core 默认值。
            return 1;
        };
        // 从同一 guard 解析默认 identity 与 scale。
        preferred_scale(&state)
    }

    // 提交 wl_output::Scale，并通知所有仍进入该 output 的窗口。
    pub(crate) fn update_output_scale(&self, output_id: u32, scale: i32) {
        // 协议要求正整数，异常值收敛为 1，禁止形成零尺寸 drawable。
        let scale = scale.max(1);
        // 锁内只更新 registry 并克隆存活窗口，通知在锁外执行。
        let (surfaces, fallback_scale, is_default) = {
            // 中毒 registry 不允许部分更新 output 事实。
            let Ok(mut state) = self.state.lock() else {
                // 记录精确更新阶段。
                self.report_lock_failure("update output scale");
                // 放弃本次协议更新。
                return;
            };
            // 未知 output 的 Scale 事件仍先建立登记事实。
            state.scales.insert(output_id, scale);
            // 没有默认 output 时采用首个实际 Scale 来源。
            if state.default_output.is_none() {
                // 保存 fallback identity。
                state.default_output = Some(output_id);
            }
            // 判断本次更新是否同时改变无 Enter 窗口的 fallback。
            let is_default = state.default_output == Some(output_id);
            // 读取更新后的默认 scale。
            let fallback_scale = preferred_scale(&state);
            // 升级存活窗口并清理已失效弱引用。
            let surfaces = state
                // 遍历全部逐窗订阅者。
                .surfaces
                // 借用弱引用集合。
                .iter()
                // 只保留仍存活的窗口状态。
                .filter_map(Weak::upgrade)
                // 收集后立即释放 registry 锁。
                .collect::<Vec<_>>();
            // 返回锁外通知所需的稳定快照。
            (surfaces, fallback_scale, is_default)
        };
        // 每个窗口独立决定该 output 是否参与自己的有效 scale。
        for surface in surfaces {
            // 通知只会写目标窗口的 metrics 与事件队列。
            surface.output_scale_updated(output_id, scale, fallback_scale, is_default);
        }
    }

    // 把 registry 锁损坏转成 backend 已有的有界 typed failure。
    fn report_lock_failure(&self, operation: &str) {
        // callback 端只入队，不执行日志或应用恢复逻辑。
        let _ = self.pending_failures.enqueue(Error::new(
            // registry owner 损坏属于稳定 InvalidState。
            Errc::InvalidState,
            // 错误保留发生操作。
            format!("Wayland output scale registry lock poisoned during {operation}"),
        ));
    }
}

// 从单一 registry guard 解析默认 output scale。
fn preferred_scale(state: &OutputScaleRegistryState) -> i32 {
    // 默认 identity 缺失或未登记 Scale 时使用 1。
    state
        // 读取默认 output identity。
        .default_output
        // 查询对应 scale。
        .and_then(|output_id| state.scales.get(&output_id).copied())
        // 未知值采用协议默认。
        .unwrap_or(1)
        // 防御异常非正值。
        .max(1)
}

// 单个 Wayland 窗口的 output 进入集合与有效 scale 状态。
struct WindowScaleState {
    // 没有 output Enter 事实时采用的 backend 默认 scale。
    fallback_scale: i32,
    // 当前 surface 已进入 output 及其最新 scale。
    entered_outputs: BTreeMap<u32, i32>,
    // 已发布给 surface 与 graphics 的有效 scale。
    effective_scale: i32,
}

// 窗口 owner、surface callback 与 graphics descriptor 共享的窄状态端口。
pub(crate) struct WaylandWindowScaleState {
    // 事件必须回到此目标窗口。
    window_id: WindowId,
    // 锁只保护 output 集合与有效 scale 决策。
    state: Mutex<WindowScaleState>,
    // logical/drawable 事实通过 presentation 共享值原子发布。
    metrics: Arc<WaylandSurfaceMetrics>,
    // scale 变化复用既有逐窗 resize 事件路径。
    events: Arc<Mutex<std::collections::VecDeque<UiEvent>>>,
    // callback 锁失败写入同一 runtime source。
    pending_failures: PendingFailureSource,
}

impl WaylandWindowScaleState {
    // 创建一个尚未进入 output 的逐窗 scale owner。
    pub(crate) fn new(
        window_id: WindowId,
        logical_width: i32,
        logical_height: i32,
        fallback_scale: i32,
        events: Arc<Mutex<std::collections::VecDeque<UiEvent>>>,
        pending_failures: PendingFailureSource,
    ) -> Arc<Self> {
        // 所有初始 scale 都收敛为 Wayland core 正整数。
        let fallback_scale = fallback_scale.max(1);
        // 创建窗口唯一共享 owner。
        Arc::new(Self {
            // 保存目标窗口身份。
            window_id,
            // 初始化 output 集合与有效 scale。
            state: Mutex::new(WindowScaleState {
                // 保存无 Enter fallback。
                fallback_scale,
                // 初始尚未进入任何 output。
                entered_outputs: BTreeMap::new(),
                // 初始有效 scale 等于 fallback。
                effective_scale: fallback_scale,
            }),
            // 创建同代 logical/drawable 元数据。
            metrics: Arc::new(WaylandSurfaceMetrics::new(
                // 保存初始逻辑宽度。
                logical_width,
                // 保存初始逻辑高度。
                logical_height,
                // 保存初始有效 scale。
                fallback_scale,
            )),
            // 保存 backend 逐窗事件队列。
            events,
            // 保存 runtime failure source clone。
            pending_failures,
        })
    }

    // 返回 graphics descriptor 与 presenter 共用的 surface 元数据。
    pub(crate) fn metrics(&self) -> Arc<WaylandSurfaceMetrics> {
        // Arc clone 不复制任何 surface 事实。
        Arc::clone(&self.metrics)
    }

    // 返回当前应提交给 wl_surface 的整数 buffer scale。
    pub(crate) fn current_scale(&self) -> i32 {
        // 中毒逐窗状态不能继续读取，报告后使用协议默认。
        let Ok(state) = self.state.lock() else {
            // 记录精确读取阶段。
            self.report_lock_failure("read current scale");
            // 安全 fallback 不放大 buffer。
            return 1;
        };
        // 有效值在每次提交时已保证为正。
        state.effective_scale.max(1)
    }

    // xdg_toplevel configure 发布最新 logical extent，scale 保持同代值。
    pub(crate) fn set_logical_extent(&self, width: i32, height: i32) {
        // 读取有效 scale 后立即释放窗口决策锁。
        let scale = self.current_scale();
        // 无效 configure 尺寸不覆盖上一份可呈现快照。
        if width <= 0 || height <= 0 {
            // 零尺寸由窗口调度的 suspend 语义处理，不进入 drawable 元数据。
            return;
        }
        // 原子提交 logical extent 与当前 scale。
        if let Err(error) = self.metrics.update(width, height, scale) {
            // metrics 错误进入 owner-thread failure source。
            let _ = self.pending_failures.enqueue(error);
        }
    }

    // 程序化 resize 在同一事务内发布新 surface 尺寸并进入窗口事件管线。
    pub(crate) fn publish_programmatic_resize(&self, width: i32, height: i32) -> Result<()> {
        // 先取得事件队列所有权，失败时不得提前改写 surface metrics。
        let mut events = self.events.lock().map_err(|_| {
            Error::new(
                Errc::InvalidState,
                "Wayland programmatic resize event queue mutex poisoned",
            )
        })?;
        // logical 与 drawable 尺寸必须使用当前有效整数 scale 同代更新。
        let updated = self.metrics.update(width, height, self.current_scale())?;
        // 复用唯一 WindowResize 管线触发布局、presentation 与后续 surface commit。
        events.push_back(
            UiEvent::resize(updated.logical_width, updated.logical_height)
                .for_window(self.window_id),
        );
        Ok(())
    }

    // surface 进入 output 后更新该 output 的 scale 并返回变化后的 buffer scale。
    pub(crate) fn enter_output(&self, output_id: u32, scale: i32) -> Option<i32> {
        // 修改闭包只操作锁内 output 集合。
        self.update_effective_scale(|state| {
            // 保存或替换当前 output scale。
            state.entered_outputs.insert(output_id, scale.max(1));
        })
    }

    // surface 离开 output 后按剩余 output 或 fallback 重算有效 scale。
    pub(crate) fn leave_output(&self, output_id: u32, fallback_scale: i32) -> Option<i32> {
        // 修改闭包同时刷新 fallback 与进入集合。
        self.update_effective_scale(|state| {
            // 保留最新 backend 默认 scale。
            state.fallback_scale = fallback_scale.max(1);
            // 移除已经离开的 output。
            state.entered_outputs.remove(&output_id);
        })
    }

    // backend 收到 output Scale 更新时只影响进入该 output 或依赖默认值的窗口。
    fn output_scale_updated(
        &self,
        output_id: u32,
        scale: i32,
        fallback_scale: i32,
        is_default: bool,
    ) {
        // 重算并由统一 helper 排队逐窗 resize。
        let _ = self.update_effective_scale(|state| {
            // 默认 output 更新同时刷新 fallback。
            if is_default {
                // 保存新的 fallback scale。
                state.fallback_scale = fallback_scale.max(1);
            }
            // 只有已经进入该 output 的窗口消费其动态 scale。
            if let Some(current) = state.entered_outputs.get_mut(&output_id) {
                // 原位替换最新正整数 scale。
                *current = scale.max(1);
            }
        });
    }

    // 在锁内提交 output 集合，并在锁外发布 metrics 与 resize 事件。
    fn update_effective_scale(&self, update: impl FnOnce(&mut WindowScaleState)) -> Option<i32> {
        // 中毒窗口状态不允许恢复后继续改写。
        let Ok(mut state) = self.state.lock() else {
            // 报告精确更新阶段。
            self.report_lock_failure("update effective scale");
            // 没有可安全应用的 scale。
            return None;
        };
        // 应用本次 output 集合事务。
        update(&mut state);
        // 同时跨多个 output 时采用最大整数 scale，避免任一输出欠采样。
        let next_scale = state
            // 遍历当前进入 output。
            .entered_outputs
            // 读取所有 scale。
            .values()
            // 选择最大值。
            .copied()
            // 无 output 时采用 fallback。
            .max()
            // fallback 始终为正。
            .unwrap_or(state.fallback_scale)
            // 防御异常非正值。
            .max(1);
        // 相同 scale 不重复排队 resize 或推进 surface revision。
        if next_scale == state.effective_scale {
            // output 集合已更新，但 drawable 事实未变化。
            return None;
        }
        // 提交新的有效 scale。
        state.effective_scale = next_scale;
        // 在触碰 metrics 与事件队列前释放 output 决策锁。
        drop(state);
        // 从同一 metrics 快照取得当前逻辑尺寸。
        let current = match self.metrics.snapshot() {
            // 健康快照继续提交新 scale。
            Ok(snapshot) => snapshot,
            // 失败只进入有界 owner failure source。
            Err(error) => {
                // 保存 typed 失败。
                let _ = self.pending_failures.enqueue(error);
                // 不发布不一致 resize。
                return None;
            }
        };
        // 原子更新 scale 与现有 logical extent。
        let updated = match self.metrics.update(
            // 保留当前逻辑宽度。
            current.logical_width,
            // 保留当前逻辑高度。
            current.logical_height,
            // 发布新 scale。
            next_scale,
        ) {
            // 健康更新建立新 drawable 事实。
            Ok(snapshot) => snapshot,
            // 失败不允许排队伪成功 resize。
            Err(error) => {
                // 保存 typed 失败。
                let _ = self.pending_failures.enqueue(error);
                // 停止本次通知。
                return None;
            }
        };
        // scale 变化通过同一窗口 resize 管线重建 graphics surface。
        match self.events.lock() {
            // 健康队列接纳目标窗口 logical resize。
            Ok(mut events) => {
                // 只发布 logical 尺寸，physical 换算留在 presentation 边界。
                events.push_back(
                    // 复用跨平台 resize 事实。
                    UiEvent::resize(updated.logical_width, updated.logical_height)
                        // 严格路由到当前窗口。
                        .for_window(self.window_id),
                );
            }
            // 中毒事件队列不能恢复后继续写入。
            Err(_) => {
                // 把投递失败转换为 owner-thread typed failure。
                let _ = self.pending_failures.enqueue(Error::new(
                    // 事件 owner 损坏属于 InvalidState。
                    Errc::InvalidState,
                    // 保留 scale 变化投递阶段。
                    "Wayland surface scale resize event queue mutex poisoned",
                ));
                // metrics 已保存真实 scale，后续 owner 恢复边界仍可观察失败。
            }
        }
        // 调用方据此立即提交 wl_surface buffer scale。
        Some(next_scale)
    }

    // 把逐窗 scale 状态锁失败写入 runtime failure source。
    fn report_lock_failure(&self, operation: &str) {
        // callback 端只做有界入队。
        let _ = self.pending_failures.enqueue(Error::new(
            // 状态 owner 损坏使用 InvalidState。
            Errc::InvalidState,
            // 保留精确操作名。
            format!("Wayland window scale state lock poisoned during {operation}"),
        ));
    }
}

// 为一个 wl_surface 绑定逐窗 output Enter/Leave 回调。
pub(crate) fn bind_surface_scale_events(
    surface: &Main<wl_surface::WlSurface>,
    registry: Arc<WaylandOutputScaleRegistry>,
    state: Arc<WaylandWindowScaleState>,
) {
    // registry 只保存弱引用，不改变窗口关闭顺序。
    registry.register_surface(&state);
    // surface 回调拥有 registry 与逐窗状态的共享句柄。
    surface.quick_assign(move |surface, event, _| {
        // 只处理 core wl_surface 的 output 生命周期事件。
        let changed_scale = match event {
            // Enter 把 output identity 与最新 registry scale 加入窗口集合。
            wl_surface::Event::Enter { output } => {
                // 读取稳定协议 identity。
                let output_id = output.id().protocol_id();
                // 查找该 output 最新整数 scale。
                let scale = registry.scale_for(output_id);
                // 更新目标窗口并取得可选新有效 scale。
                state.enter_output(output_id, scale)
            }
            // Leave 移除 output 并按剩余集合或默认 output 重算。
            wl_surface::Event::Leave { output } => {
                // 读取离开 output identity。
                let output_id = output.id().protocol_id();
                // 读取 backend 当前默认 scale。
                let fallback_scale = registry.preferred_scale();
                // 更新目标窗口并取得可选新有效 scale。
                state.leave_output(output_id, fallback_scale)
            }
            // 其他 surface 事件不改变 buffer scale。
            _ => None,
        };
        // 只有有效 scale 变化才提交协议请求。
        if let Some(scale) = changed_scale {
            // wl_surface core 协议要求正整数 buffer scale。
            surface.set_buffer_scale(scale);
        }
    });
}

// 纯状态测试不需要真实 Wayland compositor。
#[cfg(test)]
// 单元测试验证多 output、动态更新与逐窗事件路由。
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../../tests/unit/native/backends/linux/wayland/surface_scale__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
