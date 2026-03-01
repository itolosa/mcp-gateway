use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use predicates::str::contains;

#[test]
fn no_args_succeeds() {
    cargo_bin_cmd!().assert().success();
}

#[test]
fn version_flag_prints_version() {
    cargo_bin_cmd!()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn help_flag_prints_description() {
    cargo_bin_cmd!()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("proxy/firewall"));
}

#[test]
fn unknown_subcommand_fails() {
    cargo_bin_cmd!()
        .arg("nonexistent")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand"));
}

#[test]
fn add_stdio_writes_config_file() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let config_path = dir.path().join("test-config.json");

    cargo_bin_cmd!()
        .args([
            "-c",
            config_path.to_str().unwrap_or_default(),
            "add",
            "my-server",
            "-t",
            "stdio",
            "--command",
            "node",
            "--args",
            "server.js",
        ])
        .assert()
        .success();

    let contents = std::fs::read_to_string(&config_path).unwrap_or_else(|_| unreachable!());
    assert!(contents.contains("my-server"));
    assert!(contents.contains("stdio"));
    assert!(contents.contains("node"));
}

#[test]
fn list_empty_config_prints_nothing() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let config_path = dir.path().join("test-config.json");

    cargo_bin_cmd!()
        .args(["-c", config_path.to_str().unwrap_or_default(), "list"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
}

#[test]
fn list_after_add_shows_server() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let config_path = dir.path().join("test-config.json");
    let config_str = config_path.to_str().unwrap_or_default();

    cargo_bin_cmd!()
        .args([
            "-c",
            config_str,
            "add",
            "my-server",
            "-t",
            "stdio",
            "--command",
            "node",
        ])
        .assert()
        .success();

    cargo_bin_cmd!()
        .args(["-c", config_str, "list"])
        .assert()
        .success()
        .stdout(contains("my-server"))
        .stdout(contains("stdio"))
        .stdout(contains("node"));
}

#[test]
fn list_shows_http_server() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let config_path = dir.path().join("test-config.json");
    let config_str = config_path.to_str().unwrap_or_default();

    cargo_bin_cmd!()
        .args([
            "-c",
            config_str,
            "add",
            "remote",
            "-t",
            "http",
            "--url",
            "https://example.com/mcp",
        ])
        .assert()
        .success();

    cargo_bin_cmd!()
        .args(["-c", config_str, "list"])
        .assert()
        .success()
        .stdout(contains("remote"))
        .stdout(contains("http"))
        .stdout(contains("https://example.com/mcp"));
}

#[test]
fn add_duplicate_fails() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let config_path = dir.path().join("test-config.json");
    let config_str = config_path.to_str().unwrap_or_default();

    cargo_bin_cmd!()
        .args([
            "-c",
            config_str,
            "add",
            "dup",
            "-t",
            "stdio",
            "--command",
            "echo",
        ])
        .assert()
        .success();

    cargo_bin_cmd!()
        .args([
            "-c",
            config_str,
            "add",
            "dup",
            "-t",
            "stdio",
            "--command",
            "echo",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}
