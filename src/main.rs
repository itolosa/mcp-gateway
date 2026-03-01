use clap::Parser;

use mcp_gateway::cli::command::{Cli, Command};
use mcp_gateway::cli::runner::run_add;
use mcp_gateway::config::default_config_path;
use mcp_gateway::config::store::FileConfigStore;
use mcp_gateway::registry::service::RegistryService;

fn main() {
    let cli = Cli::parse();

    let config_path = cli.config.or_else(default_config_path).unwrap_or_default();
    let store = FileConfigStore::new(&config_path);
    let registry = RegistryService::new(store);

    let result = match cli.command {
        None => Ok(()),
        Some(Command::Add(args)) => run_add(&registry, args).map_err(|e| e.to_string()),
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
