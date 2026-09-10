//! Manual public-API probe: synthetic scrolling and Modal frames in an isolated AgentWorkspace.
//! This renders real CPU offscreen frames, not native-window or GPU acceptance.
//! Run with --features agent-control; output stays under target/interaction-codex.

#[cfg(all(feature = "agent-control", feature = "feedback"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use serde_json::{Value, json};
    use std::path::Path;
    use uix_app::app::agent_client::{AgentBridgeClient, AgentWindowEntry};
    use uix_app::app::agent_workspace::AgentWorkspace;
    use uix_app::prelude::*;

    fn save(name: &str, value: &Value) -> Result<(), Box<dyn std::error::Error>> {
        std::fs::write(
            Path::new("target/interaction-codex").join(format!("{name}.json")),
            serde_json::to_vec_pretty(value)?,
        )?;
        Ok(())
    }
    fn action(
        client: &mut AgentBridgeClient,
        view: AgentWindowEntry,
        target: Option<&str>,
        action: Value,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut request = json!({"type":"perform", "window_id":view.window_id,
            "generation":view.generation, "action":action});
        if let Some(id) = target {
            request["target"] = json!({"automation_id":id});
        }
        let reply = client.request(request)?;
        if reply["ok"] != true {
            return Err(format!("action rejected: {reply}").into());
        }
        Ok(())
    }
    fn screenshot(
        client: &mut AgentBridgeClient,
        view: AgentWindowEntry,
        name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let reply = client.request(json!({"type":"screenshot", "window_id":view.window_id,
            "generation":view.generation}))?;
        if reply["ok"] != true {
            return Err(format!("screenshot rejected: {reply}").into());
        }
        save(name, &reply)
    }

    std::fs::create_dir_all("target/interaction-codex")?;
    let epoch = State::new(0_u32);
    let root_epoch = epoch.clone();
    let scroll = AgentWorkspace::new(480, 648, move || {
        let revision = root_epoch.get();
        let refresh = root_epoch.clone();
        column_fit((
            button("Refresh row text")
                .height(48.0)
                .on_click_fn(move || refresh.set(refresh.get() + 1))
                .automation_id("refresh"),
            VirtualScroll::new()
                .item_count(1000)
                .item_height(48.0)
                .size(480.0, 600.0)
                .render_keyed(
                    |i| format!("item-{i}"),
                    move |i| {
                        column_fit((label(format!("Row {i} / revision {revision}"))
                            .automation_id(format!("text-{i}")),))
                        .height(48.0)
                        .automation_id(format!("row-{i}"))
                    },
                )
                .build()
                .automation_id("list"),
        ))
    })
    .title("UIX scrolling acceptance probe")
    .spawn()?;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let mut client = scroll.client()?;
        let view = client.list_windows()?[0];
        for (name, delta) in [
            ("bottom-first", 100000.0),
            ("back-up", -480.0),
            ("bottom-again", 100000.0),
        ] {
            action(
                &mut client,
                view,
                Some("list"),
                json!({"kind":"scroll", "delta_x":0.0, "delta_y":delta}),
            )?;
            save(name, &client.snapshot(view.window_id)?)?;
        }
        screenshot(&mut client, view, "bottom-pixels")?;
        save("bottom-after-pixels", &client.snapshot(view.window_id)?)?;
        action(&mut client, view, Some("refresh"), json!({"kind":"invoke"}))?;
        save("bottom-refreshed", &client.snapshot(view.window_id)?)?;
        screenshot(&mut client, view, "refreshed-pixels")?;
        Ok(())
    })();
    scroll.close()?;
    result?;

    for (case, command) in [
        (
            "enter",
            json!({"kind":"press_key", "key":"enter", "modifiers":[]}),
        ),
        (
            "escape",
            json!({"kind":"press_key", "key":"escape", "modifiers":[]}),
        ),
        // Logical coordinates read from this probe's 640x480, DPR=1 screenshot.
        ("ok-click", json!({"kind":"click_at", "x":524.0, "y":362.0})),
        (
            "cancel-click",
            json!({"kind":"click_at", "x":436.0, "y":362.0}),
        ),
    ] {
        let open = State::new(true);
        let confirmed = State::new(0);
        let cancelled = State::new(0);
        let root_open = open.clone();
        let root_confirmed = confirmed.clone();
        let root_cancelled = cancelled.clone();
        let modal = AgentWorkspace::new(640, 480, move || {
            let confirmed = root_confirmed.clone();
            let cancelled = root_cancelled.clone();
            Modal::builder()
                .open(&root_open)
                .title("Confirm synthetic action")
                .content(|| label("This probe contains no user data.").automation_id("body"))
                .footer_visible(true)
                .on_ok(move || confirmed.set(confirmed.get() + 1))
                .on_cancel(move || cancelled.set(cancelled.get() + 1))
                .build()
                .automation_id("dialog")
        })
        .title("UIX Modal acceptance probe")
        .spawn()?;
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let mut client = modal.client()?;
            let view = client.list_windows()?[0];
            save(
                &format!("modal-{case}-open"),
                &client.snapshot(view.window_id)?,
            )?;
            screenshot(&mut client, view, &format!("modal-{case}-pixels"))?;
            action(&mut client, view, None, command)?;
            save(
                &format!("modal-{case}-after"),
                &client.snapshot(view.window_id)?,
            )?;
            save(
                &format!("modal-{case}-result"),
                &json!({"open":open.get(), "confirmed":confirmed.get(), "cancelled":cancelled.get()}),
            )?;
            Ok(())
        })();
        modal.close()?;
        result?;
    }
    println!("Synthetic offscreen probe completed; not native-window/GPU acceptance.");
    Ok(())
}

#[cfg(not(all(feature = "agent-control", feature = "feedback")))]
fn main() {
    eprintln!("interaction_probe requires --features agent-control,feedback");
}
