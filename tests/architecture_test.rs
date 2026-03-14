#![allow(clippy::unwrap_used, clippy::cognitive_complexity)]

use std::fs;
use std::path::Path;

const HEXAGON_DIR: &str = "src/hexagon";

const ALLOWED_USE_PREFIXES: &[&str] = &[
    "std::",
    "core::",
    "alloc::",
    "crate::hexagon::",
    "super::",
    "self::",
];

fn cfg_test_line(content: &str) -> Option<usize> {
    content
        .lines()
        .position(|line| line.trim() == "#[cfg(test)]")
}

fn collect_violations(path: &Path) -> Vec<String> {
    let content = fs::read_to_string(path).unwrap();
    let test_start = cfg_test_line(&content).unwrap_or(usize::MAX);
    let mut violations = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        if line_num >= test_start {
            break;
        }

        let trimmed = line.trim();
        if !trimmed.starts_with("use ") {
            continue;
        }

        let import_path = trimmed
            .trim_start_matches("use ")
            .trim_end_matches(';')
            .split("::{")
            .next()
            .unwrap_or("");

        let allowed = ALLOWED_USE_PREFIXES
            .iter()
            .any(|prefix| import_path.starts_with(prefix.trim_end_matches(':')));

        if !allowed {
            violations.push(format!("{}:{}: {}", path.display(), line_num + 1, trimmed));
        }
    }

    violations
}

#[test]
fn hexagon_must_not_import_outside_stdlib_and_itself() {
    let hexagon_path = Path::new(HEXAGON_DIR);
    assert!(hexagon_path.exists(), "hexagon directory not found");

    let mut violations = Vec::new();

    for entry in walkdir(hexagon_path) {
        if entry.extension().is_some_and(|e| e == "rs") {
            violations.extend(collect_violations(&entry));
        }
    }

    assert!(
        violations.is_empty(),
        "Hexagon boundary violated — the following imports reach outside std/core/alloc/crate::hexagon:\n\n{}",
        violations.join("\n"),
    );
}

const SRC_DIR: &str = "src";
// The hexagon is vacuum-sealed: only ports (contracts) and usecases (behavior)
// are exposed. Entities are private implementation details. Any data structures
// needed by external code must be defined as plain data in ports.
// No re-exports or tricks to bypass this boundary.
const ALLOWED_HEXAGON_MODULES: &[&str] = &["crate::hexagon::usecases", "crate::hexagon::ports"];

fn collect_hexagon_access_violations(path: &Path) -> Vec<String> {
    let content = fs::read_to_string(path).unwrap();
    let test_start = cfg_test_line(&content).unwrap_or(usize::MAX);
    let mut violations = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        if line_num >= test_start {
            break;
        }

        let trimmed = line.trim();
        if !trimmed.starts_with("use ") {
            continue;
        }

        let import_path = trimmed
            .trim_start_matches("use ")
            .trim_end_matches(';')
            .split("::{")
            .next()
            .unwrap_or("");

        let normalized = if import_path.starts_with("crate::hexagon") {
            import_path.to_string()
        } else if import_path.starts_with("mcp_gateway::hexagon") {
            import_path.replacen("mcp_gateway::", "crate::", 1)
        } else {
            continue;
        };

        let allowed = ALLOWED_HEXAGON_MODULES
            .iter()
            .any(|prefix| normalized.starts_with(prefix));

        if !allowed {
            violations.push(format!("{}:{}: {}", path.display(), line_num + 1, trimmed));
        }
    }

    violations
}

#[test]
fn outside_hexagon_must_only_import_ports_and_usecases() {
    let src_path = Path::new(SRC_DIR);
    let hexagon_path = Path::new(HEXAGON_DIR);
    assert!(src_path.exists(), "src directory not found");

    let mut violations = Vec::new();

    for entry in walkdir(src_path) {
        if entry.extension().is_none_or(|e| e != "rs") || entry.starts_with(hexagon_path) {
            continue;
        }
        violations.extend(collect_hexagon_access_violations(&entry));
    }

    assert!(
        violations.is_empty(),
        "Hexagon encapsulation violated — code outside the hexagon may only import from hexagon::ports and hexagon::usecases:\n\n{}",
        violations.join("\n"),
    );
}

