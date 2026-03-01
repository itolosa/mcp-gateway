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
    /// List registered MCP servers
    List,
    /// Remove a registered MCP server
    Remove(RemoveArgs),
    /// Start the proxy for a registered MCP server
    Run(RunArgs),
}

#[derive(Debug, Parser)]
pub struct RunArgs {
    /// Name of the server to proxy
    pub name: String,
}

#[derive(Debug, Parser)]
pub struct RemoveArgs {
    /// Name of the server to remove
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
    fn parses_run() {
        let cli = Cli::try_parse_from(["mcp-gateway", "run", "my-server"]).unwrap();
        assert!(matches!(cli.command, Some(Command::Run(ref args)) if args.name == "my-server"));
    }

    #[test]
    fn run_requires_name() {
        let result = Cli::try_parse_from(["mcp-gateway", "run"]);
        assert!(result.is_err());
    }
}
