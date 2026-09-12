use uix_app::prelude::*;
struct Counter(State<u32>);
impl View for Counter {
    fn build(self) -> ViewNode {
        let label_state = self.0.clone();
        dynamic_label(move || label_state.get().to_string())
            .on_click(&self.0, |state| state.update(|count| *count += 1))
            .automation_id("counter")
    }
}
fn main() {
    let state = State::new(0);
    let captured = state.clone();
    let mut app = uix_app::ui::test_harness::TestApp::new((320.0, 200.0), move || {
        Counter(captured.clone()).build()
    });
    app.invoke("counter").unwrap();
    assert_eq!(state.get(), 1);
    println!("custom framework-only View laid out and handled an event");
}
