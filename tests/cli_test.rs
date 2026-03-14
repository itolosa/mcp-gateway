#![allow(clippy::cognitive_complexity)]
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

#[test]
fn denylist_add_show_remove_lifecycle() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let config_path = dir.path().join("denylist.json");
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

    // Show empty denylist
    cargo_bin_cmd!()
        .args(["-c", c, "denylist", "show", "my-server"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    // Add tools to denylist
    cargo_bin_cmd!()
        .args(["-c", c, "denylist", "add", "my-server", "delete", "exec"])
        .assert()
        .success();

    // Show denylist contains the added tools
    cargo_bin_cmd!()
        .args(["-c", c, "denylist", "show", "my-server"])
        .assert()
        .success()
        .stdout(contains("delete"))
        .stdout(contains("exec"));

    // Verify config file contains deniedTools
    let contents = std::fs::read_to_string(&config_path).unwrap_or_else(|_| unreachable!());
    assert!(contents.contains("deniedTools"));
    assert!(contents.contains("delete"));
    assert!(contents.contains("exec"));

    // Remove one tool
    cargo_bin_cmd!()
        .args(["-c", c, "denylist", "remove", "my-server", "delete"])
        .assert()
        .success();

    // Show only remaining tool
    let show_output = cargo_bin_cmd!()
        .args(["-c", c, "denylist", "show", "my-server"])
        .output()
        .unwrap_or_else(|_| unreachable!());
    let stdout = String::from_utf8_lossy(&show_output.stdout);
    assert!(stdout.contains("exec"));
    assert!(!stdout.contains("delete"));
}

#[test]
fn denylist_show_nonexistent_server_fails() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let config_path = dir.path().join("test-config.json");

    cargo_bin_cmd!()
        .args([
            "-c",
            config_path.to_str().unwrap_or_default(),
            "denylist",
            "show",
            "nope",
        ])
        .assert()
        .failure()
        .stderr(contains("not found"));
}

#[test]
fn denylist_add_nonexistent_server_fails() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let config_path = dir.path().join("test-config.json");

    cargo_bin_cmd!()
        .args([
            "-c",
            config_path.to_str().unwrap_or_default(),
            "denylist",
            "add",
            "nope",
            "delete",
        ])
        .assert()
        .failure()
        .stderr(contains("not found"));
}

#[test]
fn stop_when_not_running_prints_error() {
    // Use a custom PID path via HOME override so it doesn't conflict with real state
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    cargo_bin_cmd!()
        .env("HOME", dir.path())
        .arg("stop")
        .assert()
        .failure()
        .stderr(contains("no gateway instance found"));
}

#[test]
fn status_when_not_running_prints_no_instances() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    cargo_bin_cmd!()
        .env("HOME", dir.path())
        .arg("status")
        .assert()
        .success()
        .stderr(contains("no instances running"));
}

#[test]
fn attach_when_not_running_prints_error() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    cargo_bin_cmd!()
        .env("HOME", dir.path())
        .arg("attach")
        .assert()
        .failure()
        .stderr(contains("no gateway instance found"));
}

#[test]
fn run_default_transport_is_accepted() {
    // `run` with no --transport should be accepted (defaults to stdio)
    // We can't actually run it (it blocks on stdin), but we can verify --help shows transport
    cargo_bin_cmd!()
        .args(["run", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("transport"));
}

#[test]
fn run_transport_http_is_accepted() {
    cargo_bin_cmd!()
        .args(["run", "--transport", "http", "--help"])
        .assert()
        .success();
}

#[test]
fn run_rejects_invalid_transport() {
    cargo_bin_cmd!()
        .args(["run", "--transport", "sse"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value"));
}

#[test]
fn start_rejects_stdio_transport() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    cargo_bin_cmd!()
        .env("HOME", dir.path())
        .args(["start", "--transport", "stdio"])
        .assert()
        .failure()
        .stderr(contains("only supports --transport http"));
}

#[test]
fn start_accepts_explicit_http_transport() {
    // start --transport http should be accepted (it will fail because port is in use or
    // no config, but the transport validation itself should pass)
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let config_path = dir.path().join("test-config.json");
    let output = cargo_bin_cmd!()
        .env("HOME", dir.path())
        .args([
            "-c",
            config_path.to_str().unwrap_or_default(),
            "start",
            "--transport",
            "http",
        ])
        .output()
        .unwrap_or_else(|_| unreachable!());
    // Should not fail with "only supports --transport http"
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("only supports --transport http"),
        "should accept --transport http"
    );
}

#[test]
fn oauth_login_no_server_with_empty_config() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let config_path = dir.path().join("test-config.json");

    // oauth login with empty config should succeed (nothing to authenticate)
    cargo_bin_cmd!()
        .args([
            "-c",
            config_path.to_str().unwrap_or_default(),
            "oauth",
            "login",
        ])
        .assert()
        .success();
}

