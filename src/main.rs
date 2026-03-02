use std::collections::BTreeMap;

use clap::Parser;
use rmcp::ServiceExt;

use mcp_gateway::cli::command::{AllowlistAction, Cli, Command, DenylistAction};
use mcp_gateway::cli::runner::{
    run_add, run_allowlist_add, run_allowlist_remove, run_allowlist_show, run_denylist_add,
    run_denylist_remove, run_denylist_show, run_list, run_remove, run_run,
};
use mcp_gateway::cli_tools::CliToolExecutor;
use mcp_gateway::config::default_config_path;
use mcp_gateway::config::model::McpServerEntry;
use mcp_gateway::config::store::{ConfigStore, FileConfigStore};
use mcp_gateway::filter::{AllowlistFilter, CompoundFilter, DenylistFilter};
use mcp_gateway::proxy::error::ProxyError;
use mcp_gateway::proxy::handler::{ProxyHandler, UpstreamEntry};
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
        Some(Command::Run) => {
            let gateway_config = registry.store().load().map_err(|e| e.to_string());
            match gateway_config {
                Err(e) => Err(e),
                Ok(gw) => {
                    let cli_tools = if gw.cli_tools.is_empty() {
                        None
                    } else {
                        Some(CliToolExecutor::new(gw.cli_tools))
                    };
                    run_run(&registry, |servers| async move {
                        let mut upstreams = BTreeMap::new();
                        for (name, entry) in servers {
                            let filter = CompoundFilter::new(
                                AllowlistFilter::new(entry.allowed_tools().to_vec()),
                                DenylistFilter::new(entry.denied_tools().to_vec()),
                            );
                            let service = connect_upstream(&name, entry).await?;
                            upstreams.insert(name, UpstreamEntry { service, filter });
                        }
                        let handler = ProxyHandler::new(upstreams, cli_tools);
                        mcp_gateway::proxy::runner::serve_proxy(
                            handler,
                            rmcp::transport::io::stdio(),
                        )
                        .await
                    })
                    .await
                    .map_err(|e| e.to_string())
                }
            }
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

async fn connect_upstream(
    name: &str,
    entry: McpServerEntry,
) -> Result<rmcp::service::RunningService<rmcp::RoleClient, ()>, ProxyError> {
    match entry {
        McpServerEntry::Stdio(config) => {
            let transport = mcp_gateway::proxy::runner::spawn_transport(&config)?;
            ().serve(transport)
                .await
                .map_err(|e| ProxyError::UpstreamInit {
                    message: format!("{name}: {e}"),
                })
        }
        McpServerEntry::Http(ref config) if config.auth.is_some() => {
            let transport =
                mcp_gateway::proxy::runner::create_oauth_http_transport(config, name).await?;
            ().serve(transport)
                .await
                .map_err(|e| ProxyError::UpstreamInit {
                    message: format!("{name}: {e}"),
                })
        }
        McpServerEntry::Http(config) => {
            let transport = mcp_gateway::proxy::runner::create_http_transport(&config)?;
            ().serve(transport)
                .await
                .map_err(|e| ProxyError::UpstreamInit {
                    message: format!("{name}: {e}"),
                })
        }
    }
}
