//! Embedded by the strict build planner in its immutable Cargo view.
pub(crate) const STRICT: bool = false;
pub(crate) const DYNAMIC: &[&str] = &[];
#[cfg(feature = "uix-dynamic")]
pub(crate) const LIBRARIES: &str = "[]";

pub(crate) fn require_dynamic(unit: &str) -> Result<(), String> {
    if !STRICT || DYNAMIC.contains(&unit) { Ok(()) }
    else { Err(format!("dynamic unit {unit:?} was not declared by this build")) }
}
