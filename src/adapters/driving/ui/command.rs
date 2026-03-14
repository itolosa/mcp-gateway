use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// A proxy/firewall for Model Context Protocol (MCP) servers.
#[derive(Debug, Parser)]
#[command(version = option_env!("MCP_GATEWAY_VERSION").unwrap_or(env!("CARGO_PKG_VERSION")))]
pub struct Cli {
    /// Path to the configuration file
    #[arg(short, long, global = true)]
    pub config: Option<PathBuf>,

    /// Show verbose output (upstream logs, child stderr)
    #[arg(short, long, global = true)]
    pub verbose: bool,

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
    /// Start the gateway proxy in the foreground
    Run(RunArgs),
    /// Start the gateway as a background daemon
    Start(StartArgs),
    /// Stop a running gateway instance
    Stop(StopArgs),
    /// Show the status of running gateway instances
    Status(StatusArgs),
    /// Restart the gateway daemon
    Restart(StartArgs),
    /// Attach to a running gateway daemon and stream logs
    Attach(AttachArgs),
    /// Show logs from a running gateway instance
    Logs(LogsArgs),
    /// Manage OAuth authentication for upstream servers
    Oauth(OAuthArgs),
    /// Show policy rules configured for servers
    Rules(RulesArgs),
    /// List tools exposed by upstream providers (live query)
    Tools(ToolsArgs),
}

#[derive(Debug, Parser)]
pub struct StopArgs {
    /// Port of the instance to stop (prompts if multiple running)
    #[arg(long, short)]
    pub port: Option<u16>,
    /// Stop all running instances
    #[arg(long, short)]
    pub all: bool,
}

#[derive(Debug, Parser)]
pub struct StatusArgs {
    /// Port of the instance to inspect (prompts if multiple running)
    #[arg(long, short)]
    pub port: Option<u16>,
}

#[derive(Debug, Parser)]
pub struct AttachArgs {
    /// Port to connect to (prompts if multiple running)
    #[arg(long, short)]
    pub port: Option<u16>,
}

#[derive(Debug, Parser)]
pub struct LogsArgs {
    /// Port of the instance to read logs from (prompts if multiple running)
    #[arg(long, short)]
    pub port: Option<u16>,
    /// Follow log output (like tail -f)
    #[arg(long, short)]
    pub follow: bool,
}

#[derive(Debug, Parser)]
pub struct RunArgs {
    /// Downstream transport protocol
    #[arg(long, short, default_value = "stdio")]
    pub transport: DownstreamTransport,
    /// Port for HTTP transport
    #[arg(long, short, default_value_t = 8080)]
    pub port: u16,
}

#[derive(Debug, Parser)]
pub struct StartArgs {
    /// Downstream transport protocol (only http is supported for daemon mode)
    #[arg(long, short = 'T', default_value = "http")]
    pub transport: DownstreamTransport,
    /// Port for HTTP transport
    #[arg(long, short, default_value_t = 8080)]
    pub port: u16,
    /// Run in foreground (used internally by daemon launcher)
    #[arg(long, hide = true)]
    pub foreground: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, ValueEnum)]
pub enum DownstreamTransport {
    Stdio,
    Http,
}

#[derive(Debug, Parser)]
pub struct OAuthArgs {
    #[command(subcommand)]
    pub action: OAuthAction,
}

#[derive(Debug, Subcommand)]
pub enum OAuthAction {
    /// Run OAuth authentication for servers missing credentials
    Login(OAuthLoginArgs),
    /// Clear stored OAuth credentials
    Clear(OAuthClearArgs),
}

#[derive(Debug, Parser)]
pub struct OAuthLoginArgs {
    /// Name of a specific server to authenticate (authenticates all if omitted)
    pub name: Option<String>,
}

#[derive(Debug, Parser)]
pub struct OAuthClearArgs {
    /// Name of the server to clear credentials for (clears all if omitted)
    pub name: Option<String>,
    /// Skip confirmation prompt when clearing all credentials
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Parser)]
pub struct RulesArgs {
    /// Name of a specific server to inspect (shows all if omitted)
    pub name: Option<String>,
}

#[derive(Debug, Parser)]
pub struct ToolsArgs {
    /// Name of a specific server to inspect (shows all if omitted)
    pub name: Option<String>,
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
