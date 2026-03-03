use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// A proxy/firewall for Model Context Protocol (MCP) servers.
#[derive(Debug, Parser)]
#[command(version)]
pub struct Cli {
    /// Path to the configuration file
    #[arg(short, long, global = true)]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Register a new MCP server
    Add(AddArgs),
    /// Manage tool allowlists
    Allowlist(AllowlistArgs),
    /// Manage tool denylists
    Denylist(DenylistArgs),
    /// List registered MCP servers
    List,
    /// Remove a registered MCP server
    Remove(RemoveArgs),
    /// Start the gateway proxy for all registered MCP servers
    Run(RunArgs),
    /// Start the gateway as a background daemon (HTTP only)
    Start(StartArgs),
    /// Stop the running gateway daemon
    Stop,
    /// Show the status of the gateway daemon
    Status,
    /// Restart the gateway daemon
    Restart(StartArgs),
}

#[derive(Debug, Parser)]
pub struct RunArgs {
    /// Enable stdio downstream transport
    #[arg(long)]
    pub stdio: bool,
    /// Enable Streamable HTTP downstream transport
    #[arg(long)]
    pub http: bool,
    /// Port for HTTP transport
    #[arg(long, short, default_value_t = 8080)]
    pub port: u16,
}

#[derive(Debug, Parser)]
pub struct StartArgs {
    /// Port for HTTP transport
    #[arg(long, short, default_value_t = 8080)]
    pub port: u16,
    /// Run in foreground (used internally by daemon launcher)
    #[arg(long, hide = true)]
    pub foreground: bool,
}

#[derive(Debug, Parser)]
pub struct RemoveArgs {
    /// Name of the server to remove
    pub name: String,
}

#[derive(Debug, Parser)]
pub struct AllowlistArgs {
    #[command(subcommand)]
    pub action: AllowlistAction,
}

#[derive(Debug, Subcommand)]
pub enum AllowlistAction {
    /// Add tools to a server's allowlist
    Add(AllowlistModifyArgs),
    /// Remove tools from a server's allowlist
    Remove(AllowlistModifyArgs),
    /// Show a server's current allowlist
    Show(AllowlistShowArgs),
}

#[derive(Debug, Parser)]
pub struct AllowlistModifyArgs {
    /// Name of the server
    pub name: String,
    /// Tool names to add or remove
    #[arg(required = true)]
    pub tools: Vec<String>,
}

#[derive(Debug, Parser)]
pub struct AllowlistShowArgs {
    /// Name of the server
    pub name: String,
}

#[derive(Debug, Parser)]
pub struct DenylistArgs {
    #[command(subcommand)]
    pub action: DenylistAction,
}

#[derive(Debug, Subcommand)]
pub enum DenylistAction {
    /// Add tools to a server's denylist
    Add(DenylistModifyArgs),
    /// Remove tools from a server's denylist
    Remove(DenylistModifyArgs),
    /// Show a server's current denylist
    Show(DenylistShowArgs),
}

#[derive(Debug, Parser)]
pub struct DenylistModifyArgs {
    /// Name of the server
    pub name: String,
    /// Tool names to add or remove
    #[arg(required = true)]
    pub tools: Vec<String>,
}

#[derive(Debug, Parser)]
pub struct DenylistShowArgs {
    /// Name of the server
    pub name: String,
}

#[derive(Debug, Parser)]
pub struct AddArgs {
    /// Name for the server entry
    pub name: String,

    /// Transport type
    #[arg(short, long)]
    pub transport: TransportType,

    /// Command to run (stdio transport)
    #[arg(long, required_if_eq("transport", "stdio"))]
    pub command: Option<String>,

    /// Arguments for the command (stdio transport)
    #[arg(long)]
    pub args: Vec<String>,

    /// Environment variables (KEY=VALUE)
    #[arg(long = "env", value_parser = parse_key_value)]
    pub env_vars: Vec<(String, String)>,

    /// URL for the server (http transport)
    #[arg(long, required_if_eq("transport", "http"))]
    pub url: Option<String>,

    /// HTTP headers (KEY:VALUE)
    #[arg(long = "header", value_parser = parse_header)]
    pub headers: Vec<(String, String)>,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum TransportType {
    Stdio,
    Http,
}

fn parse_key_value(s: &str) -> Result<(String, String), String> {
    s.split_once('=')
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .ok_or_else(|| format!("expected KEY=VALUE, got '{s}'"))
}

fn parse_header(s: &str) -> Result<(String, String), String> {
    s.split_once(':')
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .ok_or_else(|| format!("expected KEY:VALUE, got '{s}'"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

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
    fn parse_key_value_valid() {
        let result = parse_key_value("KEY=value");
        assert_eq!(result, Ok(("KEY".to_string(), "value".to_string())));
    }

    #[test]
    fn parse_key_value_invalid() {
        let result = parse_key_value("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn parse_header_valid() {
        let result = parse_header("Content-Type: application/json");
        assert_eq!(
            result,
            Ok(("Content-Type".to_string(), "application/json".to_string()))
        );
    }

    #[test]
    fn parse_header_invalid() {
        let result = parse_header("novalue");
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
    fn parses_run_stdio() {
        let cli = Cli::try_parse_from(["mcp-gateway", "run", "--stdio"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Run(ref args)) if args.stdio && !args.http
        ));
    }

    #[test]
    fn parses_run_http_with_port() {
        let cli = Cli::try_parse_from(["mcp-gateway", "run", "--http", "--port", "3000"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Run(ref args)) if !args.stdio && args.http && args.port == 3000
        ));
    }

    #[test]
    fn parses_run_stdio_and_http() {
        let cli = Cli::try_parse_from(["mcp-gateway", "run", "--stdio", "--http"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Run(ref args)) if args.stdio && args.http && args.port == 8080
        ));
    }

    #[test]
    fn parses_run_no_flags() {
        let cli = Cli::try_parse_from(["mcp-gateway", "run"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Run(ref args)) if !args.stdio && !args.http
        ));
    }

    #[test]
    fn run_default_port_is_8080() {
        let cli = Cli::try_parse_from(["mcp-gateway", "run", "--http"]).unwrap();
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
    fn parses_start() {
        let cli = Cli::try_parse_from(["mcp-gateway", "start"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Start(ref args)) if args.port == 8080 && !args.foreground
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
        assert!(matches!(cli.command, Some(Command::Stop)));
    }

    #[test]
    fn parses_status() {
        let cli = Cli::try_parse_from(["mcp-gateway", "status"]).unwrap();
        assert!(matches!(cli.command, Some(Command::Status)));
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
        let cli = Cli::try_parse_from(["mcp-gateway", "allowlist", "remove", "my-server", "read"])
            .unwrap();
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
        let cli = Cli::try_parse_from(["mcp-gateway", "denylist", "remove", "my-server", "delete"])
            .unwrap();
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
}
