//! Pre-Cargo compilation planning. Libraries describe units; this module owns
//! dependency closure, source selection, cache identity and Cargo views.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

mod source;
mod view;

/// Application-owned build entrypoints. Paths are relative to the config file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildConfig {
    pub packages: Vec<PackageInput>,
    #[serde(default)]
    pub rust: Vec<String>,
    #[serde(default)]
    pub sources: Vec<PathBuf>,
    #[serde(default)]
    pub dynamic: Vec<String>,
    #[serde(default)]
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageInput {
    pub path: PathBuf,
    #[serde(default = "default_descriptor")]
    pub descriptor: PathBuf,
}
fn default_descriptor() -> PathBuf { "uix-library.json".into() }

/// Static library data. No hooks or executable pruning code are accepted.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LibraryDescriptor {
    pub version: u32,
    pub package: String,
    pub units: BTreeMap<String, Unit>,
    #[serde(default)]
    pub always: Vec<String>,
    /// Shared source modules, independent of any single component.
    #[serde(default)]
    pub shared: Vec<PathBuf>,
    /// Explicit ownership of public names in shared Rust source.
    #[serde(default)]
    pub symbols: BTreeMap<String, String>,
    /// Shared enums whose alternatives belong to individual units.
    #[serde(default)]
    pub enums: BTreeMap<String, BTreeMap<String, String>>,
    /// Cargo feature names used only as development compatibility switches.
    #[serde(default)]
    pub feature_units: BTreeMap<String, Vec<String>>,
    /// Normal Rust facade templates. Only named statement insertion points
    /// are expanded; templates never run in the planner process.
    #[serde(default)]
    pub generated: BTreeMap<PathBuf, String>,
    #[serde(default)]
    pub components: BTreeMap<String, crate::lang::compiler::components::ComponentDeclaration>,
    #[serde(default)]
    pub theme_factory: Option<crate::lang::compiler::components::ThemeFactory>,
    #[serde(default)]
    pub text_factory: Option<crate::lang::compiler::components::ThemeFactory>,
    #[serde(default)]
    pub data_constructors: BTreeMap<String, crate::lang::compiler::components::DataDeclaration>,
    #[serde(default)]
    pub value_types: BTreeMap<String, crate::lang::compiler::components::ValueDeclaration>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Unit {
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub modules: Vec<PathBuf>,
    #[serde(default)]
    pub exports: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub resources: Vec<PathBuf>,
    /// Rust expression statements inserted into named facade template slots.
    #[serde(default)]
    pub install: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackagePlan {
    pub source: PathBuf,
    pub descriptor: LibraryDescriptor,
    pub selected: BTreeSet<String>,
    pub dependencies: BTreeSet<String>,
    pub features: BTreeSet<String>,
    pub resources: BTreeSet<PathBuf>,
}

/// One immutable plan drives both target compilation and resource delivery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildPlan {
    pub schema: u32,
    pub application: PathBuf,
    pub target: String,
    pub roots: BTreeSet<String>,
    pub dynamic: BTreeSet<String>,
    pub packages: BTreeMap<String, PackagePlan>,
    pub fingerprint: String,
}

impl BuildPlan {
    /// Dynamic loaders must consult the exact allowlist saved with the build.
    pub fn permits_dynamic(&self, unit: &str) -> bool { self.dynamic.contains(unit) }

    pub fn require_dynamic(&self, unit: &str) -> Result<(), String> {
        if self.permits_dynamic(unit) { Ok(()) }
        else { Err(format!("dynamic unit {unit:?} was not declared by this build")) }
    }

    /// Materializes a cached build view without modifying source repositories.
    pub fn materialize(&self) -> Result<PathBuf, String> { view::materialize(self) }
}

