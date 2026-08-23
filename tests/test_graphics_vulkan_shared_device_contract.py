"""Vulkan 多窗口共享 device、逐窗口资源与恢复责任的架构门禁。"""

from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
ADAPTER = ROOT / "src/native/presentation/graphics/vulkan/adapter"
DEVICE = ADAPTER / "device.rs"
CONTEXT = ADAPTER / "context/mod.rs"
METHODS = ADAPTER / "context/methods.rs"
RHI_DEVICE = ADAPTER / "context/rhi_device.rs"
RHI_SURFACE = ADAPTER / "context/rhi_surface.rs"
RECOVERY = ROOT / "src/draw/renderer/recovery_driver.rs"
CPU_MOD = ROOT / "src/draw/backend/cpu/mod.rs"
FAULT = ADAPTER / "fault.rs"
SURFACE = ADAPTER / "surface.rs"
DOC = ROOT / "docs/架构/graphics/vulkan-multi-window.md"
NATIVE_TEST = ROOT / "tests/vulkan_gpu_parity.rs"


class VulkanSharedDeviceContractTests(unittest.TestCase):
    """防止共享 device 与逐窗口 surface/sync 责任重新混合。"""

    def test_runtime_is_thread_bound_and_reuses_only_healthy_device(self) -> None:
        source = DEVICE.read_text(encoding="utf-8")

        self.assertIn("static THREAD_RUNTIME: RefCell<Weak<VulkanRuntime>>", source)
        self.assertIn("_thread_bound: PhantomData<Rc<()>>", source)
        self.assertIn("type DeviceKey = (vk::PhysicalDevice, u32);", source)
        self.assertIn("devices: RefCell<HashMap<DeviceKey, Weak<VulkanDevice>>>", source)
        self.assertIn("if !device.is_lost()", source)
        self.assertIn("devices.insert(key, Rc::downgrade(&device))", source)

    def test_swapchain_maintenance_enables_required_instance_extension(self) -> None:
        device = DEVICE.read_text(encoding="utf-8")
        surface = SURFACE.read_text(encoding="utf-8")

        self.assertIn("ash::khr::get_surface_capabilities2::NAME.as_ptr()", surface)
        self.assertIn("ash::ext::surface_maintenance1::NAME.as_ptr()", surface)
        self.assertIn("surface_maintenance1: bool", device)
        self.assertIn("runtime.surface_maintenance1", device)
        self.assertLess(
            device.index("runtime.surface_maintenance1"),
            device.index("query_swapchain_maintenance1("),
        )

    def test_shared_queue_has_one_serial_entry_and_context_has_no_queue_owner(self) -> None:
        device = DEVICE.read_text(encoding="utf-8")
        context = CONTEXT.read_text(encoding="utf-8")
        methods = METHODS.read_text(encoding="utf-8")
        rhi_device = RHI_DEVICE.read_text(encoding="utf-8")
        rhi_surface = RHI_SURFACE.read_text(encoding="utf-8")

        self.assertIn("queue_serial: QueueSerial", device)
        self.assertEqual(device.count("pub(super) fn with_queue<T>"), 1)
        self.assertNotIn("pub(super) fn queue(&self)", device)
        self.assertNotIn("queue: vk::Queue", context)
        for source in (methods, rhi_device, rhi_surface):
            self.assertIn("with_queue(", source)

    def test_every_context_uniquely_owns_surface_and_sync_identity(self) -> None:
        context = CONTEXT.read_text(encoding="utf-8")

        for field in (
            "surface: vk::SurfaceKHR",
            "swapchain: vk::SwapchainKHR",
            "command_pool: vk::CommandPool",
            "image_available: vk::Semaphore",
            "render_finished: Vec<vk::Semaphore>",
            "present_fences: PresentFenceSet",
            "frame_fence: vk::Fence",
            "acquired_frame: Option<VulkanAcquiredFrame>",
            "submitted_frame: Option<VulkanSubmittedFrame>",
            "rhi_device: VulkanRhiDevice",
        ):
            self.assertEqual(context.count(field), 1, field)

    def test_peer_loss_preflight_and_old_generation_shutdown_are_explicit(self) -> None:
        device = DEVICE.read_text(encoding="utf-8")
        methods = METHODS.read_text(encoding="utf-8")

        active = methods[methods.index("pub(super) fn active_device") :]
        active = active[: active.index("fn swapchain_maintenance1_enabled")]
        self.assertLess(active.index("device.ensure_healthy()?"), active.index("Ok(device)"))
        shutdown = methods[methods.index("pub(super) fn shutdown_result") :]
        self.assertIn("let wait_result = if device.is_lost()", shutdown)
        self.assertLess(shutdown.index("self.surface_loader.destroy_surface"), shutdown.index("self.device_lease.take()"))
        self.assertEqual(device.count("self.device.destroy_device(None)"), 1)

    def test_real_and_deterministic_loss_recovery_tests_are_wired(self) -> None:
        device = DEVICE.read_text(encoding="utf-8")
        recovery = RECOVERY.read_text(encoding="utf-8")
        native = NATIVE_TEST.read_text(encoding="utf-8")

        for marker in (
            "healthy shared device must be reused",
            "every old-device peer must observe the same typed loss",
            "lost device must be replaced",
            "old.upgrade().is_none()",
            "shared-device empty submit",
        ):
            self.assertIn(marker, device)
        self.assertIn("run_multi_window_device_loss_contract_test", recovery)
        self.assertIn("assert_eq!(first_rebuilds.load(Ordering::Acquire), 1)", recovery)
        self.assertIn("assert_eq!(second_rebuilds.load(Ordering::Acquire), 1)", recovery)
        self.assertNotIn("EventBus", recovery)
        self.assertIn("__run_vulkan_shared_device_contract_test", native)

    def test_architecture_document_and_touched_files_stay_bounded(self) -> None:
        document = DOC.read_text(encoding="utf-8")
        self.assertIn("ThreadBound VulkanRuntime", document)
        self.assertIn("Window A VulkanContext", document)
        self.assertIn("Lost(device N)", document)
        for path in (
            DEVICE,
            CONTEXT,
            METHODS,
            RHI_DEVICE,
            RHI_SURFACE,
            RECOVERY,
            CPU_MOD,
            FAULT,
            DOC,
            Path(__file__),
        ):
            self.assertLessEqual(len(path.read_text(encoding="utf-8").splitlines()), 1500, path)


if __name__ == "__main__":
    unittest.main()
