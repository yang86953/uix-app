// Auto-organized test modules. Tests live only under src/tests.

mod active_work_registry;
mod agent_bridge;
mod agent_control;
#[cfg(feature = "agent-control")]
mod agent_protocol;
mod app_handle;
mod app_timer;
mod event_loop;
mod frame_scheduler;
mod main_thread_queue;
mod session_runtime;
mod shell;
mod test_clock;
mod window;
mod window_semantics;
mod window_session;
