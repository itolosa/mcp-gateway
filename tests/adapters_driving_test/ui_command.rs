// Tests migrated from src/adapters/driving/ui/command.rs

use std::path::PathBuf;

use mcp_gateway::adapters::driving::ui::command::*;

// NOTE: parse_key_value and parse_header are private functions in command.rs.
// Tests that call them directly cannot be migrated to integration tests:
//
// fn parse_key_value_valid()
// fn parse_key_value_invalid()
// fn parse_header_valid()
// fn parse_header_invalid()

use clap::Parser;

#[test]
fn parses_no_arguments() {
    let cli = Cli::try_parse_from(["mcp-gateway"]).unwrap();
    assert!(cli.command.is_none());
    assert!(cli.config.is_none());
}

#[test]
fn parses_config_short_flag() {
    let cli = Cli::try_parse_from(["mcp-gateway", "-c", "/tmp/cfg.json"]).unwrap();
    assert_eq!(cli.config, Some(PathBuf::from("/tmp/cfg.json")));
}

#[test]
fn parses_config_long_flag() {
    let cli = Cli::try_parse_from(["mcp-gateway", "--config", "/tmp/cfg.json"]).unwrap();
    assert_eq!(cli.config, Some(PathBuf::from("/tmp/cfg.json")));
}

#[test]
fn rejects_unknown_subcommand() {
    let result = Cli::try_parse_from(["mcp-gateway", "nonexistent"]);
    assert!(result.is_err());
}

#[test]
fn debug_assert_valid() {
    use clap::CommandFactory;
    Cli::command().debug_assert();
}

#[test]
fn parses_add_stdio() {
    let cli = Cli::try_parse_from([
        "mcp-gateway",
        "add",
        "my-server",
        "-t",
        "stdio",
        "--command",
        "node",
        "--args",
        "server.js",
        "--env",
        "KEY=val",
    ])
    .unwrap();
    assert!(
        matches!(cli.command, Some(Command::Add(ref args)) if args.name == "my-server"
            && matches!(args.transport, TransportType::Stdio)
            && args.command == Some("node".to_string())
            && args.args == vec!["server.js"]
            && args.env_vars == vec![("KEY".to_string(), "val".to_string())])
    );
}

#[test]
fn parses_add_http() {
    let cli = Cli::try_parse_from([
        "mcp-gateway",
        "add",
        "remote",
        "-t",
        "http",
        "--url",
        "https://example.com/mcp",
        "--header",
        "Authorization: Bearer tok",
    ])
    .unwrap();
    assert!(
        matches!(cli.command, Some(Command::Add(ref args)) if args.name == "remote"
            && matches!(args.transport, TransportType::Http)
            && args.url == Some("https://example.com/mcp".to_string())
            && args.headers == vec![("Authorization".to_string(), "Bearer tok".to_string())])
    );
}

#[test]
fn add_stdio_requires_command() {
    let result = Cli::try_parse_from(["mcp-gateway", "add", "my-server", "-t", "stdio"]);
    assert!(result.is_err());
}

#[test]
fn add_http_requires_url() {
    let result = Cli::try_parse_from(["mcp-gateway", "add", "my-server", "-t", "http"]);
    assert!(result.is_err());
}

#[test]
fn parses_remove() {
    let cli = Cli::try_parse_from(["mcp-gateway", "remove", "my-server"]).unwrap();
    assert!(matches!(cli.command, Some(Command::Remove(ref args)) if args.name == "my-server"));
}

#[test]
fn remove_requires_name() {
    let result = Cli::try_parse_from(["mcp-gateway", "remove"]);
    assert!(result.is_err());
}

#[test]
fn run_defaults_to_stdio() {
    let cli = Cli::try_parse_from(["mcp-gateway", "run"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Run(ref args)) if args.transport == DownstreamTransport::Stdio
    ));
}

#[test]
fn parses_run_transport_stdio() {
    let cli = Cli::try_parse_from(["mcp-gateway", "run", "--transport", "stdio"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Run(ref args)) if args.transport == DownstreamTransport::Stdio
    ));
}

#[test]
fn parses_run_transport_http() {
    let cli = Cli::try_parse_from([
        "mcp-gateway",
        "run",
        "--transport",
        "http",
        "--port",
        "3000",
    ])
    .unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Run(ref args)) if args.transport == DownstreamTransport::Http && args.port == 3000
    ));
}

#[test]
fn run_default_port_is_8080() {
    let cli = Cli::try_parse_from(["mcp-gateway", "run", "--transport", "http"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Run(ref args)) if args.port == 8080
    ));
}

#[test]
fn run_rejects_extra_args() {
    let result = Cli::try_parse_from(["mcp-gateway", "run", "my-server"]);
    assert!(result.is_err());
}

