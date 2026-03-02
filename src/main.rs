use clap::Parser;
use rmcp::ServiceExt;

use mcp_gateway::cli::command::{Cli, Command};
use mcp_gateway::cli::runner::{run_add, run_list, run_remove, run_run};
use mcp_gateway::config::default_config_path;
use mcp_gateway::config::store::FileConfigStore;
use mcp_gateway::filter::AllowlistFilter;
use mcp_gateway::proxy::error::ProxyError;
use mcp_gateway::registry::service::RegistryService;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let config_path = cli.config.or_else(default_config_path).unwrap_or_default();
    let store = FileConfigStore::new(&config_path);
    let registry = RegistryService::new(store);

    let result = match cli.command {
        None => Ok(()),
        Some(Command::Add(args)) => run_add(&registry, args).map_err(|e| e.to_string()),
        Some(Command::List) => {
            run_list(&registry, &mut std::io::stdout()).map_err(|e| e.to_string())
        }
        Some(Command::Remove(args)) => run_remove(&registry, args).map_err(|e| e.to_string()),
        Some(Command::Run(args)) => run_run(&registry, args, |entry| async move {
            let filter = AllowlistFilter::new(entry.allowed_tools().to_vec());
            match entry {
                mcp_gateway::config::model::McpServerEntry::Stdio(config) => {
                    let transport = mcp_gateway::proxy::runner::spawn_transport(&config)?;
                    let upstream =
                        ().serve(transport)
                            .await
                            .map_err(|e| ProxyError::UpstreamInit {
                                message: e.to_string(),
                            })?;
                    mcp_gateway::proxy::runner::serve_proxy(
                        upstream,
                        rmcp::transport::io::stdio(),
                        filter,
                    )
                    .await
                }
                mcp_gateway::config::model::McpServerEntry::Http(config) => {
                    let transport = mcp_gateway::proxy::runner::create_http_transport(&config)?;
                    let upstream =
                        ().serve(transport)
                            .await
                            .map_err(|e| ProxyError::UpstreamInit {
                                message: e.to_string(),
                            })?;
                    mcp_gateway::proxy::runner::serve_proxy(
                        upstream,
                        rmcp::transport::io::stdio(),
                        filter,
                    )
                    .await
                }
            }
        })
        .await
        .map_err(|e| e.to_string()),
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
