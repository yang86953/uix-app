// Auto-organized test modules. Tests live only under src/tests.

mod cpu;
mod factory;
mod gpu;
mod module;
#[cfg(feature = "opengles")]
mod opengl_native_tests;
mod rounded_additive;
mod test_backend;
