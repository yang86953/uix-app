// Auto-organized test modules. Tests live only under src/tests.

mod backends;
mod factory;
#[cfg(feature = "vulkan")]
mod gfx_r5;
mod graphics;
mod services;
mod shared;
mod test_harness;
mod traits;