#[test]
fn oauth_login_nonexistent_server_fails() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let config_path = dir.path().join("test-config.json");

    cargo_bin_cmd!()
        .args([
            "-c",
            config_path.to_str().unwrap_or_default(),
            "oauth",
            "login",
            "nope",
        ])
        .assert()
        .failure()
        .stderr(contains("not found"));
}

#[test]
fn oauth_clear_specific_server_no_creds() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    cargo_bin_cmd!()
        .env("HOME", dir.path())
        .args(["oauth", "clear", "nonexistent-server"])
        .assert()
        .success()
        .stderr(contains("no credentials found"));
}

#[test]
fn oauth_clear_all_force_no_creds() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    cargo_bin_cmd!()
        .env("HOME", dir.path())
        .args(["oauth", "clear", "--force"])
        .assert()
        .success()
        .stderr(contains("no stored credentials"));
}

#[test]
fn oauth_clear_specific_server_removes_file() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    // Create a fake credentials file
    let creds_dir = dir.path().join(".mcp-gateway").join("credentials");
    std::fs::create_dir_all(&creds_dir).unwrap_or_else(|_| unreachable!());
    let creds_file = creds_dir.join("my-server.json");
    std::fs::write(&creds_file, "{}").unwrap_or_else(|_| unreachable!());
    assert!(creds_file.exists());

    cargo_bin_cmd!()
        .env("HOME", dir.path())
        .args(["oauth", "clear", "my-server"])
        .assert()
        .success()
        .stderr(contains("cleared credentials"));

    assert!(!creds_file.exists());
}

#[test]
fn oauth_clear_all_force_removes_dir() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let creds_dir = dir.path().join(".mcp-gateway").join("credentials");
    std::fs::create_dir_all(&creds_dir).unwrap_or_else(|_| unreachable!());
    std::fs::write(creds_dir.join("a.json"), "{}").unwrap_or_else(|_| unreachable!());
    std::fs::write(creds_dir.join("b.json"), "{}").unwrap_or_else(|_| unreachable!());

    cargo_bin_cmd!()
        .env("HOME", dir.path())
        .args(["oauth", "clear", "--force"])
        .assert()
        .success()
        .stderr(contains("cleared all"));

    assert!(!creds_dir.exists());
}

#[test]
fn status_when_stale_pid_prints_no_instances() {
    let dir = tempfile::tempdir().unwrap_or_else(|_| unreachable!());
    let run_dir = dir.path().join(".mcp-gateway").join("run");
    std::fs::create_dir_all(&run_dir).unwrap_or_else(|_| unreachable!());
    // Use i32::MAX+1 instead of u32::MAX: on Linux u32::MAX wraps to pid_t -1,
    // and kill(-1, sig) targets every process — dangerous under mutation testing.
    let stale_pid: u32 = i32::MAX as u32 + 1;
    let instance_path = run_dir.join(format!("{stale_pid}.json"));
    let json = format!(r#"{{"pid":{stale_pid},"transport":"http","port":8080}}"#);
    std::fs::write(&instance_path, json).unwrap_or_else(|_| unreachable!());

    cargo_bin_cmd!()
        .env("HOME", dir.path())
        .arg("status")
        .assert()
        .success()
        .stderr(contains("no instances running"));
}