/// Reads the application config, follows UIX imports and computes the closure.
pub fn plan(config_path: &Path, target_override: Option<&str>) -> Result<BuildPlan, String> {
    let config_path = config_path.canonicalize().map_err(|e| e.to_string())?;
    let application = config_path.parent().ok_or("configuration has no parent")?.to_path_buf();
    let config: BuildConfig = read_json(&config_path)?;
    let mut packages = BTreeMap::new();
    let mut catalog = crate::lang::compiler::components::ComponentCatalog::new();
    for input in &config.packages {
        let root = application.join(&input.path).canonicalize().map_err(|e| format!("{}: {e}", input.path.display()))?;
        let descriptor: LibraryDescriptor = read_json(&root.join(&input.descriptor))?;
        if descriptor.version != 1 { return Err(format!("unsupported descriptor version {}", descriptor.version)); }
        validate_descriptor(&descriptor)?;
        catalog.read_library(&root.join(&input.descriptor))?;
        let name = descriptor.package.clone();
        if packages.insert(name.clone(), PackagePlan {
            source: root, descriptor, selected: BTreeSet::new(), dependencies: BTreeSet::new(),
            features: BTreeSet::new(), resources: BTreeSet::new(),
        }).is_some() { return Err(format!("duplicate library {name}")); }
    }
    let mut roots: BTreeSet<String> = config.rust.into_iter().chain(config.dynamic.iter().cloned()).collect();
    let mut tags = BTreeMap::<String, String>::new();
    for (name, package) in &packages {
        for (tag, component) in &package.descriptor.components {
            if let Some(old) = tags.insert(tag.clone(), component.unit.clone()) {
                if old != component.unit { return Err(format!("ambiguous UIX tag {tag}: {old}, {}", component.unit)); }
            }
        }
        for (unit_name, unit) in &package.descriptor.units {
            for tag in &unit.tags {
                if package.descriptor.components.contains_key(tag) { continue; }
                let qualified = format!("{name}/{unit_name}");
                if let Some(old) = tags.insert(tag.clone(), qualified.clone()) {
                    if old != qualified { return Err(format!("ambiguous UIX tag {tag}: {old}, {qualified}")); }
                }
            }
        }
    }
    std::sync::Arc::new(catalog).with(|| -> Result<(), String> {
        for source in &config.sources {
            let source = application.join(source);
            let text = std::fs::read_to_string(&source).map_err(|e| e.to_string())?;
            if crate::lang::compiler::modules::recognizes_source(&text) {
                let (output, units) = crate::lang::compiler::components::capture_units(|| crate::lang::compiler::modules::check_file(&source));
                output.map_err(|e| format!("{e:?}"))?;
                roots.extend(units);
            } else {
                let output = crate::lang::compiler::CompilerSystem::new().compile_file_auto(&source).map_err(|e| format!("{e:?}"))?;
                roots.extend(output.required_units);
            }
            for tag in source::uix_tags(&source)? {
                if let Some(unit) = tags.get(&tag) { roots.insert(unit.clone()); }
            }
        }
        Ok(())
    })?;
    let mut pending: Vec<String> = roots.iter().cloned().collect();
    while let Some(qualified) = pending.pop() {
        let (name, unit_name) = split_unit(&qualified)?;
        let package = packages.get_mut(name).ok_or_else(|| format!("unknown package in unit {qualified}"))?;
        let unit = package.descriptor.units.get(unit_name).cloned()
            .ok_or_else(|| format!("unknown compilation unit {qualified}"))?;
        if !package.selected.insert(unit_name.to_string()) { continue; }
        if package.selected.len() == 1 {
            pending.extend(package.descriptor.always.iter().map(|v| qualify(name, v)));
        }
        pending.extend(unit.requires.iter().map(|v| qualify(name, v)));
        package.dependencies.extend(unit.dependencies);
        package.features.extend(unit.features);
        package.resources.extend(unit.resources);
    }
    let target = target_override.map(str::to_owned).or(config.target).map(Ok).unwrap_or_else(host_target)?;
    let mut dynamic = BTreeSet::new();
    let mut pending = config.dynamic;
    while let Some(qualified) = pending.pop() {
        if !dynamic.insert(qualified.clone()) { continue; }
        let (name, unit) = split_unit(&qualified)?;
        let package = &packages[name];
        pending.extend(package.descriptor.units[unit].requires.iter().map(|v| qualify(name, v)));
        pending.extend(package.descriptor.always.iter().map(|v| qualify(name, v)));
    }
    let mut result = BuildPlan {
        schema: 1, application, target, roots, dynamic,
        packages, fingerprint: String::new(),
    };
    result.fingerprint = view::fingerprint(&result)?;
    Ok(result)
}

fn qualify(package: &str, unit: &str) -> String {
    if unit.contains('/') { unit.to_owned() } else { format!("{package}/{unit}") }
}
fn split_unit(unit: &str) -> Result<(&str, &str), String> {
    let (package, name) = unit.split_once('/').ok_or_else(|| format!("unit must be package/name: {unit}"))?;
    if package.is_empty() || name.is_empty() { return Err(format!("invalid unit {unit}")); }
    Ok((package, name))
}
fn validate_descriptor(descriptor: &LibraryDescriptor) -> Result<(), String> {
    if descriptor.package.is_empty() { return Err("descriptor package is empty".into()); }
    for (tag, component) in &descriptor.components {
        let (package, unit) = split_unit(&component.unit)?;
        if package != descriptor.package || !descriptor.units.contains_key(unit) {
            return Err(format!("component {tag} has unknown owning unit {}", component.unit));
        }
    }
    for (name, owner) in descriptor.data_constructors.iter().map(|(name, data)| (name.as_str(), data.unit.as_str()))
        .chain(descriptor.value_types.iter().map(|(name, value)| (name.as_str(), value.unit.as_str())))
        .chain(descriptor.theme_factory.iter().map(|factory| ("theme_factory", factory.unit.as_str())))
        .chain(descriptor.text_factory.iter().map(|factory| ("text_factory", factory.unit.as_str()))) {
        let (package, unit) = split_unit(owner)?;
        if package != descriptor.package || !descriptor.units.contains_key(unit) { return Err(format!("declaration {name} has unknown owning unit {owner}")); }
    }
    for unit in descriptor.units.values() {
        for path in unit.modules.iter().chain(&unit.resources).chain(&descriptor.shared).chain(descriptor.generated.keys()) {
            if path.is_absolute() || path.components().any(|p| matches!(p, std::path::Component::ParentDir | std::path::Component::Prefix(_))) {
                return Err(format!("descriptor paths must stay inside the package: {}", path.display()));
            }
        }
    }
    for owner in descriptor.symbols.values().chain(descriptor.enums.values().flat_map(|m| m.values())) {
        if !descriptor.units.contains_key(owner) { return Err(format!("unknown symbol owner {owner}")); }
    }
    Ok(())
}
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}
fn host_target() -> Result<String, String> {
    let output = std::process::Command::new("rustc").arg("-vV").output().map_err(|e| e.to_string())?;
    if !output.status.success() { return Err("rustc -vV failed".into()); }
    String::from_utf8_lossy(&output.stdout).lines().find_map(|line| line.strip_prefix("host: ").map(str::to_owned))
        .ok_or_else(|| "rustc did not report a host target".into())
}
