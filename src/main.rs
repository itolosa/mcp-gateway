use clap::Parser;
use rmcp::ServiceExt;

use mcp_gateway::cli::command::{AllowlistAction, Cli, Command, DenylistAction};
use mcp_gateway::cli::runner::{
    run_add, run_allowlist_add, run_allowlist_remove, run_allowlist_show, run_denylist_add,
    run_denylist_remove, run_denylist_show, run_list, run_remove, run_run,
};
use mcp_gateway::config::default_config_path;
use mcp_gateway::config::store::FileConfigStore;
use mcp_gateway::filter::{AllowlistFilter, CompoundFilter, DenylistFilter};
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
        Some(Command::Allowlist(args)) => match args.action {
            AllowlistAction::Add(modify_args) => {
                run_allowlist_add(&registry, modify_args).map_err(|e| e.to_string())
            }
            AllowlistAction::Remove(modify_args) => {
                run_allowlist_remove(&registry, modify_args).map_err(|e| e.to_string())
            }
            AllowlistAction::Show(show_args) => {
                run_allowlist_show(&registry, show_args, &mut std::io::stdout())
                    .map_err(|e| e.to_string())
            }
        },
        Some(Command::Denylist(args)) => match args.action {
            DenylistAction::Add(modify_args) => {
                run_denylist_add(&registry, modify_args).map_err(|e| e.to_string())
            }
            DenylistAction::Remove(modify_args) => {
                run_denylist_remove(&registry, modify_args).map_err(|e| e.to_string())
            }
            DenylistAction::Show(show_args) => {
                run_denylist_show(&registry, show_args, &mut std::io::stdout())
                    .map_err(|e| e.to_string())
            }
        },
        Some(Command::Run(args)) => run_run(&registry, args, |entry| async move {
            let filter = CompoundFilter::new(
                AllowlistFilter::new(entry.allowed_tools().to_vec()),
                DenylistFilter::new(entry.denied_tools().to_vec()),
            );
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
