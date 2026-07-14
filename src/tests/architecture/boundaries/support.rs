use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn read_source(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path)
        .expect("read source")
        .replace("\r\n", "\n")
}

pub(super) fn rust_files_under(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rust_files(dir, &mut files);
    files
}

pub(super) fn collect_rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
}

pub(super) fn relative_src_path(path: &Path) -> String {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    path.strip_prefix(src)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/")
}

pub(super) fn is_native_backend_boundary(path: &str) -> bool {
    let path = path.strip_prefix("tests/").unwrap_or(path);
    path == "native/factory.rs"
        || path.starts_with("native/factory/")
        || path.starts_with("native/backends/")
        || path.starts_with("native/graphics/")
}

pub(super) fn is_platform_cfg_boundary(path: &str) -> bool {
    let path = path.strip_prefix("tests/").unwrap_or(path);
    path == "native/factory.rs"
        || path.starts_with("native/factory/")
        || path.starts_with("native/backends/")
        || path.starts_with("native/agent_transport/")
        || (path.starts_with("native/graphics/")
            && path.split('/').any(|component| component == "platform"))
}

pub(super) fn is_architecture_guard(path: &str) -> bool {
    path == "tests/architecture/boundaries.rs" || path.starts_with("tests/architecture/boundaries/")
}

pub(super) fn rust_code_without_comments(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'r' {
            let mut quote = index + 1;
            while quote < bytes.len() && bytes[quote] == b'#' {
                quote += 1;
            }
            if quote < bytes.len() && bytes[quote] == b'"' {
                let hashes = quote - index - 1;
                output.extend_from_slice(&bytes[index..=quote]);
                index = quote + 1;
                while index < bytes.len() {
                    output.push(bytes[index]);
                    if bytes[index] == b'"'
                        && index + hashes < bytes.len()
                        && (hashes == 0
                            || bytes[index + 1..=index + hashes]
                                .iter()
                                .all(|byte| *byte == b'#'))
                    {
                        if hashes > 0 {
                            output.extend_from_slice(&bytes[index + 1..=index + hashes]);
                        }
                        index += hashes + 1;
                        break;
                    }
                    index += 1;
                }
                continue;
            }
        }

        if bytes[index] == b'"' {
            output.push(bytes[index]);
            index += 1;
            while index < bytes.len() {
                output.push(bytes[index]);
                if bytes[index] == b'\\' && index + 1 < bytes.len() {
                    index += 1;
                    output.push(bytes[index]);
                } else if bytes[index] == b'"' {
                    index += 1;
                    break;
                }
                index += 1;
            }
            continue;
        }

        if bytes[index..].starts_with(b"//") {
            while index < bytes.len() && bytes[index] != b'\n' {
                output.push(b' ');
                index += 1;
            }
            continue;
        }

        if bytes[index..].starts_with(b"/*") {
            let mut depth = 1;
            output.extend_from_slice(b"  ");
            index += 2;
            while index < bytes.len() && depth > 0 {
                if bytes[index..].starts_with(b"/*") {
                    depth += 1;
                    output.extend_from_slice(b"  ");
                    index += 2;
                } else if bytes[index..].starts_with(b"*/") {
                    depth -= 1;
                    output.extend_from_slice(b"  ");
                    index += 2;
                } else {
                    output.push(if bytes[index] == b'\n' { b'\n' } else { b' ' });
                    index += 1;
                }
            }
            continue;
        }

        output.push(bytes[index]);
        index += 1;
    }

    String::from_utf8(output).expect("comment stripping must preserve UTF-8 source")
}

