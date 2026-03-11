#![allow(clippy::unwrap_used)]

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
const ALLOWED_HEXAGON_MODULES: &[&str] = &[
    "crate::hexagon::usecases",
    "crate::hexagon::ports",
    "crate::hexagon::entities",
];

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

        if !import_path.starts_with("crate::hexagon") {
            continue;
        }

        let allowed = ALLOWED_HEXAGON_MODULES
            .iter()
            .any(|prefix| import_path.starts_with(prefix));

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
