//! Data declarations for Rust-backed components. Libraries use this same
//! interface regardless of whether they are first-party or application-local.

use serde::{Deserialize, Serialize};
use std::{cell::RefCell, collections::BTreeMap, path::Path, sync::Arc};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentDeclaration {
    /// Portable module projection contract supplied by the owning library.
    #[serde(default)]
    pub module_view: Option<ModuleViewDeclaration>,
    /// Compilation unit that owns this component, qualified by package name.
    pub unit: String,
    #[serde(default)]
    pub category: Option<String>,
    /// A Rust expression; `$children` and declared property names are inputs.
    pub constructor: String,
    #[serde(default)]
    pub properties: BTreeMap<String, PropertyDeclaration>,
    /// Setter ordering, where public builder operations have an ordering contract.
    #[serde(default)]
    pub property_order: Vec<String>,
    #[serde(default)]
    pub exclusive_properties: Vec<Vec<String>>,
    #[serde(default)]
    pub required_any: Vec<Vec<String>>,
    #[serde(default)]
    pub events: BTreeMap<String, EventDeclaration>,
    #[serde(default)]
    pub slots: BTreeMap<String, SlotDeclaration>,
    #[serde(default)]
    pub children: ChildrenDeclaration,
    /// Rust expression applied after all component properties and events.
    #[serde(default)]
    pub finish: Option<String>,
    #[serde(default)]
    pub record: bool,
    #[serde(default)]
    pub reject_events: bool,
    /// For a record that owns a View, applies common decoration through an
    /// ordinary Rust mapping function using `$record`, `$view`, `$decorated`.
    #[serde(default)]
    pub view_map: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleViewDeclaration {
    #[serde(default)]
    pub properties: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub event: Option<String>,
    #[serde(default)]
    pub event_fields: BTreeMap<String, String>,
    #[serde(default)]
    pub children: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PropertyDeclaration {
    #[serde(default)]
    pub kind: PropertyKind,
    /// A public Rust builder method, or a Rust expression using `$widget/$value`.
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub rust: Option<String>,
    /// A distinct public API for an expression-backed value (for example a
    /// state handle) versus a static literal.
    #[serde(default)]
    pub expression_rust: Option<String>,
    #[serde(default)]
    pub allow_expression: bool,
    #[serde(default)]
    pub state_handle: Option<String>,
    #[serde(default)]
    pub separator: Option<String>,
    /// Literal spelling to typed Rust value. Unknown spellings are diagnosed.
    #[serde(default)]
    pub values: BTreeMap<String, String>,
    /// Default Rust value, used for constructor arguments when absent.
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub nonempty: bool,
    #[serde(default)]
    pub owned: bool,
    #[serde(default)]
    pub unique: bool,
    #[serde(default)]
    pub literal_only: bool,
    #[serde(default)]
    pub literal_pattern: Option<String>,
    #[serde(default)]
    pub call_only: bool,
    #[serde(default)]
    pub maximum_items: Option<usize>,
    #[serde(default)]
    pub fields: BTreeMap<String, PropertyDeclaration>,
    /// Constructs a typed value from object fields or an array's `$items`.
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub item: Option<Box<PropertyDeclaration>>,
    #[serde(default)]
    pub minimum: Option<f64>,
    #[serde(default)]
    pub exclusive_minimum: Option<f64>,
    #[serde(default)]
    pub maximum: Option<f64>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PropertyKind {
    #[default]
    Expression,
    Identifier,
    String,
    Number,
    Integer,
    Boolean,
    Enumeration,
    State,
    Object,
    Array,
    GridTracks,
    Color,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventDeclaration {
    /// Rust expression using `$widget`, `$handler` and `$event`.
    pub rust: String,
    #[serde(default)]
    pub fields: Vec<String>,
    #[serde(default)]
    pub no_payload: bool,
    #[serde(default)]
    pub bare_handler_payload: bool,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub before_finish: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlotDeclaration {
    #[serde(default)]
    pub rust: String,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub trailing: bool,
    #[serde(default)]
    pub bound_events: BTreeMap<String, String>,
    #[serde(default)]
    pub static_element: bool,
    #[serde(default)]
    pub exclusive_properties: Vec<String>,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeFactory {
    pub unit: String,
    /// Creates a Theme from `$dark`, optional `$primary`/`$background` colors,
    /// and a typed `$patch`. Color derivation stays in the owning library.
    pub rust: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ChildrenDeclaration {
    #[default]
    None,
    Text,
    TextFunction,
    Views {
        #[serde(default)]
        minimum: Option<usize>,
        #[serde(default)]
        maximum: Option<usize>,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        item_finish: Option<String>,
    },
    Single {
        #[serde(default)]
        optional: bool,
        #[serde(default)]
        static_element: bool,
    },
    Fold {
        initial: String,
        tags: Vec<String>,
        #[serde(default)]
        minimum: Option<usize>,
    },
    Lazy {
        data_property: String,
        #[serde(default)]
        item_property: Option<String>,
        render: String,
        render_keyed: String,
    },
    Records {
        tags: Vec<String>,
        #[serde(default)]
        minimum: Option<usize>,
    },
}

/// A library-owned value constructor and its typed method mappings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataDeclaration {
    pub unit: String,
    pub rust_path: String,
    #[serde(default = "default_constructor_method")]
    pub method: String,
    #[serde(default)]
    pub normalize_numbers: bool,
    #[serde(default)]
    pub call: Option<CallDeclaration>,
    #[serde(default)]
    pub methods: BTreeMap<String, CallDeclaration>,
}
fn default_constructor_method() -> String {
    "new".into()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CallDeclaration {
    pub parameters: Vec<PropertyDeclaration>,
    pub rust: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValueDeclaration {
    pub unit: String,
    pub rust_type: String,
    #[serde(default)]
    pub allowed_in_props: bool,
    #[serde(default)]
    pub allowed_in_state_props: bool,
    #[serde(default)]
    pub initial: PropertyDeclaration,
}

/// A catalog is immutable during one compilation and may be shared by tools.
#[derive(Debug, Clone, Default)]
pub struct ComponentCatalog {
    pub theme_tokens: BTreeMap<String, super::projection_schema::ThemeTokenSpec>,
    declarations: BTreeMap<String, ComponentDeclaration>,
    theme: Option<ThemeFactory>,
    text: Option<ThemeFactory>,
    data: BTreeMap<String, DataDeclaration>,
    values: BTreeMap<String, ValueDeclaration>,
}

impl ComponentCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Uses the nearest project's descriptor/configuration. No official package
    /// names or locations are embedded in the language implementation.
    pub fn for_project(path: &Path) -> Result<Self, String> {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|e| e.to_string())?
                .join(path)
        };
        let directory = if absolute.is_dir() {
            absolute.as_path()
        } else {
            absolute.parent().unwrap_or(&absolute)
        };
        for directory in directory.ancestors() {
            let configuration = directory.join("uix.json");
            if configuration.is_file() {
                #[derive(Deserialize)]
                struct Package {
                    path: std::path::PathBuf,
                    #[serde(default)]
                    descriptor: Option<std::path::PathBuf>,
                }
                #[derive(Deserialize)]
                struct Configuration {
                    #[serde(default)]
                    packages: Vec<Package>,
                }
                let configuration: Configuration = serde_json::from_slice(
                    &std::fs::read(&configuration).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                let mut catalog = Self::new();
                for package in configuration.packages {
                    catalog.read_library(
                        &directory.join(package.path).join(
                            package
                                .descriptor
                                .unwrap_or_else(|| "uix-library.json".into()),
                        ),
                    )?;
                }
                return Ok(catalog);
            }
            let descriptor = directory.join("uix-library.json");
            if descriptor.is_file() {
                let mut catalog = Self::new();
                catalog.read_library(&descriptor)?;
                return Ok(catalog);
            }
        }
        Ok(Self::new())
    }

    pub fn insert(
        &mut self,
        name: String,
        declaration: ComponentDeclaration,
    ) -> Result<(), String> {
        if name.is_empty()
            || declaration.constructor.trim().is_empty()
            || declaration.unit.is_empty()
        {
            return Err("component name, unit and constructor must be nonempty".into());
        }
        if self.declarations.contains_key(&name) {
            return Err(format!("duplicate component declaration {name}"));
        }
        self.declarations.insert(name, declaration);
        Ok(())
    }

    /// Reads the component section of the same descriptor used by the build planner.
    pub fn read_library(&mut self, path: &Path) -> Result<(), String> {
        let source =
            std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        self.read_library_source(&source)
    }

    /// Loads an embedded library declaration without accessing the filesystem.
    pub fn read_library_source(&mut self, source: &str) -> Result<(), String> {
        #[derive(Deserialize)]
        struct Library {
            #[serde(default)]
            theme_tokens: BTreeMap<String, super::projection_schema::ThemeTokenSpec>,
            #[serde(default)]
            components: BTreeMap<String, ComponentDeclaration>,
            #[serde(default)]
            theme_factory: Option<ThemeFactory>,
            #[serde(default)]
            text_factory: Option<ThemeFactory>,
            #[serde(default)]
            data_constructors: BTreeMap<String, DataDeclaration>,
            #[serde(default)]
            value_types: BTreeMap<String, ValueDeclaration>,
        }
        let library: Library = serde_json::from_str(source).map_err(|e| e.to_string())?;
        for (name, declaration) in library.data_constructors {
            syn::parse_str::<syn::Path>(&declaration.rust_path)
                .map_err(|e| format!("invalid data constructor path {name}: {e}"))?;
            if self.data.insert(name.clone(), declaration).is_some() {
                return Err(format!("duplicate data constructor {name}"));
            }
        }
        for (name, declaration) in library.value_types {
            syn::parse_str::<syn::Type>(&declaration.rust_type)
                .map_err(|e| format!("invalid value type {name}: {e}"))?;
            if self.values.insert(name.clone(), declaration).is_some() {
                return Err(format!("duplicate value type {name}"));
            }
        }
        for (name, declaration) in library.components {
            self.insert(name, declaration)?;
        }
        for (name, mut token) in library.theme_tokens {
            if token.key.is_empty() || token.unit.is_empty() {
                return Err(format!("token {name} needs a key and owning unit"));
            }
            token.name = name.clone();
            token.id = format!("theme.{name}");
            if self.theme_tokens.insert(name.clone(), token).is_some() {
                return Err(format!("duplicate theme token {name}"));
            }
        }

        if let Some(text) = library.text_factory {
            if self.text.is_some() {
                return Err("multiple default text factories".into());
            }
            self.text = Some(text);
        }
        if let Some(theme) = library.theme_factory {
            if self.theme.is_some() {
                return Err("multiple default theme factories; select one theme provider in the library catalog".into());
            }
            self.theme = Some(theme);
        }
        Ok(())
    }

    pub fn declarations(&self) -> &BTreeMap<String, ComponentDeclaration> {
        &self.declarations
    }

    /// The scope is restored even when a compiler operation unwinds. Catalogs
    /// from independent threads and nested compiler invocations cannot overlap.
    pub fn with<R>(self: &Arc<Self>, operation: impl FnOnce() -> R) -> R {
        struct Restore(Option<Arc<ComponentCatalog>>);
        impl Drop for Restore {
            fn drop(&mut self) {
                CURRENT.with(|current| {
                    *current.borrow_mut() = self.0.take();
                });
            }
        }
        let _restore = Restore(CURRENT.with(|current| current.replace(Some(self.clone()))));
        operation()
    }
}

thread_local! { static CURRENT: RefCell<Option<Arc<ComponentCatalog>>> = const { RefCell::new(None) }; }

pub(crate) fn declaration(name: &str) -> Option<ComponentDeclaration> {
    CURRENT.with(|current| current.borrow().as_ref()?.declarations.get(name).cloned())
}

pub(crate) fn names() -> Vec<String> {
    CURRENT.with(|current| {
        current
            .borrow()
            .as_ref()
            .map(|c| c.declarations.keys().cloned().collect())
            .unwrap_or_default()
    })
}

pub(crate) fn data_declaration(name: &str) -> Option<DataDeclaration> {
    CURRENT.with(|current| current.borrow().as_ref()?.data.get(name).cloned())
}
pub(crate) fn value_declaration(name: &str) -> Option<ValueDeclaration> {
    CURRENT.with(|current| current.borrow().as_ref()?.values.get(name).cloned())
}

pub(crate) fn text_factory() -> Option<ThemeFactory> {
    CURRENT.with(|current| current.borrow().as_ref()?.text.clone())
}

pub(crate) fn theme_factory() -> Option<ThemeFactory> {
    CURRENT.with(|current| current.borrow().as_ref()?.theme.clone())
}

pub(crate) fn has_scope() -> bool {
    CURRENT.with(|current| current.borrow().is_some())
}

pub(crate) fn with_project<R>(
    path: &Path,
    operation: impl FnOnce() -> R,
) -> Result<R, super::CompilerDiagnostic> {
    let catalog = ComponentCatalog::for_project(path)
        .map_err(|message| super::source_io_diagnostic(path, std::io::Error::other(message)))?;
    Ok(Arc::new(catalog).with(operation))
}

pub(crate) fn identity() -> u64 {
    use std::hash::Hasher;
    CURRENT.with(|current| {
        let current = current.borrow();
        let Some(catalog) = current.as_ref() else {
            return 0;
        };
        if catalog.declarations.is_empty()
            && catalog.theme.is_none()
            && catalog.text.is_none()
            && catalog.data.is_empty()
            && catalog.values.is_empty()
            && catalog.theme_tokens.is_empty()
        {
            return 0;
        }
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        if let Ok(bytes) = serde_json::to_vec(&(
            &catalog.declarations,
            &catalog.theme,
            &catalog.text,
            &catalog.data,
            &catalog.values,
            &catalog.theme_tokens,
        )) {
            hash.write(&bytes);
        }
        hash.finish()
    })
}

thread_local! { static USED_UNITS: RefCell<Vec<std::collections::BTreeSet<String>>> = const { RefCell::new(Vec::new()) }; }

pub(crate) fn use_unit(unit: &str) {
    USED_UNITS.with(|scopes| {
        if let Some(scope) = scopes.borrow_mut().last_mut() {
            scope.insert(unit.into());
        }
    });
}

pub(crate) fn capture_units<R>(
    operation: impl FnOnce() -> R,
) -> (R, std::collections::BTreeSet<String>) {
    struct Scope;
    impl Drop for Scope {
        fn drop(&mut self) {
            USED_UNITS.with(|scopes| {
                scopes.borrow_mut().pop();
            });
        }
    }
    USED_UNITS.with(|scopes| scopes.borrow_mut().push(Default::default()));
    let scope = Scope;
    let result = operation();
    let units = USED_UNITS.with(|scopes| scopes.borrow().last().cloned().unwrap_or_default());
    drop(scope);
    (result, units)
}

pub(crate) fn query_entries(kind: super::QueryKind) -> Vec<super::QueryEntry> {
    use super::{QueryEntry, QueryKind};
    fn entry(id: String, name: &str, unit: &str, value: &impl Serialize) -> QueryEntry {
        QueryEntry::Library {
            id,
            name: name.into(),
            unit: unit.into(),
            declaration: serde_json::to_value(value).expect("declaration is serializable"),
        }
    }
    CURRENT.with(|current| {
        let current = current.borrow();
        let Some(catalog) = current.as_ref() else { return Vec::new(); };
        match kind {
            QueryKind::Components => catalog.declarations.iter().map(|(name, c)| entry(format!("component.{name}"), name, &c.unit, c)).collect(),
            QueryKind::Data => catalog.data.iter().map(|(name, c)| entry(format!("data.{name}"), name, &c.unit, c)).collect(),
            QueryKind::Handles => catalog.declarations.iter().flat_map(|(component, c)| c.properties.iter().filter(|(_, p)| p.state_handle.is_some() || matches!(p.kind, PropertyKind::State)).map(move |(name, p)| entry(format!("handle.{component}.{name}"), name, &c.unit, &serde_json::json!({"component":component,"attribute":name,"value_type":p.state_handle.as_deref().unwrap_or("State<_>")})))).collect(),
            QueryKind::Slots => catalog.declarations.iter().flat_map(|(component, c)| c.slots.iter().map(move |(name, slot)| entry(format!("slot.{component}.{name}"), name, &c.unit, &serde_json::json!({"component":component,"name":name,"required":slot.required,"multiple":false})))).collect(),
            _ => Vec::new(),
        }
    })
}

pub(crate) fn current() -> Arc<ComponentCatalog> {
    CURRENT.with(|current| current.borrow().clone().unwrap_or_default())
}