pub(super) fn skip_string(bytes: &[u8], index: &mut usize) -> bool {
    if bytes.get(*index) == Some(&b'r') {
        let mut quote = *index + 1;
        while quote < bytes.len() && bytes[quote] == b'#' {
            quote += 1;
        }
        if quote < bytes.len() && bytes[quote] == b'"' {
            let hashes = quote - *index - 1;
            *index = quote + 1;
            while *index < bytes.len() {
                if bytes[*index] == b'"'
                    && *index + hashes < bytes.len()
                    && (hashes == 0
                        || bytes[*index + 1..=*index + hashes]
                            .iter()
                            .all(|byte| *byte == b'#'))
                {
                    *index += hashes + 1;
                    return true;
                }
                *index += 1;
            }
            return true;
        }
    }

    if bytes.get(*index) != Some(&b'"') {
        return false;
    }
    *index += 1;
    while *index < bytes.len() {
        if bytes[*index] == b'\\' && *index + 1 < bytes.len() {
            *index += 2;
        } else if bytes[*index] == b'"' {
            *index += 1;
            break;
        } else {
            *index += 1;
        }
    }
    true
}

pub(super) fn rust_code_without_comments_or_strings(text: &str) -> String {
    let code = rust_code_without_comments(text);
    let bytes = code.as_bytes();
    let mut output = bytes.to_vec();
    let mut index = 0;

    while index < bytes.len() {
        let string_start = index;
        if skip_string(bytes, &mut index) {
            for byte in &mut output[string_start..index] {
                if *byte != b'\n' {
                    *byte = b' ';
                }
            }
        } else {
            index += 1;
        }
    }

    String::from_utf8(output).expect("source masking must preserve UTF-8 source")
}

pub(super) fn cfg_attribute_mentions_platform(attribute: &str) -> bool {
    let bytes = attribute.as_bytes();
    let mut identifiers = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        if skip_string(bytes, &mut index) {
            continue;
        }
        if bytes[index].is_ascii_alphabetic() || bytes[index] == b'_' {
            let start = index;
            index += 1;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
            {
                index += 1;
            }
            identifiers.push(&attribute[start..index]);
        } else {
            index += 1;
        }
    }

    matches!(identifiers.first(), Some(&"cfg") | Some(&"cfg_attr"))
        && identifiers.iter().skip(1).any(|identifier| {
            matches!(
                *identifier,
                "windows" | "unix" | "target_os" | "target_family"
            )
        })
}

pub(super) fn platform_cfg_attribute_lines(text: &str) -> Vec<usize> {
    let code = rust_code_without_comments(text);
    let bytes = code.as_bytes();
    let mut lines = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        if skip_string(bytes, &mut index) {
            continue;
        }
        if bytes[index] != b'#' {
            index += 1;
            continue;
        }

        let attribute_offset = index;
        index += 1;
        if bytes.get(index) == Some(&b'!') {
            index += 1;
        }
        while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
            index += 1;
        }
        if bytes.get(index) != Some(&b'[') {
            continue;
        }

        let attribute_start = index + 1;
        let mut depth = 1;
        index += 1;
        while index < bytes.len() && depth > 0 {
            if skip_string(bytes, &mut index) {
                continue;
            }
            match bytes[index] {
                b'[' => depth += 1,
                b']' => depth -= 1,
                _ => {}
            }
            index += 1;
        }

        if depth == 0
            && cfg_attribute_mentions_platform(&code[attribute_start..index.saturating_sub(1)])
        {
            lines.push(
                code[..attribute_offset]
                    .bytes()
                    .filter(|byte| *byte == b'\n')
                    .count()
                    + 1,
            );
        }
    }

    lines
}

pub(super) fn assert_domain_has_no_forbidden_dependencies(domain: &str, forbidden: &[&str]) {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let root = src.join(domain);
    let mut violations = Vec::new();

    for file in rust_files_under(&root) {
        let text = rust_code_without_comments_or_strings(&fs::read_to_string(&file).unwrap());
        let rel = relative_src_path(&file);
        for needle in forbidden {
            if text.contains(needle) {
                violations.push(format!("{rel} contains {needle}"));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "{domain} domain must not depend on forbidden upper domains: {violations:?}"
    );
}
