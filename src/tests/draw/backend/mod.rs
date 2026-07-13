// Auto-organized test modules. Tests live only under src/tests.

mod cpu;
mod module;
mod native_gpu;
mod null;
mod offscreen_pool;
#[cfg(feature = "opengles")]
mod opengl_native_tests;
mod registry;
