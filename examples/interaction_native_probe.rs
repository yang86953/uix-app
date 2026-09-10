//! 单窗口原生 Vulkan 交互/视觉人工检查，不是项目测试。
//! cargo run --locked --offline --features test-harness --example interaction_native_probe
//! 使用公开应用内动作与 surface 回读；不创建/改绑 Agent 后台工作面。
//! 可显式设置公开 test-harness 的 UIX_AUTOMATION_DIR，独立核对原生只读语义导出。

#[cfg(all(
    feature = "test-harness",
    feature = "image-codecs",
    feature = "vulkan",
    feature = "navigation",
    feature = "feedback"
))]
mod native {
    use std::path::PathBuf;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    };
    use std::thread::JoinHandle;
    use std::time::{Duration, Instant};
    use uix_app::prelude::*;
    use uix_app::ui::test_harness::AUTOMATION_DIR_ENV;

    type ProbeResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

    fn output_directory() -> PathBuf {
        std::env::var_os("UIX_INTERACTION_PROBE_OUTPUT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("target/interaction-codex/native"))
    }

    fn capture_semantics(handle: &AppHandle, name: &str) -> ProbeResult {
        let Some(directory) = std::env::var_os(AUTOMATION_DIR_ENV) else {
            return Ok(());
        };
        // GPU 票据完成后跨过一次 UI 队列边界，让同轮 WindowDriver 完成帧尾只读导出。
        // 不调用 WidgetTree 私有 API，不刷新/点击目标行，不把 PNG 当作语义快照。
        let (send, receive) = mpsc::sync_channel(1);
        handle.post_to_ui(move || {
            let _ = send.send(());
        });
        receive.recv_timeout(Duration::from_secs(3))?;
        let source = PathBuf::from(directory).join(format!(
            "uix-{}-window-{}.json",
            std::process::id(),
            handle.window_id().raw(),
        ));
        let bytes = std::fs::read(&source)?;
        // 保存后由独立观察者解析 JSON 并核对 PID/窗口/修订/可见 bounds，
        // 不为人工诊断示例增加运行期 JSON codec feature 或私有树访问。
        std::fs::write(
            output_directory().join(format!("{name}.snapshot.json")),
            bytes,
        )?;
        println!(
            "saved native semantic export: {name}; source={}",
            source.display()
        );
        Ok(())
    }

    fn capture(handle: &AppHandle, tick: &State<u32>, name: &str) -> ProbeResult {
        let capture_handle = handle.clone();
        let tick = tick.clone();
        let (send, receive) = mpsc::sync_channel(1);
        handle.post_to_ui(move || {
            // 只失效独立的 header canvas，不重建/触碰列表行来制造“自愈”。
            tick.update(|value| *value += 1);
            let _ = send.send(capture_handle.request_surface_readback_for_test());
        });
        let snapshot = receive
            .recv_timeout(Duration::from_secs(10))??
            .recv_timeout(Duration::from_secs(10))?;
        let mut rgba = Vec::with_capacity(snapshot.pixels.len() * 4);
        for pixel in snapshot.pixels {
            rgba.extend_from_slice(&[
                (pixel >> 16) as u8,
                (pixel >> 8) as u8,
                pixel as u8,
                (pixel >> 24) as u8,
            ]);
        }
        let path = output_directory().join(format!("{name}.png"));
        image::save_buffer(
            &path,
            &rgba,
            snapshot.width as u32,
            snapshot.height as u32,
            image::ColorType::Rgba8,
        )?;
        println!(
            "saved {} ({}x{})",
            path.display(),
            snapshot.width,
            snapshot.height
        );
        capture_semantics(handle, name)
    }

    fn action(handle: &AppHandle, target: &str, action: SemanticAction) -> ProbeResult {
        handle
            .perform_automation_action(target, action)?
            .recv_timeout(Duration::from_secs(10))?
            .map_err(|error| -> Box<dyn std::error::Error + Send + Sync> { error.into() })?;
        println!("public action accepted and executed: {target}");
        Ok(())
    }

    fn set_on_ui<T: Clone + Send + Sync + 'static>(
        handle: &AppHandle,
        state: &State<T>,
        value: T,
    ) -> ProbeResult {
        let state = state.clone();
        let (send, receive) = mpsc::sync_channel(1);
        handle.post_to_ui(move || {
            state.set(value);
            let _ = send.send(());
        });
        receive.recv_timeout(Duration::from_secs(10))?;
        Ok(())
    }

    pub fn run() -> ProbeResult {
        std::fs::create_dir_all(output_directory())?;
        let scene = State::new(0_u32);
        let epoch = State::new(0_u32);
        let tick = State::new(0_u32);
        let active = State::new("one".to_owned());
        let open = State::new(true);
        let background_count = State::new(0_u32);
        let finished = Arc::new(AtomicBool::new(false));
        let worker = Arc::new(Mutex::new(None::<JoinHandle<ProbeResult>>));
        let started = Instant::now();

        let code = App::new()
            .title("UIX native interaction probe — synthetic only")
            .size(480, 648)
            .graphics_backend(GraphicsBackend::Vulkan)
            .root({
                let scene = scene.clone();
                let epoch = epoch.clone();
                let tick = tick.clone();
                let active = active.clone();
                let open = open.clone();
                let background_count = background_count.clone();
                move || {
                    let tick = tick.clone();
                    let marker = canvas(160.0, 48.0, move |bounds, ctx| {
                        let number = tick.get();
                        ctx.fill_rect(bounds, Color::WHITE, None);
                        ctx.draw_text(
                            &format!("Capture {number}"),
                            Point::new(bounds.x + 8.0, bounds.y + 16.0),
                            Color::BLACK,
                            14.0,
                        );
                    });
                    match scene.get() {
                        0 => {
                            let revision = epoch.get();
                            let refresh = epoch.clone();
                            column_fit((
                                row((
                                    button("Refresh row text")
                                        .width(320.0)
                                        .height(48.0)
                                        .on_click_fn(move || refresh.update(|value| *value += 1))
                                        .automation_id("refresh"),
                                    marker,
                                ))
                                .height(48.0),
                                VirtualScroll::new()
                                    .item_count(1000)
                                    .item_height(48.0)
                                    .size(480.0, 600.0)
                                    .render_keyed(
                                        |i| format!("item-{i}"),
                                        move |i| {
                                            column_fit((label(format!(
                                                "Row {i} / revision {revision}"
                                            ))
                                            .automation_id(format!("text-{i}")),))
                                            .height(48.0)
                                            .automation_id(format!("row-{i}"))
                                        },
                                    )
                                    .build()
                                    .automation_id("list"),
                            ))
                        }
                        1 => column((
                            marker,
                            ViewNode::new(
                                Tabs::new()
                                    .tab("One", "one")
                                    .tab("Two", "two")
                                    .active_key(&active),
                                vec![
                                    label("First panel — selected key one")
                                        .bg(Color::from_rgb(225, 240, 255)),
                                    label("Second panel — selected key two")
                                        .bg(Color::from_rgb(240, 225, 255)),
                                ],
                            )
                            .flex_grow(1.0)
                            .automation_id("tabs"),
                        )),
                        _ => {
                            let background_count = background_count.clone();
                            column((
                                marker,
                                button("Background must remain blocked")
                                    .on_click_fn(move || {
                                        background_count.update(|value| *value += 1)
                                    })
                                    .automation_id("background"),
                                Modal::builder()
                                    .open(&open)
                                    .width(420.0)
                                    .title("Confirm synthetic action")
                                    .footer_visible(true)
                                    .content(|| label("Default footer should be visible."))
                                    .build()
                                    .automation_id("dialog"),
                            ))
                        }
                    }
                }
            })
            .on_start({
                let scene = scene.clone();
                let open = open.clone();
                let tick = tick.clone();
                let active = active.clone();
                let background_count = background_count.clone();
                let finished = Arc::clone(&finished);
                let worker = Arc::clone(&worker);
                move |handle| {
                    handle
                        .run_interval(Duration::from_millis(100), || {})
                        .detach();
                    *worker.lock().unwrap() = Some(std::thread::spawn(move || {
                        let result = (|| -> ProbeResult {
                            capture(&handle, &tick, "initial")?;
                            for (name, delta_y) in [
                                ("bottom-first", 100000.0),
                                // 此公开动作复用滚轮增量；当前定高列表每单位 40 logical px。
                                ("back-up", -12.0),
                                ("bottom-again", 100000.0),
                            ] {
                                action(
                                    &handle,
                                    "list",
                                    SemanticAction::Scroll {
                                        delta: Point::new(0.0, delta_y),
                                    },
                                )?;
                                capture(&handle, &tick, name)?;
                            }
                            capture(&handle, &tick, "bottom-retained")?;
                            action(&handle, "refresh", SemanticAction::Invoke)?;
                            capture(&handle, &tick, "bottom-refreshed")?;
                            action(
                                &handle,
                                "list",
                                SemanticAction::Scroll {
                                    delta: Point::new(0.0, -600.0),
                                },
                            )?;
                            capture(&handle, &tick, "middle")?;
                            action(&handle, "refresh", SemanticAction::Invoke)?;
                            capture(&handle, &tick, "middle-refreshed")?;

                            set_on_ui(&handle, &scene, 1)?;
                            capture(&handle, &tick, "tabs-first")?;
                            action(&handle, "tabs", SemanticAction::Select("1".to_owned()))?;
                            capture(&handle, &tick, "tabs-second")?;
                            if active.get() != "two" {
                                return Err("Tabs controlled state did not select two".into());
                            }
                            action(&handle, "tabs", SemanticAction::Select("0".to_owned()))?;
                            capture(&handle, &tick, "tabs-first-again")?;
                            if active.get() != "one" {
                                return Err("Tabs controlled state did not return to one".into());
                            }

                            set_on_ui(&handle, &scene, 2)?;
                            capture(&handle, &tick, "modal-open")?;
                            let rejected = handle
                                .perform_automation_action("background", SemanticAction::Invoke)?
                                .recv_timeout(Duration::from_secs(10))?;
                            if rejected.is_ok() || background_count.get() != 0 {
                                return Err("Modal did not isolate background action".into());
                            }
                            println!(
                                "native Modal background action rejected: {}",
                                rejected.unwrap_err()
                            );
                            // 默认 footer 键鼠有独立无窗口/CPU后台用例；这里不注入原生键鼠，仅关闭 State。
                            set_on_ui(&handle, &open, false)?;
                            capture(&handle, &tick, "modal-closing")?;
                            // 默认离场为 200ms。由窗口 timer 等待动画完成，不把首张退场图当作关闭终态。
                            let (send, receive) = mpsc::sync_channel(1);
                            handle
                                .run_after(Duration::from_millis(500), move || {
                                    let _ = send.send(());
                                })
                                .detach();
                            receive.recv_timeout(Duration::from_secs(3))?;
                            capture(&handle, &tick, "modal-closed")?;
                            action(&handle, "background", SemanticAction::Invoke)?;
                            if background_count.get() != 1 {
                                return Err("background did not resume after Modal exit".into());
                            }
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
                    finished.load(Ordering::Acquire) || started.elapsed() > Duration::from_secs(80)
                }
            })
            .run();
        let worker = worker
            .lock()
            .unwrap()
            .take()
            .ok_or("native startup did not complete")?;
        worker
            .join()
            .map_err(|_| "native probe worker panicked")??;
        println!(
            "native_exit_code={code}; background_invocations={}",
            background_count.get()
        );
        if code != 0 {
            return Err("native event loop failed".into());
        }
        Ok(())
    }
}

#[cfg(all(
    feature = "test-harness",
    feature = "image-codecs",
    feature = "vulkan",
    feature = "navigation",
    feature = "feedback"
))]
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    native::run()
}

#[cfg(not(all(
    feature = "test-harness",
    feature = "image-codecs",
    feature = "vulkan",
    feature = "navigation",
    feature = "feedback"
)))]
fn main() {
    eprintln!("requires test-harness, image-codecs, vulkan, navigation and feedback");
}
