//! GFX-R5 证据场景运行器。

use super::*;

pub(super) fn present_solid(context: &mut VulkanContext, color: u32) -> GfxR5Result<()> {
    // 一次读取当前 Vulkan drawable 的完整 surface 快照。
    let (width, height) = drawable_extent(context);
    let pixels = vec![color; (width as usize) * (height as usize)];
    context
        .present_pixels(&pixels, width, height, PresentDamage::Full)
        .map_err(map_error)
}

pub(super) fn readback_one(context: &mut VulkanContext) -> GfxR5Result<u32> {
    // 通过 Vulkan PixelUpload recipe 的显式诊断辅助读取证据像素。
    context
        // 读取左上角单像素区域。
        .readback_pixels(0, 0, 1, 1)
        .map(|pixels| pixels[0])
        .map_err(map_error)
}

/// Evidence for the vendor/resize/present/readback exact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VendorResizeEvidence {
    pub adapter_diagnostic: String,
    pub vendor_id: u32,
    pub swapchain_maintenance1: bool,
    pub initial_drawable: (i32, i32),
    pub initial_readback: u32,
    pub resized_drawable: (i32, i32),
    pub resized_readback: u32,
}

/// Runs the real Win32 Vulkan vendor and resize boundary without making any
/// test assertions in the library.
pub fn run_vendor_resize_present_readback(
    window: &mut NativeWindow,
) -> GfxR5Result<VendorResizeEvidence> {
    let surface = window.surface()?;
    let mut context = VulkanContext::new(surface, 137, 103).map_err(map_error)?;
    let evidence = (|| {
        let adapter_diagnostic = context.adapter_info.diagnostic_summary();
        let vendor_id = context.adapter_info.vendor_id;
        let swapchain_maintenance1 = context.swapchain_maintenance1_enabled_for_test();
        let initial_drawable = drawable_extent(&context);
        present_solid(&mut context, 0xFF3478BC)?;
        let initial_readback = readback_one(&mut context)?;

        window.resize(211, 149)?;
        // Vulkan GFX-R5 通过专用 PixelUpload surface 执行 resize。
        context
            // 使用与生产 PixelUpload runtime 相同的 typed 契约。
            .resize_pixel_upload_surface(211, 149)
            // 将 UIX 错误转换为公开测试证据错误。
            .map_err(map_error)?;
        let resized_drawable = drawable_extent(&context);
        present_solid(&mut context, 0xFF9A5C21)?;
        let resized_readback = readback_one(&mut context)?;

        Ok(VendorResizeEvidence {
            adapter_diagnostic,
            vendor_id,
            swapchain_maintenance1,
            initial_drawable,
            initial_readback,
            resized_drawable,
            resized_readback,
        })
    })();
    context.try_shutdown().map_err(map_error)?;
    evidence
}

/// Evidence for a native out-of-date fault followed by an explicit surface
/// resize and recovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeOutOfDateEvidence {
    pub adapter_diagnostic: String,
    pub fault: GfxR5Error,
    pub initial_drawable: (i32, i32),
    pub logical_extent: (i32, i32),
    pub resized_drawable: (i32, i32),
    pub recovered_readback: u32,
}

