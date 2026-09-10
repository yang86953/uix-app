//! 手动测量公开 Agent API 的连续事务，不是项目性能测试或真实桌面验收。
//! cargo run --locked --features agent-control --example agent_control_probe -- 1800 target/probe.json
#[cfg(feature = "agent-control")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use serde_json::{Value, json};
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};
    use uix_app::app::agent_workspace::AgentWorkspace;
    use uix_app::prelude::*;

    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.len() != 2 {
        return Err("expected: seconds(1..1800) output-under-target.json".into());
    }
    let seconds: u64 = arguments[0].parse()?;
    if !(1..=1800).contains(&seconds) {
        return Err("seconds must be 1..1800".into());
    }
    let output = PathBuf::from(&arguments[1]);
    let target = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .canonicalize()?;
    if !output
        .parent()
        .ok_or("output parent missing")?
        .canonicalize()?
        .starts_with(&target)
    {
        return Err("output must remain in the source build directory".into());
    }
    let mut report_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    let draft = State::new(String::new());
    let commits = State::new(0_u64);
    let saved = State::new(String::new());
    let (view_draft, view_commits, view_saved) = (draft.clone(), commits.clone(), saved.clone());
    let workspace = AgentWorkspace::new(400, 200, move || {
        let input_value = view_draft.clone();
        let saved = view_saved.clone();
        column_fit((
            input()
                .value(&view_draft)
                .build()
                .automation_id("probe-editor"),
            button("Commit probe")
                .on_click(&view_commits, move |count| {
                    saved.set(input_value.get());
                    count.set(count.get() + 1);
                })
                .build()
                .automation_id("probe-commit"),
            label(format!("Count {}", view_commits.get())).automation_id("probe-count"),
        ))
    })
    .title("UIX owned AI control probe")
    .spawn()?;

    let mut samples = Vec::<f64>::new();
    let started = Instant::now();
    let mut last_progress = started;
    let mut observed_calls = 0_u64;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let mut client = workspace.client()?;
        let windows = client.list_windows()?;
        if windows.len() != 1 {
            return Err("probe requires its single owned window".into());
        }
        let view = windows[0];
        let mut last_revision = 0;
        while started.elapsed() < Duration::from_secs(seconds) {
            let cycle = Instant::now();
            let number = samples.len() as u64 + 1;
            let text = format!("probe-{number:08}-private-draft");
            client.perform_set_value(view.window_id, view.generation, "probe-editor", &text)?;
            observed_calls += 1;
            client.perform_raw(
                view.window_id,
                view.generation,
                "probe-editor",
                json!({"kind":"focus"}),
            )?;
            observed_calls += 1;
            for key in ["a", "c"] {
                let response = client.request(json!({"type":"perform","window_id":view.window_id,
                    "generation":view.generation,"action":{"kind":"press_key","key":key,"modifiers":["ctrl"]}}))?;
                if response["ok"] != true {
                    return Err("private copy key was not accepted".into());
                }
                observed_calls += 1;
            }
            client.perform_set_value(view.window_id, view.generation, "probe-editor", "")?;
            observed_calls += 1;
            let paste = client.request(json!({"type":"perform","window_id":view.window_id,
                "generation":view.generation,"action":{"kind":"press_key","key":"v","modifiers":["ctrl"]}}))?;
            if paste["ok"] != true {
                return Err("private paste was not accepted".into());
            }
            observed_calls += 1;
            client.perform_invoke(view.window_id, view.generation, "probe-commit")?;
            observed_calls += 1;
            let snapshot = client.snapshot(view.window_id)?;
            observed_calls += 1;
            let revision = snapshot["revision"].as_u64().ok_or("revision missing")?;
            if snapshot["generation"] != view.generation
                || revision <= last_revision
                || draft.get() != text
                || saved.get() != text
                || commits.get() != number
            {
                return Err(
                    "generation, revision, clipboard content or exactly-once commit mismatch"
                        .into(),
                );
            }
            let expected = format!("Count {number}");
            let count_visible = snapshot["nodes"].as_array().is_some_and(|nodes| {
                nodes
                    .iter()
                    .any(|node| node["automation_id"] == "probe-count" && node["name"] == expected)
            });
            if !count_visible {
                return Err("semantic readback does not reflect commit".into());
            }
            last_revision = revision;
            samples.push(cycle.elapsed().as_secs_f64() * 1000.0);
            if last_progress.elapsed() >= Duration::from_secs(60) {
                println!(
                    "{}",
                    json!({"elapsed_seconds":started.elapsed().as_secs(),"completed_cycles":samples.len(),"verified_calls":observed_calls})
                );
                last_progress = Instant::now();
            }
            // Controlled 10 Hz load; excluded from cycle latency. This is not a fixed UI readiness sleep.
            if let Some(idle) = Duration::from_millis(100).checked_sub(cycle.elapsed()) {
                std::thread::sleep(
                    idle.min(Duration::from_secs(seconds).saturating_sub(started.elapsed())),
                );
            }
        }
        Ok(())
    })();
    let elapsed_seconds = started.elapsed().as_secs_f64();
    let close = workspace.close();
    let mut ordered = samples.clone();
    ordered.sort_by(f64::total_cmp);
    let percentile = |p: f64| -> Value {
        if ordered.is_empty() {
            Value::Null
        } else {
            json!(ordered[((ordered.len() as f64 * p).ceil() as usize).saturating_sub(1)])
        }
    };
    let report = json!({"scope":"owned offscreen UIX workspace, not Portal/EIS or human baseline",
        "requested_seconds":seconds,"elapsed_seconds":elapsed_seconds,"completed_cycles":samples.len(),
        "verified_calls":observed_calls,"p50_ms":percentile(0.5),"p95_ms":percentile(0.95),
        "cycle_samples_ms":samples,"private_clipboard_and_commit_verified":result.is_ok(),
        "completed":result.is_ok() && close.is_ok(),"close_confirmed":close.is_ok(),
        "failure":result.as_ref().err().map(ToString::to_string),
        "close_failure":close.as_ref().err().map(ToString::to_string),
        "desktop_input_sent":false,"human_baseline_measured":false});
    serde_json::to_writer_pretty(&mut report_file, &report)?;
    report_file.write_all(b"\n")?;
    report_file.sync_all()?;
    println!(
        "{}",
        json!({"completed":report["completed"],"elapsed_seconds":elapsed_seconds,
        "completed_cycles":report["completed_cycles"],"p50_ms":report["p50_ms"],"p95_ms":report["p95_ms"],
        "close_confirmed":report["close_confirmed"]})
    );
    result?;
    close?;
    Ok(())
}

#[cfg(not(feature = "agent-control"))]
fn main() {
    eprintln!("this example requires --features agent-control");
}
