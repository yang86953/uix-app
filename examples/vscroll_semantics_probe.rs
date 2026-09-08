//! Manual native consumer-reproduction probe, not a project test.
//! cargo run --locked --offline --features test-harness --example vscroll_semantics_probe
//! Set UIX_AUTOMATION_DIR to a private directory for this synthetic process only.
//! Existing read-only export + public AppHandle actions; no foreground Agent endpoint.
//! After the saved phases are complete, close this synthetic window normally if it remains open.

#![allow(non_snake_case, unused_braces)]

#[cfg(all(feature = "test-harness", feature = "image-codecs", feature = "vulkan"))]
mod native {
    use std::path::PathBuf;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    };
    use std::thread::JoinHandle;
    use std::time::{Duration, Instant};
    use uix::prelude::*;
    use uix::ui::test_harness::AUTOMATION_DIR_ENV;

    uix_items!("examples/fixtures/vscroll_report.uix");

    type ProbeResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

    fn output_directory() -> PathBuf {
        std::env::var_os("UIX_VSCROLL_PROBE_OUTPUT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("target/interaction-codex/native-uix-lang"))
    }

    fn rows(revision: u32, count: usize) -> Vec<ProbeRow> {
        let statuses = ["待处理", "进行中", "受阻", "已完成"];
        let priorities = ["低", "中", "高", "紧急"];
        (0..count)
            .map(|i| ProbeRow {
                id: format!("TASK-{i:04}"),
                title: format!("任务 {i:04} · 接入验证 / revision {revision}"),
                statusLabel: statuses[i % 4].to_owned(),
                priority: priorities[i % 4].to_owned(),
                due: format!("2026-{:02}-{:02}", 1 + i % 9, 1 + i % 28),
                automationId: format!("task-TASK-{i:04}"),
            })
            .collect()
    }

    fn capture_semantics(handle: &AppHandle, phase: &str) -> ProbeResult {
        let directory = std::env::var_os(AUTOMATION_DIR_ENV)
            .ok_or("set UIX_AUTOMATION_DIR to an explicit private directory")?;
        // Cross a UI queue boundary so the previous render can publish its frame-tail export.
        let (send, receive) = mpsc::sync_channel(1);
        handle.post_to_ui(move || {
            let _ = send.send(());
        });
        receive.recv_timeout(Duration::from_secs(3))?;
        let source = PathBuf::from(directory).join(format!(
            "uix-{}-window-{}.json",
            std::process::id(),
            handle.window_id().raw()
        ));
        std::fs::write(
            output_directory().join(format!("{phase}.snapshot.json")),
            std::fs::read(source)?,
        )?;
        println!("saved semantic export: {phase}");
        Ok(())
    }

    fn capture(handle: &AppHandle, tick: &State<u32>, phase: &str) -> ProbeResult {
        let (send, receive) = mpsc::sync_channel(1);
        let capture_handle = handle.clone();
        let tick = tick.clone();
        handle.post_to_ui(move || {
            // Paint-only dependency outside the original 844px consumer root.
            // Never rebuild/select the tail just to obtain pixels or repair its bounds.
            tick.update(|value| *value += 1);
            let _ = send.send(capture_handle.request_surface_readback_for_test());
        });
        let frame = receive
            .recv_timeout(Duration::from_secs(10))??
            .recv_timeout(Duration::from_secs(10))?;
        let mut rgba = Vec::with_capacity(frame.pixels.len() * 4);
        for pixel in frame.pixels {
            rgba.extend_from_slice(&[
                (pixel >> 16) as u8,
                (pixel >> 8) as u8,
                pixel as u8,
                (pixel >> 24) as u8,
            ]);
        }
        image::save_buffer(
            output_directory().join(format!("{phase}.png")),
            &rgba,
            frame.width as u32,
            frame.height as u32,
            image::ColorType::Rgba8,
        )?;
        capture_semantics(handle, phase)?;
        println!("captured {phase}: {}x{}", frame.width, frame.height);
        Ok(())
    }

    fn action(handle: &AppHandle, target: &str, action: SemanticAction) -> ProbeResult {
        handle
            .perform_automation_action(target, action)?
            .recv_timeout(Duration::from_secs(10))??;
        println!("public action completed: {target}");
        Ok(())
    }

    fn scroll(handle: &AppHandle, pixels: f32) -> ProbeResult {
        action(
            handle,
            "probe-list",
            SemanticAction::Scroll {
                delta: Point::new(0.0, pixels / 40.0),
            },
        )
    }

    fn update_rows(handle: &AppHandle, data: &State<Vec<ProbeRow>>, revision: u32) -> ProbeResult {
        let data = data.clone();
        let (send, receive) = mpsc::sync_channel(1);
        handle.post_to_ui(move || {
            // Models an external projection refresh without touching selectedTaskId.
            data.set(rows(revision, 1000));
            let _ = send.send(());
        });
        receive.recv_timeout(Duration::from_secs(3))?;
        Ok(())
    }

    pub fn run() -> ProbeResult {
        if std::env::var_os(AUTOMATION_DIR_ENV).is_none() {
            return Err("set UIX_AUTOMATION_DIR to an explicit private directory".into());
        }
        std::fs::create_dir_all(output_directory())?;
        let probeRows = State::new(rows(0, 1000));
        let selectedTaskId = State::new(String::new());
        let probeStatus = State::new("千行合成数据就绪".to_owned());
        let probeShrinkTrigger = State::new(false);
        let probeRestoreTrigger = State::new(false);
        {
            let data = probeRows.clone();
            let status = probeStatus.clone();
            probeShrinkTrigger.watch(move |_| {
                data.set(rows(2, 600));
                status.set("已截断为 600 行".to_owned());
            });
        }
        {
            let data = probeRows.clone();
            let status = probeStatus.clone();
            probeRestoreTrigger.watch(move |_| {
                data.set(rows(2, 1000));
                status.set("已恢复千行".to_owned());
            });
        }
        let tick = State::new(0_u32);
        let finished = Arc::new(AtomicBool::new(false));
        let worker = Arc::new(Mutex::new(None::<JoinHandle<ProbeResult>>));
        let started = Instant::now();
        let code = App::new()
            .title("UIX Lang VirtualScroll report probe — synthetic only")
            .size(1440, 900)
            .custom_title_bar(true)
            .theme(Theme::antd_light())
            .graphics_backend(GraphicsBackend::Vulkan)
            .root({
                let probeRows = probeRows.clone();
                let selectedTaskId = selectedTaskId.clone();
                let tick = tick.clone();
                move || {
                    let marker_tick = tick.clone();
                    column_fit((
                        uix!("examples/fixtures/vscroll_report.uix"),
                        canvas(1440.0, 16.0, move |bounds, ctx| {
                            let value = marker_tick.get();
                            ctx.fill_rect(bounds, Color::WHITE, None);
                            ctx.draw_text(
                                &format!("Capture {value} — synthetic UIX Lang fixture"),
                                Point::new(bounds.x + 8.0, bounds.y + 1.0),
                                Color::BLACK,
                                12.0,
                            );
                        }),
                    ))
                }
            })
            .on_start({
                let finished = Arc::clone(&finished);
                let worker = Arc::clone(&worker);
                move |handle| {
                    handle
                        .run_interval(Duration::from_millis(100), || {})
                        .detach();
                    *worker.lock().unwrap() = Some(std::thread::spawn(move || {
                        let result = (|| -> ProbeResult {
                            capture(&handle, &tick, "initial")?;
                            // Original report: one overshoot, observe before any tail activation.
                            scroll(&handle, 1_048_576.0)?;
                            capture_semantics(&handle, "tail-before-readback")?;
                            capture(&handle, &tick, "tail-first")?;
                            capture_semantics(&handle, "tail-repeat")?;
                            scroll(&handle, -480.0)?;
                            capture(&handle, &tick, "back-up")?;
                            scroll(&handle, 1_048_576.0)?;
                            capture(&handle, &tick, "tail-again")?;
                            capture(&handle, &tick, "tail-retained")?;
                            if !selectedTaskId.get().is_empty() {
                                return Err("tail was selected before observation".into());
                            }
                            // Original consumer sequence: select first, then middle, then clamp to tail.
                            scroll(&handle, -1_048_576.0)?;
                            action(&handle, "task-TASK-0000", SemanticAction::Invoke)?;
                            if selectedTaskId.get() != "TASK-0000" {
                                return Err("first selection failed".into());
                            }
                            for _ in 0..12 {
                                scroll(&handle, 1920.0)?;
                            }
                            capture(&handle, &tick, "middle-before-selection")?;
                            action(&handle, "task-TASK-0480", SemanticAction::Invoke)?;
                            capture(&handle, &tick, "middle-selected")?;
                            if selectedTaskId.get() != "TASK-0480" {
                                return Err("middle selection failed".into());
                            }
                            scroll(&handle, 1_048_576.0)?;
                            capture(&handle, &tick, "tail-after-middle-selection")?;
                            if selectedTaskId.get() != "TASK-0480" {
                                return Err("scroll changed selection".into());
                            }
                            update_rows(&handle, &probeRows, 1)?;
                            capture(&handle, &tick, "tail-refreshed")?;
                            scroll(&handle, -24000.0)?;
                            update_rows(&handle, &probeRows, 2)?;
                            capture(&handle, &tick, "middle-refreshed")?;
                            scroll(&handle, 1_048_576.0)?;
                            // Only now invoke the tail, after all pre-invoke evidence has been saved.
                            action(&handle, "task-TASK-0999", SemanticAction::Invoke)?;
                            capture(&handle, &tick, "tail-selected")?;
                            if selectedTaskId.get() != "TASK-0999" {
                                return Err("tail invocation failed".into());
                            }
                            action(&handle, "probe-shrink", SemanticAction::Invoke)?;
                            capture(&handle, &tick, "tail-shrunk")?;
                            if probeRows.get().len() != 600 {
                                return Err("shrink failed".into());
                            }
                            action(&handle, "task-TASK-0590", SemanticAction::Invoke)?;
                            action(&handle, "probe-restore", SemanticAction::Invoke)?;
                            scroll(&handle, -1_048_576.0)?;
                            capture(&handle, &tick, "restored-top")?;
                            if selectedTaskId.get() != "TASK-0590" || probeRows.get().len() != 1000
                            {
                                return Err(
                                    "restore took over controlled selection or lost rows".into()
                                );
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
                // Sampled on native UiEvent, not a hard timer. An idle window may need normal closure.
                let finished = Arc::clone(&finished);
                move |_| {
                    finished.load(Ordering::Acquire) || started.elapsed() > Duration::from_secs(90)
                }
            })
            .run();
        worker
            .lock()
            .unwrap()
            .take()
            .ok_or("native startup did not complete")?
            .join()
            .map_err(|_| "native probe worker panicked")??;
        if code != 0 {
            return Err("native event loop failed".into());
        }
        println!("native_exit_code={code}; completed original row-template sequence");
        Ok(())
    }
}

#[cfg(all(feature = "test-harness", feature = "image-codecs", feature = "vulkan"))]
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    native::run()
}

#[cfg(not(all(feature = "test-harness", feature = "image-codecs", feature = "vulkan")))]
fn main() {
    eprintln!("requires test-harness, image-codecs and vulkan");
}
