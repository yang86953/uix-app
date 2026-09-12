// 各项自带原 cfg 门控（uix_gpu_parity_* 或含 test/生产分支的 any 组合），
// 在源文件模块作用域 include! 展开。

// 确定性验证共享 device loss 仍由两个窗口各自消费一次，不引入全局恢复协调。
#[cfg(any(uix_gpu_parity_vulkan, uix_gpu_parity_opengl, uix_gpu_parity_d3d11))]
pub(crate) fn run_multi_window_device_loss_contract_test() {
    use crate::core::Errc;
    use crate::draw::backend::cpu::noop_canvas_2d::NoopCanvas2D;
    use std::sync::atomic::AtomicUsize;

    struct WindowTarget {
        shared_lost: Arc<AtomicBool>,
        canvas: NoopCanvas2D,
    }

    impl RenderTarget for WindowTarget {
        fn initialize(&mut self, _width: i32, _height: i32) -> Result<(), Error> {
            Ok(())
        }

        fn try_shutdown(&mut self) -> Result<(), Error> {
            Ok(())
        }

        fn resize(&mut self, _width: i32, _height: i32) -> Result<(), Error> {
            Ok(())
        }

        fn begin_frame(&mut self, _strategy: UpdateStrategy) -> RenderOutcome {
            if self.shared_lost.load(Ordering::Acquire) {
                return RenderOutcome::Failed(GraphicsFailure::DeviceLost(Error::new(
                    Errc::GraphicsDeviceLost,
                    "shared graphics device lost in deterministic window test",
                )));
            }
            RenderOutcome::FrameReady(DamageRegion::full())
        }

        fn end_frame(&mut self, _present_damage: &DamageRegion) -> RenderOutcome {
            RenderOutcome::Present(DamageRegion::full())
        }

        fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
            &mut self.canvas
        }
    }

    fn driver(shared_lost: Arc<AtomicBool>, rebuilds: Arc<AtomicUsize>) -> RecoveryDriver {
        let rebuilder = Box::new(move |_action, _width, _height| {
            rebuilds.fetch_add(1, Ordering::AcqRel);
            Ok(Box::new(WindowTarget {
                shared_lost: Arc::new(AtomicBool::new(false)),
                canvas: NoopCanvas2D,
            }) as Box<dyn RenderTarget>)
        });
        RecoveryDriver::new(
            Box::new(WindowTarget {
                shared_lost,
                canvas: NoopCanvas2D,
            }),
            rebuilder,
        )
        .with_extent(64, 64)
    }

    let shared_lost = Arc::new(AtomicBool::new(true));
    let first_rebuilds = Arc::new(AtomicUsize::new(0));
    let second_rebuilds = Arc::new(AtomicUsize::new(0));
    let mut first = driver(Arc::clone(&shared_lost), Arc::clone(&first_rebuilds));
    let mut second = driver(shared_lost, Arc::clone(&second_rebuilds));

    for window in [&mut first, &mut second] {
        assert!(matches!(
            window.begin_frame(UpdateStrategy::FullRedraw),
            RenderOutcome::Failed(GraphicsFailure::DeviceLost(_))
        ));
        assert!(matches!(
            window.begin_frame(UpdateStrategy::FullRedraw),
            RenderOutcome::FrameReady(_)
        ));
        assert!(matches!(
            window.begin_frame(UpdateStrategy::FullRedraw),
            RenderOutcome::FrameReady(_)
        ));
    }
    assert_eq!(first_rebuilds.load(Ordering::Acquire), 1);
    assert_eq!(second_rebuilds.load(Ordering::Acquire), 1);
}
