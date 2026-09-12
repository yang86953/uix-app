use super::{BuildPlan, PackagePlan, source};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn fingerprint(plan: &BuildPlan) -> Result<String, String> {
    let mut hash = Sha256::new();
    hash.update(b"uix-demand-view-1\0");
    let mut identity = plan.clone();
    identity.fingerprint.clear();
    hash.update(serde_json::to_vec(&identity).map_err(|e| e.to_string())?);
    hash_directory(&mut hash, &plan.application, &plan.application)?;
    for package in plan.packages.values() { hash_directory(&mut hash, &package.source, &package.source)?; }
    Ok(format!("{:x}", hash.finalize()))
}

fn entries(path: &Path) -> Result<Vec<PathBuf>, String> {
    let mut result = fs::read_dir(path).map_err(|e| format!("{}: {e}", path.display()))?
        .map(|entry| entry.map(|e| e.path()).map_err(|e| e.to_string())).collect::<Result<Vec<_>, _>>()?;
    result.sort();
    Ok(result)
}
fn ignored(path: &Path) -> bool {
    matches!(path.file_name().and_then(|v| v.to_str()), Some(".git" | "target" | "node_modules" | ".uix-cache"))
}
fn hash_directory(hash: &mut Sha256, root: &Path, directory: &Path) -> Result<(), String> {
    for path in entries(directory)? {
        if ignored(&path) { continue; }
        let relative = path.strip_prefix(root).map_err(|e| e.to_string())?;
        hash.update(relative.to_string_lossy().as_bytes());
        hash.update([0]);
        let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if metadata.file_type().is_symlink() { return Err(format!("build source contains a symlink; declare its target explicitly: {}", path.display())); }
        if metadata.is_dir() { hash.update(b"directory\0"); hash_directory(hash, root, &path)?; }
        else if metadata.is_file() {
            let bytes = fs::read(&path).map_err(|e| e.to_string())?;
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        }
    }
    Ok(())
}