fn collect_reexport_violations(path: &Path) -> Vec<String> {
    let content = fs::read_to_string(path).unwrap();
    let test_start = cfg_test_line(&content).unwrap_or(usize::MAX);
    let mut violations = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        if line_num >= test_start {
            break;
        }

        let trimmed = line.trim();
        if trimmed.starts_with("pub use ") {
            violations.push(format!("{}:{}: {}", path.display(), line_num + 1, trimmed));
        }
    }

    violations
}

#[test]
fn hexagon_must_not_contain_reexports() {
    let hexagon_path = Path::new(HEXAGON_DIR);
    assert!(hexagon_path.exists(), "hexagon directory not found");

    let mut violations = Vec::new();

    for entry in walkdir(hexagon_path) {
        if entry.extension().is_some_and(|e| e == "rs") {
            violations.extend(collect_reexport_violations(&entry));
        }
    }

    assert!(
        violations.is_empty(),
        "Hexagon must not contain pub use re-exports — define types where they belong:\n\n{}",
        violations.join("\n"),
    );
}

const ALLOWED_PUBLIC_HEXAGON_MODULES: &[&str] = &["ports", "usecases"];

#[test]
fn hexagon_mod_must_only_expose_ports_and_usecases() {
    let content = fs::read_to_string(Path::new("src/hexagon/mod.rs")).unwrap();
    let mut violations = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if !trimmed.starts_with("pub mod ") {
            continue;
        }
        let module_name = trimmed.trim_start_matches("pub mod ").trim_end_matches(';');
        if !ALLOWED_PUBLIC_HEXAGON_MODULES.contains(&module_name) {
            violations.push(format!(
                "src/hexagon/mod.rs:{}: `pub mod {module_name}` — only ports and usecases may be public",
                line_num + 1
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "Hexagon mod.rs exposes internal modules — entities must be private:\n\n{}",
        violations.join("\n"),
    );
}

// Usecases must be vertical slices: no lateral dependencies between them.
// Only composition roots (gateway.rs, registry_service.rs) may import sibling usecases.
const USECASE_DIR: &str = "src/hexagon/usecases";
const COMPOSITION_ROOTS: &[&str] = &["gateway.rs", "registry_service.rs", "mod.rs"];

fn collect_lateral_usecase_violations(path: &Path) -> Vec<String> {
    let file_name = path.file_name().unwrap().to_str().unwrap();
    if COMPOSITION_ROOTS.contains(&file_name) {
        return vec![];
    }

    let content = fs::read_to_string(path).unwrap();
    let test_start = cfg_test_line(&content).unwrap_or(usize::MAX);

    let usecase_files: Vec<String> = fs::read_dir(USECASE_DIR)
        .unwrap()
        .filter_map(|e| {
            let name = e.ok()?.file_name().to_str()?.to_string();
            let stem = name.strip_suffix(".rs")?;
            if name == file_name || name == "mod.rs" {
                None
            } else {
                Some(stem.to_string())
            }
        })
        .collect();

    let mut violations = Vec::new();
    for (line_num, line) in content.lines().enumerate() {
        if line_num >= test_start {
            break;
        }
        let trimmed = line.trim();
        if !trimmed.starts_with("use super::") {
            continue;
        }
        let after_super = trimmed.trim_start_matches("use super::");
        let target_module = after_super.split("::").next().unwrap_or("");

        // Allow importing from composition roots (gateway, registry_service)
        if COMPOSITION_ROOTS
            .iter()
            .any(|root| target_module == root.trim_end_matches(".rs"))
        {
            continue;
        }

        if usecase_files.iter().any(|uc| uc == target_module) {
            violations.push(format!(
                "{}:{}: {} — lateral usecase dependency (usecases must be vertical slices)",
                path.display(),
                line_num + 1,
                trimmed,
            ));
        }
    }
    violations
}

#[test]
fn usecases_must_be_vertical_slices() {
    let usecase_path = Path::new(USECASE_DIR);
    assert!(usecase_path.exists(), "usecases directory not found");

    let mut violations = Vec::new();
    for entry in walkdir(usecase_path) {
        if entry.extension().is_some_and(|e| e == "rs") {
            violations.extend(collect_lateral_usecase_violations(&entry));
        }
    }

    assert!(
        violations.is_empty(),
        "Vertical slice violation — usecases must not have lateral dependencies (only composition roots like gateway.rs and registry_service.rs may import sibling usecases):\n\n{}",
        violations.join("\n"),
    );
}

fn walkdir(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            files.extend(walkdir(&path));
        } else {
            files.push(path);
        }
    }
    files.sort();
    files
}
