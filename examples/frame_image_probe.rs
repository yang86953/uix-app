//! 合成帧的单原生窗口人工检查入口，不属于项目测试。
//! cargo run --locked --offline --features test-harness --example frame_image_probe
//! 只写 target/frame-image-codex/native，不加载用户媒体，不注册 Agent 后台工作面。

#[cfg(all(feature = "test-harness", feature = "image-codecs", feature = "vulkan"))]
mod native {
    use std::path::Path;
    use std::sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
        mpsc,
    };
    use std::thread::JoinHandle;
    use std::time::{Duration, Instant};

    use uix::draw::{FrameImage, SurfaceReadback};
    use uix::prelude::*;

    type ProbeResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

    #[derive(Clone, Default)]
    struct CurrentFrame {
        image: Option<FrameImage>,
        revision: u32,
    }

    fn synthetic_frame(revision: u32) -> Arc<Vec<u32>> {
        let colors = if revision == 0 {
            [0xffff3030, 0xff30d050, 0xff3070ff, 0xffffd030]
        } else {
            [0xff30d0ff, 0xffd040ff, 0xfff09030, 0xff80e080]
        };
        let mut pixels = Vec::with_capacity(160 * 90);
        for y in 0..90 {
            for x in 0..160 {
                let pixel = if x < 2 || x >= 158 || y < 2 || y >= 88 {
                    0xffffffff
                } else {
                    colors[usize::from(x >= 80) + 2 * usize::from(y >= 45)]
                };
                pixels.push(pixel);
            }
        }
        Arc::new(pixels)
    }

    fn save_snapshot(path: &Path, snapshot: SurfaceReadback) -> ProbeResult {
        let mut rgba = Vec::with_capacity(snapshot.pixels.len() * 4);
        for pixel in snapshot.pixels {
            rgba.extend_from_slice(&[
                (pixel >> 16) as u8,
                (pixel >> 8) as u8,
                pixel as u8,
                (pixel >> 24) as u8,
            ]);
        }
        image::save_buffer(
            path,
            &rgba,
            snapshot.width as u32,
            snapshot.height as u32,
            image::ColorType::Rgba8,
        )?;
        println!(
            "saved {} ({}x{} physical pixels)",
            path.display(),
            snapshot.width,
            snapshot.height
        );
        Ok(())
    }

    // 票据在设置状态的同一 UI turn 内请求；等待和文件 I/O 留在生产线程。
    // 整个示例最多一帧在途，不向 UI 队列无界投递；该同步仅供人工验收使用。
    fn publish_and_capture(
        handle: &AppHandle,
        current: &State<CurrentFrame>,
        next: CurrentFrame,
        cancelled: &Arc<AtomicBool>,
        output: &Path,
    ) -> ProbeResult {
        let current = current.clone();
        let capture_handle = handle.clone();
        let cancelled = Arc::clone(cancelled);
        let (send, receive) = mpsc::sync_channel(1);
        if !handle.is_open() {
            return Err("native window closed before publishing".into());
        }
        handle.post_to_ui(move || {
            if cancelled.load(Ordering::Acquire) || !capture_handle.is_open() {
                return;
            }
            current.set(next);
            let _ = send.send(capture_handle.request_surface_readback_for_test());
        });
        let ticket = receive.recv_timeout(Duration::from_secs(10))??;
        let snapshot = ticket.recv_timeout(Duration::from_secs(10))?;
        save_snapshot(output, snapshot)
    }

    pub fn run() -> ProbeResult {
        let current = State::new(CurrentFrame::default());
        let finished = Arc::new(AtomicBool::new(false));
        let cancelled = Arc::new(AtomicBool::new(false));
        let leases = Arc::new(Mutex::new(Vec::<Weak<Vec<u32>>>::new()));
        let worker = Arc::new(Mutex::new(None::<JoinHandle<ProbeResult>>));
        let output = Path::new("target/frame-image-codex/native");
        std::fs::create_dir_all(output)?;
        let started = Instant::now();

        let code = App::new()
            .title("UIX synthetic frames — native Vulkan probe")
            .size(640, 440)
            .graphics_backend(GraphicsBackend::Vulkan)
            .root({
                let current = current.clone();
                move || {
                    let current = current.clone();
                    canvas(640.0, 440.0, move |bounds, ctx| {
                        let current = current.get();
                        ctx.fill_rect(bounds, Color::from_rgb(24, 28, 36), None);
                        ctx.draw_text(
                            &format!("Synthetic frame {} — 160 x 90", current.revision),
                            Point::new(bounds.x + 20.0, bounds.y + 16.0),
                            Color::WHITE,
                            18.0,
                        );
                        let fit = Rect::new(bounds.x + 20.0, bounds.y + 84.0, 280.0, 280.0);
                        let fill = Rect::new(bounds.x + 340.0, bounds.y + 84.0, 280.0, 280.0);
                        for (rect, text) in [
                            (fit, "Contain / aspect preserved"),
                            (fill, "Stretch / fill"),
                        ] {
                            ctx.draw_text(
                                text,
                                Point::new(rect.x, rect.y - 28.0),
                                Color::WHITE,
                                14.0,
                            );
                            ctx.fill_rect(rect, Color::BLACK, None);
                        }
                        if let Some(image) = current.image.as_ref() {
                            ctx.draw_frame_image(image, fit);
                            ctx.draw_frame_image_fill(image, fill);
                        }
                        ctx.draw_text(
                            if current.image.is_some() {
                                "White frame border must remain visible on all sides."
                            } else {
                                "Stopped — no previous frame should remain."
                            },
                            Point::new(bounds.x + 20.0, bounds.y + 398.0),
                            Color::WHITE,
                            14.0,
                        );
                    })
                }
            })
            .on_start({
                let current = current.clone();
                let finished = Arc::clone(&finished);
                let cancelled = Arc::clone(&cancelled);
                let worker = Arc::clone(&worker);
                let leases = Arc::clone(&leases);
                move |handle| {
                    // 有界唤醒便于正常退出和 watchdog；窗口拆除取消该 timer。
                    handle
                        .run_interval(Duration::from_millis(100), || {})
                        .detach();
                    *worker.lock().unwrap() = Some(std::thread::spawn(move || {
                        let result = (|| -> ProbeResult {
                            for revision in 0..2 {
                                if cancelled.load(Ordering::Acquire) {
                                    return Err("probe cancelled".into());
                                }
                                let pixels = synthetic_frame(revision);
                                leases.lock().unwrap().push(Arc::downgrade(&pixels));
                                let frame = FrameImage::from_shared(160, 90, pixels)?;
                                publish_and_capture(
                                    &handle,
                                    &current,
                                    CurrentFrame {
                                        image: Some(frame),
                                        revision,
                                    },
                                    &cancelled,
                                    &output.join(format!("phase-{revision}.png")),
                                )?;
                            }
                            publish_and_capture(
                                &handle,
                                &current,
                                CurrentFrame {
                                    image: None,
                                    revision: 2,
                                },
                                &cancelled,
                                &output.join("cleared.png"),
                            )?;
                            let released = leases
                                .lock()
                                .unwrap()
                                .iter()
                                .all(|lease| lease.upgrade().is_none());
                            println!("all_source_arcs_released_after_clear={released}");
                            Ok(())
                        })();
                        finished.store(true, Ordering::Release);
                        handle.post_to_ui(|| {});
                        result
                    }));
                }
            })
            .on_exit({
                // on_exit 在原生 UiEvent 上检查，不是硬定时退出；空闲时可正常关闭本合成窗口。
                let finished = Arc::clone(&finished);
                move |_| {
                    finished.load(Ordering::Acquire) || started.elapsed() > Duration::from_secs(40)
                }
            })
            .run();

        cancelled.store(true, Ordering::Release);
        current.set(CurrentFrame::default());
        let worker = worker
            .lock()
            .unwrap()
            .take()
            .ok_or("native startup did not run on_start")?;
        worker.join().map_err(|_| "probe worker panicked")??;
        let released = leases
            .lock()
            .unwrap()
            .iter()
            .all(|lease| lease.upgrade().is_none());
        println!("native_exit_code={code}; all_source_arcs_released_after_shutdown={released}");
        if code != 0 || !released {
            return Err("native shutdown or source-frame release failed".into());
        }
        Ok(())
    }
}

#[cfg(all(feature = "test-harness", feature = "image-codecs", feature = "vulkan"))]
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    native::run()
}

#[cfg(not(all(feature = "test-harness", feature = "image-codecs", feature = "vulkan")))]
fn main() {
    eprintln!("requires test-harness, image-codecs and vulkan features");
}