#[test]
fn run_rejects_invalid_transport() {
    let result = Cli::try_parse_from(["mcp-gateway", "run", "--transport", "sse"]);
    assert!(result.is_err());
}

#[test]
fn start_defaults_to_http() {
    let cli = Cli::try_parse_from(["mcp-gateway", "start"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Start(ref args)) if args.transport == DownstreamTransport::Http
            && args.port == 8080 && !args.foreground
    ));
}

#[test]
fn parses_start_transport_http() {
    let cli = Cli::try_parse_from(["mcp-gateway", "start", "--transport", "http"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Start(ref args)) if args.transport == DownstreamTransport::Http
    ));
}

#[test]
fn parses_start_with_port() {
    let cli = Cli::try_parse_from(["mcp-gateway", "start", "--port", "9090"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Start(ref args)) if args.port == 9090
    ));
}

#[test]
fn parses_start_foreground() {
    let cli = Cli::try_parse_from(["mcp-gateway", "start", "--foreground"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Start(ref args)) if args.foreground
    ));
}

#[test]
fn parses_stop() {
    let cli = Cli::try_parse_from(["mcp-gateway", "stop"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Stop(ref args)) if args.port.is_none() && !args.all
    ));
}

#[test]
fn parses_stop_with_port() {
    let cli = Cli::try_parse_from(["mcp-gateway", "stop", "--port", "9090"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Stop(ref args)) if args.port == Some(9090) && !args.all
    ));
}

#[test]
fn parses_stop_all() {
    let cli = Cli::try_parse_from(["mcp-gateway", "stop", "--all"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Stop(ref args)) if args.all && args.port.is_none()
    ));
}

#[test]
fn parses_stop_all_short() {
    let cli = Cli::try_parse_from(["mcp-gateway", "stop", "-a"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Stop(ref args)) if args.all
    ));
}

#[test]
fn parses_status() {
    let cli = Cli::try_parse_from(["mcp-gateway", "status"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Status(ref args)) if args.port.is_none()
    ));
}

#[test]
fn parses_status_with_port() {
    let cli = Cli::try_parse_from(["mcp-gateway", "status", "--port", "8080"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Status(ref args)) if args.port == Some(8080)
    ));
}

#[test]
fn parses_restart() {
    let cli = Cli::try_parse_from(["mcp-gateway", "restart"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Restart(ref args)) if args.port == 8080
    ));
}

#[test]
fn parses_restart_with_port() {
    let cli = Cli::try_parse_from(["mcp-gateway", "restart", "--port", "9090"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Restart(ref args)) if args.port == 9090
    ));
}

#[test]
fn parses_allowlist_add() {
    let cli = Cli::try_parse_from([
        "mcp-gateway",
        "allowlist",
        "add",
        "my-server",
        "read",
        "write",
    ])
    .unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Allowlist(AllowlistArgs {
            action: AllowlistAction::Add(ref args),
        })) if args.name == "my-server" && args.tools == vec!["read", "write"]
    ));
}

#[test]
fn parses_allowlist_remove() {
    let cli =
        Cli::try_parse_from(["mcp-gateway", "allowlist", "remove", "my-server", "read"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Allowlist(AllowlistArgs {
            action: AllowlistAction::Remove(ref args),
        })) if args.name == "my-server" && args.tools == vec!["read"]
    ));
}

#[test]
fn parses_allowlist_show() {
    let cli = Cli::try_parse_from(["mcp-gateway", "allowlist", "show", "my-server"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Allowlist(AllowlistArgs {
            action: AllowlistAction::Show(ref args),
        })) if args.name == "my-server"
    ));
}

#[test]
fn allowlist_add_requires_tools() {
    let result = Cli::try_parse_from(["mcp-gateway", "allowlist", "add", "my-server"]);
    assert!(result.is_err());
}

#[test]
fn allowlist_remove_requires_tools() {
    let result = Cli::try_parse_from(["mcp-gateway", "allowlist", "remove", "my-server"]);
    assert!(result.is_err());
}

#[test]
fn parses_denylist_add() {
    let cli = Cli::try_parse_from([
        "mcp-gateway",
        "denylist",
        "add",
        "my-server",
        "delete",
        "exec",
    ])
    .unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Denylist(DenylistArgs {
            action: DenylistAction::Add(ref args),
        })) if args.name == "my-server" && args.tools == vec!["delete", "exec"]
    ));
}

#[test]
fn parses_denylist_remove() {
    let cli =
        Cli::try_parse_from(["mcp-gateway", "denylist", "remove", "my-server", "delete"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Denylist(DenylistArgs {
            action: DenylistAction::Remove(ref args),
        })) if args.name == "my-server" && args.tools == vec!["delete"]
    ));
}