/// Runs the native Vulkan out-of-date transition and returns the typed fault
/// for assertions in the integration test.
pub fn run_native_out_of_date_recovery(
    window: &mut NativeWindow,
) -> GfxR5Result<NativeOutOfDateEvidence> {
    let surface = window.surface()?;
    let mut context = VulkanContext::new(surface, 137, 103).map_err(map_error)?;
    let evidence = (|| {
        let adapter_diagnostic = context.adapter_info.diagnostic_summary();
        let initial_drawable = drawable_extent(&context);
        present_solid(&mut context, 0xFF3478BC)?;

        window.resize(223, 157)?;
        let stale_width = initial_drawable.0;
        let stale_height = initial_drawable.1;
        let stale_pixels = vec![0xFF3478BC; (stale_width as usize) * (stale_height as usize)];
        let fault = match context.present_pixels(
            &stale_pixels,
            stale_width,
            stale_height,
            PresentDamage::Full,
        ) {
            Ok(()) => {
                return Err(support_failure(
                    "GFX-R5 native resize did not produce ERROR_OUT_OF_DATE_KHR",
                ))
            }
            Err(error) => map_error(error),
        };
        let logical_extent = (223, 157);

        // out-of-date 恢复通过专用 PixelUpload surface 重建 swapchain。
        context
            // 使用证据场景请求的目标逻辑尺寸。
            .resize_pixel_upload_surface(logical_extent.0, logical_extent.1)
            // 映射为 GFX-R5 稳定错误证据。
            .map_err(map_error)?;
        let resized_drawable = drawable_extent(&context);
        present_solid(&mut context, 0xFFB7642D)?;
        let recovered_readback = readback_one(&mut context)?;

        Ok(NativeOutOfDateEvidence {
            adapter_diagnostic,
            fault,
            initial_drawable,
            logical_extent,
            resized_drawable,
            recovered_readback,
        })
    })();
    context.try_shutdown().map_err(map_error)?;
    evidence
}

/// Evidence for attempting to create a Vulkan surface from a destroyed HWND.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FatalSurfaceEvidence {
    pub adapter_diagnostic: String,
    pub fault: GfxR5Error,
}

/// Runs the fatal native-surface exact against a real destroyed Win32 HWND.
pub fn run_destroyed_hwnd_surface_lost(
    window: &mut NativeWindow,
) -> GfxR5Result<FatalSurfaceEvidence> {
    let surface = window.surface()?;
    let mut context = VulkanContext::new(surface, 137, 103).map_err(map_error)?;
    let adapter_diagnostic = context.adapter_info.diagnostic_summary();
    present_solid(&mut context, 0xFF3478BC)?;
    context.try_shutdown().map_err(map_error)?;
    drop(context);
    window.close()?;

    let fault = match VulkanContext::new(surface, 137, 103) {
        Ok(mut replacement) => {
            let _ = replacement.try_shutdown();
            return Err(support_failure(
                "GFX-R5 destroyed HWND unexpectedly created a Vulkan context",
            ));
        }
        Err(error) => map_error(error),
    };
    Ok(FatalSurfaceEvidence {
        adapter_diagnostic,
        fault,
    })
}

struct VulkanRecoveryTarget {
    context: VulkanContext,
    canvas: NoopCanvas2D,
    recovery_evidence: Arc<Mutex<Option<(i32, i32, u32)>>>,
}

impl VulkanRecoveryTarget {
    fn new(context: VulkanContext, recovery_evidence: Arc<Mutex<Option<(i32, i32, u32)>>>) -> Self {
        Self {
            context,
            canvas: NoopCanvas2D,
            recovery_evidence,
        }
    }
}

impl RenderTarget for VulkanRecoveryTarget {
    fn initialize(&mut self, _width: i32, _height: i32) -> Result<(), Error> {
        Ok(())
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        self.context.try_shutdown()
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        // 恢复 target 复用 Vulkan PixelUpload 的专用 surface resize。
        self.context.resize_pixel_upload_surface(width, height)
    }

    fn begin_frame(&mut self, _strategy: UpdateStrategy) -> RenderOutcome {
        RenderOutcome::FrameReady(DamageRegion::full())
    }

    fn end_frame(&mut self, _present_damage: &DamageRegion) -> RenderOutcome {
        RenderOutcome::Present(DamageRegion::full())
    }

    fn test_present(&mut self) -> Result<PresentTestResult, Error> {
        // 一次读取恢复 target 的 drawable extent。
        let (width, height) = drawable_extent(&self.context);
        let pixels = vec![0xFFB7642D; (width as usize) * (height as usize)];
        self.context
            .present_pixels(&pixels, width, height, PresentDamage::Full)?;
        // 通过显式诊断辅助读取恢复后的左上角证据像素。
        let readback = self.context.readback_pixels(0, 0, 1, 1)?[0];
        // 毒锁恢复：Mutex 中毒时取回内部值，不把锁竞争转化为 panic。
        *self
            .recovery_evidence
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some((width, height, readback));
        Ok(PresentTestResult::Presentable)
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        &mut self.canvas
    }

