//! Standalone State / Computed / Effect and scoped dependency tracking.
//! Only core values are required. UI refresh destinations implement the
//! `state::InvalidationTarget` notification port; no renderer is imported here.
pub mod state;