#[test]
fn parses_denylist_show() {
    let cli = Cli::try_parse_from(["mcp-gateway", "denylist", "show", "my-server"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Denylist(DenylistArgs {
            action: DenylistAction::Show(ref args),
        })) if args.name == "my-server"
    ));
}

#[test]
fn denylist_add_requires_tools() {
    let result = Cli::try_parse_from(["mcp-gateway", "denylist", "add", "my-server"]);
    assert!(result.is_err());
}

#[test]
fn denylist_remove_requires_tools() {
    let result = Cli::try_parse_from(["mcp-gateway", "denylist", "remove", "my-server"]);
    assert!(result.is_err());
}

#[test]
fn parses_logs() {
    let cli = Cli::try_parse_from(["mcp-gateway", "logs"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Logs(ref args)) if args.port.is_none() && !args.follow
    ));
}

#[test]
fn parses_logs_with_follow() {
    let cli = Cli::try_parse_from(["mcp-gateway", "logs", "--follow"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Logs(ref args)) if args.follow
    ));
}

#[test]
fn parses_logs_with_follow_short() {
    let cli = Cli::try_parse_from(["mcp-gateway", "logs", "-f"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Logs(ref args)) if args.follow
    ));
}

#[test]
fn parses_logs_with_port() {
    let cli = Cli::try_parse_from(["mcp-gateway", "logs", "--port", "9090"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Logs(ref args)) if args.port == Some(9090)
    ));
}

#[test]
fn parses_attach() {
    let cli = Cli::try_parse_from(["mcp-gateway", "attach"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Attach(ref args)) if args.port.is_none()
    ));
}

#[test]
fn parses_attach_with_port() {
    let cli = Cli::try_parse_from(["mcp-gateway", "attach", "--port", "9090"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Attach(ref args)) if args.port == Some(9090)
    ));
}

#[test]
fn parses_oauth_login_no_server() {
    let cli = Cli::try_parse_from(["mcp-gateway", "oauth", "login"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Oauth(OAuthArgs {
            action: OAuthAction::Login(ref args),
        })) if args.name.is_none()
    ));
}

#[test]
fn parses_oauth_login_with_server() {
    let cli = Cli::try_parse_from(["mcp-gateway", "oauth", "login", "my-server"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Oauth(OAuthArgs {
            action: OAuthAction::Login(ref args),
        })) if args.name.as_deref() == Some("my-server")
    ));
}

#[test]
fn parses_oauth_clear_with_server() {
    let cli = Cli::try_parse_from(["mcp-gateway", "oauth", "clear", "my-server"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Oauth(OAuthArgs {
            action: OAuthAction::Clear(ref args),
        })) if args.name.as_deref() == Some("my-server") && !args.force
    ));
}

#[test]
fn parses_oauth_clear_all() {
    let cli = Cli::try_parse_from(["mcp-gateway", "oauth", "clear"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Oauth(OAuthArgs {
            action: OAuthAction::Clear(ref args),
        })) if args.name.is_none() && !args.force
    ));
}

#[test]
fn parses_oauth_clear_force() {
    let cli = Cli::try_parse_from(["mcp-gateway", "oauth", "clear", "--force"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Oauth(OAuthArgs {
            action: OAuthAction::Clear(ref args),
        })) if args.name.is_none() && args.force
    ));
}

#[test]
fn parses_oauth_clear_server_with_force() {
    let cli =
        Cli::try_parse_from(["mcp-gateway", "oauth", "clear", "my-server", "--force"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Oauth(OAuthArgs {
            action: OAuthAction::Clear(ref args),
        })) if args.name.as_deref() == Some("my-server") && args.force
    ));
}

#[test]
fn parses_rules_no_args() {
    let cli = Cli::try_parse_from(["mcp-gateway", "rules"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Rules(ref args)) if args.name.is_none()
    ));
}

#[test]
fn parses_rules_with_server_name() {
    let cli = Cli::try_parse_from(["mcp-gateway", "rules", "my-server"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Rules(ref args)) if args.name.as_deref() == Some("my-server")
    ));
}

#[test]
fn parses_tools_no_args() {
    let cli = Cli::try_parse_from(["mcp-gateway", "tools"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Tools(ref args)) if args.name.is_none()
    ));
}

#[test]
fn parses_tools_with_server_name() {
    let cli = Cli::try_parse_from(["mcp-gateway", "tools", "my-server"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Tools(ref args)) if args.name.as_deref() == Some("my-server")
    ));
}

#[test]
fn parses_run_transport_short_flag() {
    let cli = Cli::try_parse_from(["mcp-gateway", "run", "-t", "http"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Run(ref args)) if args.transport == DownstreamTransport::Http
    ));
}