    fn logical_extent(&mut self) -> (i32, i32) {
        // GFX-R5 target 直接报告当前 PixelUpload drawable extent。
        drawable_extent(&self.context)
    }
}

/// Evidence for a native surface fault crossing the engine recovery driver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineRecoveryEvidence {
    pub adapter_diagnostic: String,
    pub fault: GfxR5Error,
    pub action: &'static str,
    pub logical_extent: (i32, i32),
    pub drawable_extent: (i32, i32),
    pub recovered_readback: u32,
}

/// Runs the real Vulkan fault through the bounded engine recovery wrapper.
pub fn run_engine_recovery_boundary(
    window: &mut NativeWindow,
) -> GfxR5Result<EngineRecoveryEvidence> {
    let surface = window.surface()?;
    let mut context = VulkanContext::new(surface, 137, 103).map_err(map_error)?;
    let adapter_diagnostic = context.adapter_info.diagnostic_summary();
    present_solid(&mut context, 0xFF3478BC)?;

    window.resize(229, 163)?;
    // 从同一 surface 快照保存 fault 前的 stale drawable extent。
    let (stale_width, stale_height) = drawable_extent(&context);
    let stale_pixels = vec![0xFF3478BC; (stale_width as usize) * (stale_height as usize)];
    let fault = match context.present_pixels(
        &stale_pixels,
        stale_width,
        stale_height,
        PresentDamage::Full,
    ) {
        Ok(()) => {
            return Err(support_failure(
                "GFX-R5 engine recovery did not observe ERROR_OUT_OF_DATE_KHR",
            ))
        }
        Err(error) => error,
    };
    let fault_evidence = map_error(fault.clone());

    let recovery_evidence = Arc::new(Mutex::new(None));
    let rebuilder_evidence = Arc::clone(&recovery_evidence);
    let rebuilder: RenderTargetRebuilder = Box::new(move |action, width, height| {
        if action != RecoveryAction::RebuildSurface {
            return Err(Error::new(
                Errc::InvalidState,
                format!("GFX-R5 expected RebuildSurface, got {action:?}"),
            ));
        }
        if (width, height) != (229, 163) {
            return Err(Error::new(
                Errc::InvalidArgument,
                format!("GFX-R5 recovery extent was {width}x{height}, expected 229x163"),
            ));
        }
        let replacement = VulkanContext::new(surface, width, height)?;
        Ok(Box::new(VulkanRecoveryTarget::new(
            replacement,
            Arc::clone(&rebuilder_evidence),
        )) as Box<dyn RenderTarget>)
    });
    let target = VulkanRecoveryTarget::new(context, Arc::clone(&recovery_evidence));
    let mut driver = RecoveryDriver::new(Box::new(target), rebuilder).with_extent(229, 163);
    driver.external_present_failed(fault);

    let outcome = driver.begin_frame(UpdateStrategy::FullRedraw);
    if !matches!(outcome, RenderOutcome::FrameReady(_)) {
        let _ = driver.try_shutdown();
        return Err(support_failure(format!(
            "GFX-R5 engine recovery did not resume after rebuild: {outcome:?}"
        )));
    }
    if driver.test_present().map_err(map_error)? != PresentTestResult::Presentable {
        let _ = driver.try_shutdown();
        return Err(support_failure(
            "GFX-R5 engine recovery replacement was not presentable",
        ));
    }

    let recovered = recovery_evidence
        .lock()
        .map_err(|_| support_failure("GFX-R5 recovery evidence lock poisoned"))?
        .ok_or_else(|| support_failure("GFX-R5 recovery present evidence missing"))?;
    driver.external_present_succeeded();
    driver.try_shutdown().map_err(map_error)?;

    Ok(EngineRecoveryEvidence {
        adapter_diagnostic,
        fault: fault_evidence,
        action: "RebuildSurface",
        logical_extent: (229, 163),
        drawable_extent: (recovered.0, recovered.1),
        recovered_readback: recovered.2,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoakMeasurements {
    pub duration_seconds: u64,
    pub rounds: u64,
    pub handles_before: u32,
    pub handles_after: u32,
    pub peak_handles: u32,
    pub warmup_seconds: u64,
    pub warmup_rounds: u64,
}

/// Evidence for a bounded single-window resize/present soak.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoakEvidence {
    pub adapter_diagnostic: String,
    pub measurements: SoakMeasurements,
    pub swapchain_maintenance1: bool,
}

fn configured_soak_seconds() -> GfxR5Result<u64> {
    let seconds = std::env::var("UIX_VULKAN_SOAK_SECONDS")
        .ok()
        .map(|value| {
            value.parse::<u64>().map_err(|_| {
                support_failure(format!("GFX-R5 soak duration is not an integer: {value}"))
            })
        })
        .transpose()?
        .unwrap_or(900);
    if !(900..=3600).contains(&seconds) {
        return Err(support_failure(format!(
            "GFX-R5 soak duration must be 900..=3600 seconds, got {seconds}"
        )));
    }
    Ok(seconds)
}

fn gui_handle_count() -> GfxR5Result<u32> {
    // SAFETY：GetCurrentProcess 返回当前进程的伪句柄，无参数、不写调用方内存。
    let process = unsafe { GetCurrentProcess() };
    // SAFETY：伪句柄对 GetGuiResources 有效；两个调用均为只读统计查询，不写内存。
    let gdi = unsafe { GetGuiResources(process, GR_GDIOBJECTS) };
    let user = unsafe { GetGuiResources(process, GR_USEROBJECTS) };
    let count = gdi.saturating_add(user);
    if count == 0 {
        return Err(support_failure(
            "GFX-R5 GetGuiResources returned zero for the live test process",
        ));
    }
    Ok(count)
}

fn soak_round(
    window: &mut NativeWindow,
    context: &mut VulkanContext,
    round: u64,
) -> GfxR5Result<()> {
    let (width, height, color) = if round % 2 == 0 {
        (137, 103, 0xFF3478BC)
    } else {
        (211, 149, 0xFF9A5C21)
    };
    window.resize(width, height)?;
    // soak 每轮都通过专用 PixelUpload surface 更新 swapchain。
    context
        // 保持与生产 runtime 相同的 resize 生命周期。
        .resize_pixel_upload_surface(width, height)
        // 映射为 GFX-R5 稳定错误证据。
        .map_err(map_error)?;
    present_solid(context, color)
}

fn shared_soak_round(
    first_window: &mut NativeWindow,
    first_context: &mut VulkanContext,
    second_window: &mut NativeWindow,
    second_context: &mut VulkanContext,
    round: u64,
) -> GfxR5Result<()> {
    soak_round(first_window, first_context, round)?;
    soak_round(second_window, second_context, round.wrapping_add(1))
}

fn soak_warmup_single(
    window: &mut NativeWindow,
    context: &mut VulkanContext,
) -> GfxR5Result<(u64, u64)> {
    let started = Instant::now();
    let mut rounds = 0;
    while rounds < 8_192 || started.elapsed() < Duration::from_secs(60) {
        soak_round(window, context, rounds)?;
        rounds += 1;
    }
    Ok((started.elapsed().as_secs().max(60), rounds))
}

fn soak_warmup_shared(
    first_window: &mut NativeWindow,
    first_context: &mut VulkanContext,
    second_window: &mut NativeWindow,
    second_context: &mut VulkanContext,
) -> GfxR5Result<(u64, u64)> {
    let started = Instant::now();
    let mut rounds = 0;
    while rounds < 8_192 || started.elapsed() < Duration::from_secs(60) {
        shared_soak_round(
            first_window,
            first_context,
            second_window,
            second_context,
            rounds,
        )?;
        rounds += 1;
    }
    Ok((started.elapsed().as_secs().max(60), rounds))
}

/// Runs the configured 15–60 minute single-window Vulkan soak.
pub fn run_single_window_soak(window: &mut NativeWindow) -> GfxR5Result<SoakEvidence> {
    let seconds = configured_soak_seconds()?;
    let surface = window.surface()?;
    let mut context = VulkanContext::new(surface, 137, 103).map_err(map_error)?;
    let adapter_diagnostic = context.adapter_info.diagnostic_summary();
    let swapchain_maintenance1 = context.swapchain_maintenance1_enabled_for_test();
    let handles_before = gui_handle_count()?;
    let run_result = (|| {
        let (warmup_seconds, warmup_rounds) = soak_warmup_single(window, &mut context)?;
        let started = Instant::now();
        let mut rounds = 0;
        let mut peak_handles = handles_before;
        while started.elapsed() < Duration::from_secs(seconds) {
            soak_round(window, &mut context, rounds)?;
            rounds += 1;
            peak_handles = peak_handles.max(gui_handle_count()?);
        }
        Ok(SoakMeasurements {
            duration_seconds: seconds,
            rounds,
            handles_before,
            handles_after: 0,
            peak_handles,
            warmup_seconds,
            warmup_rounds,
        })
    })();
    let shutdown_result = context.try_shutdown().map_err(map_error);
    let measurements = match (run_result, shutdown_result) {
        (Ok(measurements), Ok(())) => measurements,
        (Err(error), Ok(())) => return Err(error),
        (Ok(_), Err(error)) => return Err(error),
        (Err(error), Err(shutdown)) => {
            return Err(support_failure(format!(
                "{error}; GFX-R5 soak cleanup failed: {shutdown}"
            )))
        }
    };
    Ok(SoakEvidence {
        adapter_diagnostic,
        measurements: SoakMeasurements {
            handles_after: gui_handle_count()?,
            ..measurements
        },
        swapchain_maintenance1,
    })
}

/// Runs the configured 15–60 minute two-window shared-device Vulkan soak.
pub fn run_shared_device_soak(window: &mut NativeWindow) -> GfxR5Result<SoakEvidence> {
    let seconds = configured_soak_seconds()?;
    let mut second_window = NativeWindow::new(149, 107)?;
    let first_surface = window.surface()?;
    let second_surface = second_window.surface()?;
    let mut first_context = VulkanContext::new(first_surface, 137, 103).map_err(map_error)?;
    let mut second_context = VulkanContext::new(second_surface, 149, 107).map_err(map_error)?;
    if first_context.shared_device_identity() == 0
        || first_context.shared_device_identity() != second_context.shared_device_identity()
    {
        return Err(support_failure(
            "GFX-R5 shared-device soak did not acquire one logical Vulkan device",
        ));
    }
    let adapter_diagnostic = first_context.adapter_info.diagnostic_summary();
    let swapchain_maintenance1 = first_context.swapchain_maintenance1_enabled_for_test()
        && second_context.swapchain_maintenance1_enabled_for_test();
    let handles_before = gui_handle_count()?;
    let run_result = (|| {
        let (warmup_seconds, warmup_rounds) = soak_warmup_shared(
            window,
            &mut first_context,
            &mut second_window,
            &mut second_context,
        )?;
        let started = Instant::now();
        let mut rounds = 0;
        let mut peak_handles = handles_before;
        while started.elapsed() < Duration::from_secs(seconds) {
            shared_soak_round(
                window,
                &mut first_context,
                &mut second_window,
                &mut second_context,
                rounds,
            )?;
            rounds += 1;
            peak_handles = peak_handles.max(gui_handle_count()?);
        }
        Ok(SoakMeasurements {
            duration_seconds: seconds,
            rounds,
            handles_before,
            handles_after: 0,
            peak_handles,
            warmup_seconds,
            warmup_rounds,
        })
    })();
    let first_shutdown = first_context.try_shutdown().map_err(map_error);
    let second_shutdown = second_context.try_shutdown().map_err(map_error);
    let measurements = match (run_result, first_shutdown, second_shutdown) {
        (Ok(measurements), Ok(()), Ok(())) => measurements,
        (Err(error), Ok(()), Ok(())) => return Err(error),
        (Ok(_), Err(error), Ok(())) | (Ok(_), Ok(()), Err(error)) => return Err(error),
        (Ok(_), Err(first), Err(second)) => {
            return Err(support_failure(format!(
                "GFX-R5 shared soak cleanup failed: first={first}; second={second}"
            )))
        }
        (Err(error), first, second) => {
            return Err(support_failure(format!(
                "{error}; GFX-R5 shared soak cleanup failed: first={first:?}; second={second:?}"
            )))
        }
    };
    Ok(SoakEvidence {
        adapter_diagnostic,
        measurements: SoakMeasurements {
            handles_after: gui_handle_count()?,
            ..measurements
        },
        swapchain_maintenance1,
    })
}

/// Evidence for the shared logical-device external-reset/replacement exact.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceLostEvidence {
    pub adapter_diagnostic: String,
    pub detector: &'static str,
    pub frames: u32,
    pub surface_faults: u32,
    pub detection_seconds: f64,
    pub recovery_seconds: f64,
    pub device_fault: bool,
    pub fault: GfxR5Error,
    pub peer: GfxR5Error,
    pub replacement_attempts: u32,
    pub replacement: String,
}

/// Injects the existing Vulkan shared-device loss hook, proves the typed
/// peer error, then acquires and presents with a replacement logical device.
pub fn run_external_device_loss_recovery(
    window: &mut NativeWindow,
) -> GfxR5Result<DeviceLostEvidence> {
    let surface = window.surface()?;
    let peer_window = NativeWindow::new(137, 103)?;
    let peer_surface = peer_window.surface()?;
    let mut primary = VulkanContext::new(surface, 137, 103).map_err(map_error)?;
    let mut peer_context = VulkanContext::new(peer_surface, 137, 103).map_err(map_error)?;
    if primary.shared_device_identity() == 0
        || primary.shared_device_identity() != peer_context.shared_device_identity()
    {
        return Err(support_failure(
            "GFX-R5 device-loss exact did not acquire a shared logical device",
        ));
    }
    let adapter_diagnostic = primary.adapter_info.diagnostic_summary();
    let device_fault = primary.device_fault_reporting_enabled_for_test();
    present_solid(&mut primary, 0xFF3478BC)?;
    primary.mark_shared_device_lost_for_test();
    let detection_started = Instant::now();
    let fault = match present_solid(&mut primary, 0xFF9A5C21) {
        Ok(()) => {
            return Err(support_failure(
                "GFX-R5 external reset hook did not return GraphicsDeviceLost",
            ))
        }
        Err(error) if error.code == "graphics_device_lost" => error,
        Err(error) => {
            return Err(support_failure(format!(
                "GFX-R5 external reset hook returned {} instead of graphics_device_lost",
                error.code
            )))
        }
    };
    let detection_seconds = detection_started.elapsed().as_secs_f64();
    let peer_error = match present_solid(&mut peer_context, 0xFF9A5C21) {
        Ok(()) => {
            return Err(support_failure(
                "GFX-R5 shared peer did not observe GraphicsDeviceLost",
            ))
        }
        Err(error) if error.code == "graphics_device_lost" => error,
        Err(error) => {
            return Err(support_failure(format!(
                "GFX-R5 shared peer returned {} instead of graphics_device_lost",
                error.code
            )))
        }
    };

    primary.try_shutdown().map_err(map_error)?;
    peer_context.try_shutdown().map_err(map_error)?;
    drop(primary);
    drop(peer_context);

    let recovery_started = Instant::now();
    let mut replacement = VulkanContext::new(surface, 137, 103).map_err(map_error)?;
    let replacement_diagnostic = replacement.adapter_info.diagnostic_summary();
    present_solid(&mut replacement, 0xFFB7642D)?;
    replacement.try_shutdown().map_err(map_error)?;
    let recovery_seconds = recovery_started.elapsed().as_secs_f64();

    Ok(DeviceLostEvidence {
        adapter_diagnostic,
        detector: "first",
        frames: 2,
        surface_faults: 0,
        detection_seconds,
        recovery_seconds,
        device_fault,
        fault,
        peer: peer_error,
        replacement_attempts: 1,
        replacement: replacement_diagnostic,
    })
}
