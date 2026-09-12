//! Element generation resolves library declarations before neutral framework primitives.
use proc_macro2::TokenStream;
use super::{Diagnostic, Element};

pub(super) fn generate_element(element: &Element) -> Result<TokenStream, Diagnostic> {
    if element.control.is_some() {
        return Err(Diagnostic::new(element.span, "control bindings require a control element", "Use If or For in a child slot"));
    }
    if let Some(declaration) = crate::lang::compiler::components::declaration(&element.name) {
        if declaration.record {
            return Err(Diagnostic::new(element.span, format!("<{}> is a record slot, not a View", element.name), "Place it inside a component that declares this record slot"));
        }
        return super::component_codegen::generate(element, &declaration);
    }
    match element.name.as_str() {
        "KernelView" => super::generate_kernel_view(element),
        "KernelHost" => super::generate_kernel_host(element),
        "Canvas" => super::generate_canvas(element),
        "FocusTrap" => super::generate_focus_trap(element),
        _ => Err(Diagnostic::new(element.span, format!("undeclared component <{}>", element.name),
            "Load the component library descriptor in the project or build configuration")),
    }
}
