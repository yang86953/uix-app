use super::PackagePlan;
use quote::ToTokens;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use syn::visit_mut::VisitMut;

/// Discover references from the actual UIX parser, including imported sources.
pub(super) fn uix_tags(entry: &Path) -> Result<BTreeSet<String>, String> {
    use crate::lang::compiler::uix_lang::{Declaration, Node, parse_items_document};
    let entry = entry.canonicalize().map_err(|e| e.to_string())?;
    let mut definitions = std::collections::BTreeMap::new();
    let mut roots = Vec::new();
    let mut seen = BTreeSet::new();
    let mut pending = vec![entry.clone()];
    while let Some(path) = pending.pop() {
        let path = path.canonicalize().map_err(|e| format!("{}: {e}", path.display()))?;
        if !seen.insert(path.clone()) { continue; }
        let source = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let document = parse_items_document(&source).map_err(|e| format!("{}:{}:{}: {}", path.display(), e.span.line, e.span.column, e.message))?;
        if path == entry { roots.push(Node::Element(document.root)); }
        for declaration in document.declarations {
            match declaration {
                Declaration::Import(import) => pending.push(path.parent().ok_or("source has no parent")?.join(import.path)),
                Declaration::Widget(widget) => { definitions.insert(widget.name, widget.children); }
                _ => {}
            }
        }
    }
    let mut tags = BTreeSet::new();
    let mut expanded = BTreeSet::new();
    while let Some(node) = roots.pop() {
        if let Node::Element(element) = node {
            roots.extend(element.children);
            if let Some(body) = definitions.get(&element.name) {
                if expanded.insert(element.name) { roots.extend(body.iter().cloned()); }
            } else { tags.insert(element.name); }
        }
    }
    Ok(tags)
}

pub(super) fn includes(package: &PackagePlan, path: &Path) -> bool {
    let units = package.selected.iter().flat_map(|name| &package.descriptor.units[name].modules);
    let entries: Vec<_> = units.chain(&package.descriptor.shared).collect();
    if entries.is_empty() { return true; }
    if path == Path::new("src/lib.rs") { return true; }
    entries.iter().any(|entry| {
        if path == entry.as_path() || path.starts_with(entry) { return true; }
        let parent = path.parent().unwrap_or(Path::new(""));
        matches!(path.file_name().and_then(|v| v.to_str()), Some("mod.rs" | "lib.rs")) && entry.starts_with(parent)
    })
}

pub(super) fn rewrite(package: &PackagePlan, path: &Path, source: &str) -> Result<String, String> {
    let mut file = syn::parse_file(source).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut selector = Selector { package, path: path.to_path_buf(), enum_scope: None, disabled_modules: BTreeSet::new() };
    for item in &file.items {
        if let syn::Item::Mod(module) = item {
            if module.content.is_none() && !includes(package, &selector.module_path(module)) {
                selector.disabled_modules.insert(module.ident.to_string());
            }
        }
    }
    file.items = file.items.into_iter().filter_map(|mut item| {
        if !selector.keep_item(&mut item) { return None; }
        selector.visit_item_mut(&mut item);
        Some(item)
    }).collect();
    Ok(format!("// UIX selected compilation view; edit the source repository.\n{}\n", file.into_token_stream()))
}