pub(super) fn materialize(plan: &BuildPlan) -> Result<PathBuf, String> {
    let cache = plan.application.join("target/uix/views");
    fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let destination = cache.join(&plan.fingerprint);
    let complete = destination.join("plan.json");
    if complete.is_file() {
        let previous: BuildPlan = super::read_json(&complete)?;
        if previous.fingerprint == plan.fingerprint { return Ok(destination.join("application/Cargo.toml")); }
        return Err("build cache identity mismatch".into());
    }
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
    let staging = cache.join(format!(".{}-{}-{nonce}", plan.fingerprint, std::process::id()));
    fs::create_dir(&staging).map_err(|e| e.to_string())?;
    let result = write_view(plan, &staging);
    if let Err(error) = result {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    if fingerprint(plan)? != plan.fingerprint {
        let _ = fs::remove_dir_all(&staging);
        return Err("source changed while preparing the build view; run the build again".into());
    }
    // The completion record is written last. A concurrent identical planner
    // may win the rename; its completed result has the same content identity.
    fs::write(staging.join("plan.json"), serde_json::to_vec_pretty(plan).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    if let Err(error) = fs::rename(&staging, &destination) {
        let _ = fs::remove_dir_all(&staging);
        if !complete.is_file() { return Err(error.to_string()); }
    }
    Ok(destination.join("application/Cargo.toml"))
}

fn write_view(plan: &BuildPlan, root: &Path) -> Result<(), String> {
    let active: BTreeMap<_, _> = plan.packages.iter().filter(|(_, p)| !p.selected.is_empty()).collect();
    for (name, package) in &active {
        let destination = root.join("packages").join(name);
        copy_package(package, &package.source, &destination)?;
        generate_facades(package, &destination)?;
        if destination.join("src/build_policy.rs").is_file() {
            let libraries = plan.packages.values().map(|p| {
                let mut descriptor = p.descriptor.clone();
                descriptor.components.retain(|_, c| plan.dynamic.contains(&c.unit));
                descriptor.data_constructors.retain(|_, c| plan.dynamic.contains(&c.unit));
                descriptor.value_types.retain(|_, c| plan.dynamic.contains(&c.unit));
                descriptor.theme_factory = None;
                descriptor.text_factory = None;
                serde_json::json!({"components": descriptor.components, "data_constructors": descriptor.data_constructors, "value_types": descriptor.value_types})
            }).collect::<Vec<_>>();
            let libraries = serde_json::to_string(&libraries).map_err(|e| e.to_string())?;
            let policy = format!("pub(crate) const STRICT: bool = true;\npub(crate) const DYNAMIC: &[&str] = &{:?};\n#[cfg(feature = \"uix-dynamic\")]\npub(crate) const LIBRARIES: &str = {:?};\npub(crate) fn require_dynamic(unit: &str) -> Result<(), String> {{ if DYNAMIC.contains(&unit) {{ Ok(()) }} else {{ Err(format!(\"dynamic unit {{unit:?}} was not declared by this build\")) }} }}\n", plan.dynamic.iter().collect::<Vec<_>>(), libraries);
            fs::write(destination.join("src/build_policy.rs"), policy).map_err(|e| e.to_string())?;
        }
        fs::write(destination.join("uix-host-library.json"), serde_json::to_vec_pretty(&package.descriptor).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let mut descriptor = package.descriptor.clone();
        descriptor.components.retain(|_, component| {
            component.unit.strip_prefix(&format!("{name}/")).is_some_and(|unit| package.selected.contains(unit))
        });
        let selected = |unit: &str| unit.strip_prefix(&format!("{name}/")).is_some_and(|unit| package.selected.contains(unit));
        descriptor.data_constructors.retain(|_, data| selected(&data.unit));
        descriptor.value_types.retain(|_, value| selected(&value.unit));
        if descriptor.theme_factory.as_ref().is_some_and(|factory| !selected(&factory.unit)) { descriptor.theme_factory = None; }
        if descriptor.text_factory.as_ref().is_some_and(|factory| !selected(&factory.unit)) { descriptor.text_factory = None; }
        fs::write(destination.join("uix-library.json"), serde_json::to_vec_pretty(&descriptor).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        rewrite_manifest(plan, &package.source, &destination, Some(package))?;
    }
    let application = root.join("application");
    copy_application(&plan.application, &application)?;
    rewrite_manifest(plan, &plan.application, &application, None)?;
    // Compiler discovery inside the cache must resolve the selected package views.
    // Original relative checkout paths do not describe the cache layout.
    let mut config = serde_json::json!({});
    if let Ok(bytes) = fs::read(plan.application.join("uix.json")) {
        config = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    }
    config["packages"] = serde_json::Value::Array(active.keys().map(|name| serde_json::json!({"path": format!("../packages/{name}"), "descriptor": "uix-host-library.json"})).collect());
    fs::write(application.join("uix.json"), serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let resources = root.join("resources");
    for (name, package) in active {
        for resource in &package.resources {
            let from = package.source.join(resource);
            let to = resources.join(name).join(resource);
            copy_file(&from, &to)?;
        }
    }
    Ok(())
}
fn generate_facades(package: &PackagePlan, destination: &Path) -> Result<(), String> {
    let mut slots = BTreeMap::<String, Vec<String>>::new();
    for unit in &package.selected {
        for (slot, statements) in &package.descriptor.units[unit].install {
            slots.entry(slot.clone()).or_default().extend(statements.clone());
        }
    }
    for (path, template) in &package.descriptor.generated {
        let mut source = template.clone();
        while let Some(start) = source.find("{{uix:") {
            let end = source[start..].find("}}").ok_or("unterminated facade insertion point")? + start;
            let name = &source[start + 6..end];
            let statements = slots.get(name).map(|v| v.join("\n")).unwrap_or_default();
            source.replace_range(start..end + 2, &statements);
        }
        syn::parse_file(&source).map_err(|e| format!("generated facade {}: {e}", path.display()))?;
        let target = destination.join(path);
        fs::create_dir_all(target.parent().ok_or("facade path has no parent")?).map_err(|e| e.to_string())?;
        fs::write(target, source).map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn copy_file(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to.parent().ok_or("copy destination has no parent")?).map_err(|e| e.to_string())?;
    fs::copy(from, to).map_err(|e| format!("{}: {e}", from.display()))?;
    Ok(())
}
fn copy_application(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for path in entries(from)? {
        if ignored(&path) { continue; }
        let destination = to.join(path.file_name().ok_or("path has no filename")?);
        if path.is_dir() { copy_application(&path, &destination)?; }
        else { copy_file(&path, &destination)?; }
    }
    Ok(())
}
fn copy_package(package: &PackagePlan, directory: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|e| e.to_string())?;
    for path in entries(directory)? {
        if ignored(&path) { continue; }
        let relative = path.strip_prefix(&package.source).map_err(|e| e.to_string())?;
        // Development executables/tests/docs never become runtime package inputs.
        if matches!(relative.components().next().and_then(|c| c.as_os_str().to_str()), Some("tests" | "tests-src" | "examples" | "docs" | "scripts" | "editor")) { continue; }
        let target = destination.join(path.file_name().ok_or("path has no filename")?);
        if path.is_dir() { copy_package(package, &path, &target)?; continue; }
        let in_source = relative.starts_with("src");
        let selected = if in_source { source::includes(package, relative) }
            else { relative == Path::new("build.rs") || relative == Path::new("Cargo.toml") || relative == Path::new("LICENSE") || relative == Path::new("THIRD_PARTY_NOTICES.md") || package.resources.contains(relative) };
        if !selected { continue; }
        if in_source && path.extension().is_some_and(|v| v == "rs") {
            let input = fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let output = source::rewrite(package, relative, &input)?;
            fs::write(&target, output).map_err(|e| e.to_string())?;
        } else { copy_file(&path, &target)?; }
    }
    Ok(())
}

fn rewrite_manifest(plan: &BuildPlan, source: &Path, destination: &Path, selected: Option<&PackagePlan>) -> Result<(), String> {
    let input = fs::read_to_string(source.join("Cargo.toml")).map_err(|e| e.to_string())?;
    let mut manifest: toml::Value = toml::from_str(&input).map_err(|e| e.to_string())?;
    let table = manifest.as_table_mut().ok_or("Cargo manifest must be a table")?;
    let original_dependencies = runtime_dependency_aliases(table);
    // Each package is a view of the same package identity, not a helper crate.
    table.remove("workspace");
    table.insert("workspace".into(), toml::Value::Table(Default::default()));
    if selected.is_some() {
        table.remove("dev-dependencies");
        table.remove("test");
        table.remove("example");
        table.remove("bin");
        if let Some(package) = table.get_mut("package").and_then(toml::Value::as_table_mut) {
            package.insert("autotests".into(), false.into());
            package.insert("autoexamples".into(), false.into());
            package.insert("autobins".into(), false.into());
        }
    }
    let mut selected_features = selected.map(|p| p.features.iter().cloned().collect::<Vec<_>>()).unwrap_or_default();
    rewrite_dependencies(plan, table, source, destination, selected, false, &mut selected_features)?;
    if let Some(targets) = table.get_mut("target").and_then(toml::Value::as_table_mut) {
        for target in targets.iter_mut().filter_map(|(_, value)| value.as_table_mut()) {
            rewrite_dependencies(plan, target, source, destination, selected, false, &mut selected_features)?;
        }
    }
    let remaining_dependencies = runtime_dependency_aliases(table);
    let removed_dependencies: Vec<_> = original_dependencies.difference(&remaining_dependencies).collect();
    let features = table.entry("features").or_insert_with(|| toml::Value::Table(Default::default())).as_table_mut().ok_or("features must be a table")?;
    let declared_features: std::collections::BTreeSet<_> = features.keys().cloned().collect();
    for values in features.iter_mut().filter_map(|(_, value)| value.as_array_mut()) {
        values.retain(|value| {
            let Some(value) = value.as_str() else { return true; };
            !removed_dependencies.iter().any(|alias| {
                value == format!("dep:{alias}")
                    || value.starts_with(&format!("{alias}/"))
                    || value.starts_with(&format!("{alias}?/"))
                    || (value == alias.as_str() && !declared_features.contains(value))
            })
        });
    }
    if selected.is_some() { features.insert("default".into(), toml::Value::Array(Vec::new())); }
    selected_features.sort(); selected_features.dedup();
    features.insert("__uix_selected".into(), toml::Value::Array(selected_features.into_iter().map(toml::Value::String).collect()));
    fs::write(destination.join("Cargo.toml"), toml::to_string_pretty(&manifest).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

fn runtime_dependency_aliases(table: &toml::map::Map<String, toml::Value>) -> std::collections::BTreeSet<String> {
    let mut names = std::collections::BTreeSet::new();
    if let Some(deps) = table.get("dependencies").and_then(toml::Value::as_table) {
        names.extend(deps.keys().cloned());
    }
    if let Some(targets) = table.get("target").and_then(toml::Value::as_table) {
        for target in targets.values().filter_map(toml::Value::as_table) {
            names.extend(runtime_dependency_aliases(target));
        }
    }
    names
}

fn rewrite_dependencies(plan: &BuildPlan, table: &mut toml::map::Map<String, toml::Value>, source: &Path, destination: &Path, selected: Option<&PackagePlan>, _host: bool, selected_features: &mut Vec<String>) -> Result<(), String> {
    for section in ["dependencies", "build-dependencies"] {
        let host = section == "build-dependencies";
        let Some(dependencies) = table.get_mut(section).and_then(toml::Value::as_table_mut) else { continue; };
        let names: Vec<_> = dependencies.keys().cloned().collect();
        for alias in names {
            let value = dependencies.get_mut(&alias).ok_or("dependency disappeared")?;
            if let Some(version) = value.as_str().map(str::to_owned) {
                *value = toml::Value::Table([(String::from("version"), version.into())].into_iter().collect());
            }
            let spec = value.as_table_mut().ok_or("dependency declaration must be a string or table")?;
            let name = spec.get("package").and_then(toml::Value::as_str).unwrap_or(&alias).to_string();
            if let Some(package) = plan.packages.get(&name) {
                if package.selected.is_empty() && !host {
                    dependencies.remove(&alias);
                    continue;
                }
                if package.selected.is_empty() { return Err(format!("host dependency {name} has no selected package view")); }
                let relative = if selected.is_some() { PathBuf::from("..").join(&name) }
                    else { PathBuf::from("../packages").join(&name) };
                spec.remove("git"); spec.remove("branch"); spec.remove("rev"); spec.remove("tag");
                spec.insert("path".into(), relative.to_string_lossy().to_string().into());
                spec.insert("default-features".into(), false.into());
                if !host {
                    spec.insert("features".into(), toml::Value::Array(vec!["__uix_selected".into()]));
                    if selected.is_none() {
                        spec.insert("optional".into(), true.into());
                        selected_features.push(format!("dep:{alias}"));
                    }
                }
            } else if let Some(path) = spec.get("path").and_then(toml::Value::as_str).map(str::to_owned) {
                let absolute = source.join(path).canonicalize().map_err(|e| e.to_string())?;
                spec.insert("path".into(), absolute.to_string_lossy().to_string().into());
            }
            if !host {
                if let Some(package) = selected {
                    // Candidate dependencies remain resolvable for host tools,
                    // but only the selected set becomes a target compilation edge.
                    if !plan.packages.contains_key(&name) {
                        spec.insert("optional".into(), true.into());
                        if package.dependencies.contains(&alias) { selected_features.push(format!("dep:{alias}")); }
                    }
                }
            }
        }
    }
    let _ = destination;
    Ok(())
}
