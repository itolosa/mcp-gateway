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

#[test]
fn remove_existing_server_succeeds() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let config_path = dir.path().join("test-config.json");
    let config_str = config_path.to_str().unwrap_or_default();

    cargo_bin_cmd!()
        .args([
            "-c",
            config_str,
            "add",
            "to-remove",
            "-t",
            "stdio",
            "--command",
            "echo",
        ])
        .assert()
        .success();

    cargo_bin_cmd!()
        .args(["-c", config_str, "remove", "to-remove"])
        .assert()
        .success();

    cargo_bin_cmd!()
        .args(["-c", config_str, "list"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
}

#[test]
fn full_lifecycle_add_list_remove() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let config_path = dir.path().join("lifecycle.json");
    let c = config_path.to_str().unwrap_or_default();

    // Add stdio server with env vars
    cargo_bin_cmd!()
        .args([
            "-c",
            c,
            "add",
            "local",
            "-t",
            "stdio",
            "--command",
            "node",
            "--args",
            "server.js",
            "--env",
            "API_KEY=secret",
        ])
        .assert()
        .success();

    // List shows only local
    cargo_bin_cmd!()
        .args(["-c", c, "list"])
        .assert()
        .success()
        .stdout(contains("local"))
        .stdout(contains("stdio"))
        .stdout(contains("node"));

    // Add http server with headers
    cargo_bin_cmd!()
        .args([
            "-c",
            c,
            "add",
            "remote",
            "-t",
            "http",
            "--url",
            "https://api.example.com/mcp",
            "--header",
            "Authorization: Bearer tok",
        ])
        .assert()
        .success();

    // List shows both
    let list_both = cargo_bin_cmd!().args(["-c", c, "list"]).assert().success();
    list_both
        .stdout(contains("local"))
        .stdout(contains("remote"))
        .stdout(contains("stdio"))
        .stdout(contains("http"));

    // Config file contains env vars and headers
    let contents = std::fs::read_to_string(&config_path).unwrap_or_else(|_| unreachable!());
    assert!(contents.contains("API_KEY"));
    assert!(contents.contains("secret"));
    assert!(contents.contains("Authorization"));
    assert!(contents.contains("Bearer tok"));

    // Remove local
    cargo_bin_cmd!()
        .args(["-c", c, "remove", "local"])
        .assert()
        .success();

    // List shows only remote
    cargo_bin_cmd!()
        .args(["-c", c, "list"])
        .assert()
        .success()
        .stdout(contains("remote"))
        .stdout(contains("https://api.example.com/mcp"));

    // Verify local is gone from list output
    let final_list = cargo_bin_cmd!()
        .args(["-c", c, "list"])
        .output()
        .unwrap_or_else(|_| unreachable!());
    let stdout = String::from_utf8_lossy(&final_list.stdout);
    assert!(!stdout.contains("local"));
}

#[test]
fn remove_nonexistent_fails() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let config_path = dir.path().join("test-config.json");

    cargo_bin_cmd!()
        .args([
            "-c",
            config_path.to_str().unwrap_or_default(),
            "remove",
            "nope",
        ])
        .assert()
        .failure()
        .stderr(contains("not found"));
}

#[test]
fn allowlist_add_show_remove_lifecycle() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let config_path = dir.path().join("allowlist.json");
    let c = config_path.to_str().unwrap_or_default();

    // Add a server first
    cargo_bin_cmd!()
        .args([
            "-c",
            c,
            "add",
            "my-server",
            "-t",
            "stdio",
            "--command",
            "echo",
        ])
        .assert()
        .success();

    // Show empty allowlist
    cargo_bin_cmd!()
        .args(["-c", c, "allowlist", "show", "my-server"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    // Add tools to allowlist
    cargo_bin_cmd!()
        .args(["-c", c, "allowlist", "add", "my-server", "read", "write"])
        .assert()
        .success();

    // Show allowlist contains the added tools
    cargo_bin_cmd!()
        .args(["-c", c, "allowlist", "show", "my-server"])
        .assert()
        .success()
        .stdout(contains("read"))
        .stdout(contains("write"));

    // Verify config file contains allowedTools
    let contents = std::fs::read_to_string(&config_path).unwrap_or_else(|_| unreachable!());
    assert!(contents.contains("allowedTools"));
    assert!(contents.contains("read"));
    assert!(contents.contains("write"));

    // Remove one tool
    cargo_bin_cmd!()
        .args(["-c", c, "allowlist", "remove", "my-server", "read"])
        .assert()
        .success();

    // Show only remaining tool
    let show_output = cargo_bin_cmd!()
        .args(["-c", c, "allowlist", "show", "my-server"])
        .output()
        .unwrap_or_else(|_| unreachable!());
    let stdout = String::from_utf8_lossy(&show_output.stdout);
    assert!(stdout.contains("write"));
    assert!(!stdout.contains("read"));
}

#[test]
fn allowlist_show_nonexistent_server_fails() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let config_path = dir.path().join("test-config.json");

    cargo_bin_cmd!()
        .args([
            "-c",
            config_path.to_str().unwrap_or_default(),
            "allowlist",
            "show",
            "nope",
        ])
        .assert()
        .failure()
        .stderr(contains("not found"));
}

#[test]
fn allowlist_add_nonexistent_server_fails() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let config_path = dir.path().join("test-config.json");

    cargo_bin_cmd!()
        .args([
            "-c",
            config_path.to_str().unwrap_or_default(),
            "allowlist",
            "add",
            "nope",
            "read",
        ])
        .assert()
        .failure()
        .stderr(contains("not found"));
}