struct Selector<'a> {
    package: &'a PackagePlan,
    path: PathBuf,
    enum_scope: Option<String>,
    disabled_modules: BTreeSet<String>,
}
impl Selector<'_> {
    fn available(&self, name: &str) -> bool {
        self.package.descriptor.symbols.get(name).is_none_or(|unit| self.package.selected.contains(unit))
    }
    fn enum_variant(&self, name: &str, variant: &str) -> bool {
        self.package.descriptor.enums.get(name).and_then(|m| m.get(variant))
            .is_none_or(|unit| self.package.selected.contains(unit))
    }
    fn attrs(&self, attrs: &mut Vec<syn::Attribute>) -> bool {
        let mut keep = true;
        attrs.retain(|attr| {
            if attr.path().is_ident("cfg") {
                if let Ok(meta) = attr.parse_args::<syn::Meta>() {
                    match self.cfg(&meta) {
                        Some(false) => { keep = false; return false; }
                        Some(true) => return false,
                        None => {}
                    }
                }
            }
            true
        });
        keep
    }
    fn cfg(&self, meta: &syn::Meta) -> Option<bool> {
        use syn::parse::Parser;
        match meta {
            syn::Meta::NameValue(value) if value.path.is_ident("feature") => {
                let syn::Expr::Lit(value) = &value.value else { return None; };
                let syn::Lit::Str(value) = &value.lit else { return None; };
                self.package.descriptor.feature_units.get(&value.value())
                    .map(|units| units.iter().any(|unit| self.package.selected.contains(unit)))
            }
            syn::Meta::List(list) => {
                let args = syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated.parse2(list.tokens.clone()).ok()?;
                let values: Vec<_> = args.iter().map(|arg| self.cfg(arg)).collect();
                if list.path.is_ident("all") {
                    if values.contains(&Some(false)) { Some(false) }
                    else if values.iter().all(|v| *v == Some(true)) { Some(true) } else { None }
                } else if list.path.is_ident("any") {
                    if values.contains(&Some(true)) { Some(true) }
                    else if values.iter().all(|v| *v == Some(false)) { Some(false) } else { None }
                } else if list.path.is_ident("not") && values.len() == 1 { values[0].map(|v| !v) }
                else { None }
            }
            _ => None,
        }
    }
    fn keep_item(&self, item: &mut syn::Item) -> bool {
        match item {
            syn::Item::Const(v) => self.attrs(&mut v.attrs) && self.available(&v.ident.to_string()),
            syn::Item::Enum(v) => self.attrs(&mut v.attrs) && self.available(&v.ident.to_string()),
            syn::Item::Fn(v) => self.attrs(&mut v.attrs) && self.available(&v.sig.ident.to_string())
                && v.sig.inputs.iter().all(|arg| match arg {
                    syn::FnArg::Typed(arg) => self.type_available(&arg.ty),
                    _ => true,
                }) && match &v.sig.output {
                    syn::ReturnType::Type(_, ty) => self.type_available(ty),
                    _ => true,
                },
            syn::Item::Struct(v) => self.attrs(&mut v.attrs) && self.available(&v.ident.to_string()),
            syn::Item::Trait(v) => self.attrs(&mut v.attrs) && self.available(&v.ident.to_string()),
            syn::Item::Type(v) => self.attrs(&mut v.attrs) && self.available(&v.ident.to_string()),
            syn::Item::Static(v) => self.attrs(&mut v.attrs) && self.available(&v.ident.to_string()),
            syn::Item::Impl(v) => {
                self.attrs(&mut v.attrs) && self.type_available(&v.self_ty)
                    && v.trait_.as_ref().is_none_or(|(path, _)| self.type_available(&syn::Type::Path(syn::TypePath { attrs: Vec::new(), qself: None, path: path.clone() })))
            }
            syn::Item::Use(v) => self.attrs(&mut v.attrs) && self.use_tree(&mut v.tree),
            syn::Item::Mod(v) => {
                if !self.attrs(&mut v.attrs) { return false; }
                if v.content.is_some() { return true; }
                let module = self.module_path(v);
                includes(self.package, &module)
            }
            syn::Item::Macro(v) => self.attrs(&mut v.attrs),
            _ => true,
        }
    }
    fn type_available(&self, ty: &syn::Type) -> bool {
        match ty {
            syn::Type::Path(path) => path.path.segments.iter().all(|segment| {
                self.available(&segment.ident.to_string()) && match &segment.arguments {
                    syn::PathArguments::AngleBracketed(args) => args.args.iter().all(|arg| {
                        if let syn::GenericArgument::Type(ty) = arg { self.type_available(ty) } else { true }
                    }),
                    _ => true,
                }
            }),
            syn::Type::Reference(v) => self.type_available(&v.elem),
            syn::Type::Slice(v) => self.type_available(&v.elem),
            syn::Type::Array(v) => self.type_available(&v.elem),
            syn::Type::Tuple(v) => v.elems.iter().all(|ty| self.type_available(ty)),
            _ => true,
        }
    }
    fn use_tree(&self, tree: &mut syn::UseTree) -> bool {
        match tree {
            syn::UseTree::Name(v) => self.available(&v.ident.to_string()),
            syn::UseTree::Rename(v) => self.available(&v.ident.to_string()) && self.available(&v.rename.to_string()),
            syn::UseTree::Path(v) => !self.disabled_modules.contains(&v.ident.to_string()) && self.available(&format!("module:{}", v.ident)) && self.use_tree(&mut v.tree),
            syn::UseTree::Group(v) => {
                v.items = std::mem::take(&mut v.items).into_iter().filter_map(|mut item| {
                    if self.use_tree(&mut item) { Some(item) } else { None }
                }).collect();
                !v.items.is_empty()
            }
            syn::UseTree::Glob(_) => true,
        }
    }
    fn module_path(&self, module: &syn::ItemMod) -> PathBuf {
        let base = self.path.parent().unwrap_or(Path::new(""));
        for attr in &module.attrs {
            if attr.path().is_ident("path") {
                if let syn::Meta::NameValue(value) = &attr.meta {
                    if let syn::Expr::Lit(value) = &value.value {
                        if let syn::Lit::Str(value) = &value.lit { return normalize(&base.join(value.value())); }
                    }
                }
            }
        }
        let base = if matches!(self.path.file_name().and_then(|v| v.to_str()), Some("mod.rs" | "lib.rs")) {
            base.to_path_buf()
        } else { base.join(self.path.file_stem().unwrap_or_default()) };
        let file = base.join(format!("{}.rs", module.ident));
        if self.package.source.join(&file).exists() { file }
        else { base.join(module.ident.to_string()).join("mod.rs") }
    }
    fn pattern(&self, pattern: &mut syn::Pat) -> bool {
        match pattern {
            syn::Pat::Guard(v) => self.pattern(&mut v.pat),
            syn::Pat::Or(v) => {
                v.cases = std::mem::take(&mut v.cases).into_iter().filter_map(|mut p| if self.pattern(&mut p) { Some(p) } else { None }).collect();
                !v.cases.is_empty()
            }
            syn::Pat::Tuple(v) => v.elems.iter_mut().all(|p| self.pattern(p)),
            syn::Pat::Paren(v) => self.pattern(&mut v.pat),
            syn::Pat::Reference(v) => self.pattern(&mut v.pat),
            syn::Pat::Struct(v) => self.pattern_path(&v.path),
            syn::Pat::TupleStruct(v) => self.pattern_path(&v.path),
            syn::Pat::Path(v) => self.pattern_path(&v.path),
            _ => true,
        }
    }
    fn pattern_path(&self, path: &syn::Path) -> bool {
        if path.segments.len() < 2 { return true; }
        let mut segments = path.segments.iter().rev();
        let Some(variant) = segments.next() else { return true; };
        let Some(owner) = segments.next() else { return true; };
        let owner = if owner.ident == "Self" { self.enum_scope.as_deref().unwrap_or("Self").to_owned() } else { owner.ident.to_string() };
        self.enum_variant(&owner, &variant.ident.to_string())
    }
}
impl VisitMut for Selector<'_> {
    fn visit_item_enum_mut(&mut self, item: &mut syn::ItemEnum) {
        item.variants = std::mem::take(&mut item.variants).into_iter().filter_map(|mut variant| {
            if self.attrs(&mut variant.attrs) && self.enum_variant(&item.ident.to_string(), &variant.ident.to_string()) { Some(variant) } else { None }
        }).collect();
        syn::visit_mut::visit_item_enum_mut(self, item);
    }
    fn visit_item_impl_mut(&mut self, item: &mut syn::ItemImpl) {
        let old = self.enum_scope.take();
        self.enum_scope = if let syn::Type::Path(path) = item.self_ty.as_ref() { path.path.segments.last().map(|s| s.ident.to_string()) } else { None };
        syn::visit_mut::visit_item_impl_mut(self, item);
        self.enum_scope = old;
    }
    fn visit_expr_match_mut(&mut self, expression: &mut syn::ExprMatch) {
        expression.arms = std::mem::take(&mut expression.arms).into_iter().filter_map(|mut arm| {
            if self.attrs(&mut arm.attrs) && self.pattern(&mut arm.pat) { Some(arm) } else { None }
        }).collect();
        syn::visit_mut::visit_expr_match_mut(self, expression);
    }
    fn visit_item_mod_mut(&mut self, item: &mut syn::ItemMod) {
        if let Some((_, items)) = &mut item.content {
            *items = std::mem::take(items).into_iter().filter_map(|mut child| {
                if self.keep_item(&mut child) { self.visit_item_mut(&mut child); Some(child) } else { None }
            }).collect();
        }
    }
}
fn normalize(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => { result.pop(); }
            std::path::Component::CurDir => {}
            other => result.push(other.as_os_str()),
        }
    }
    result
}
