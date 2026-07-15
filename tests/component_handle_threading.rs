use uix::prelude::{AppState, ComponentHandle};

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn public_app_state_and_component_handle_are_send_sync() {
    assert_send_sync::<AppState>();
    assert_send_sync::<ComponentHandle>();
}
