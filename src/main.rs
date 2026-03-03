use std::collections::BTreeMap;
use std::sync::Arc;

use clap::Parser;
use rmcp::ServiceExt;
use tokio_util::sync::CancellationToken;

use mcp_gateway::cli::command::{AllowlistAction, Cli, Command, DenylistAction};
use mcp_gateway::cli::runner::{
    run_add, run_allowlist_add, run_allowlist_remove, run_allowlist_show, run_denylist_add,
    run_denylist_remove, run_denylist_show, run_list, run_remove, run_run,
};
use mcp_gateway::cli_tools::CliToolExecutor;
use mcp_gateway::config::default_config_path;
use mcp_gateway::config::model::McpServerEntry;
use mcp_gateway::config::store::{ConfigStore, FileConfigStore};
use mcp_gateway::daemon::pid;
use mcp_gateway::filter::{AllowlistFilter, CompoundFilter, DenylistFilter};
use mcp_gateway::proxy::error::ProxyError;
use mcp_gateway::proxy::handler::{ProxyHandler, UpstreamEntry};
use mcp_gateway::proxy::runner::{serve_proxy, serve_proxy_http};
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
        Some(Command::Run(args)) => {
            if !args.stdio && !args.http {
                Err("must specify at least one transport: --stdio and/or --http".to_string())
            } else {
                run_gateway(&registry, args.stdio, args.http, args.port)
                    .await
                    .map_err(|e| e.to_string())
            }
        }
        Some(Command::Start(args)) => {
            if args.foreground {
                run_gateway(&registry, false, true, args.port)
                    .await
                    .map_err(|e| e.to_string())
            } else {
                start_daemon(&config_path, args.port).map_err(|e| e.to_string())
            }
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

async fn run_gateway<S: ConfigStore>(
    registry: &RegistryService<S>,
    stdio: bool,
    http: bool,
    port: u16,
) -> Result<(), ProxyError> {
    let gateway_config = registry.store().load()?;
    let cli_tools = if gateway_config.cli_tools.is_empty() {
        None
    } else {
        Some(CliToolExecutor::new(gateway_config.cli_tools))
    };
    run_run(registry, |servers| async move {
        let handler = build_handler(servers, cli_tools).await?;
        let handler = Arc::new(handler);
        match (stdio, http) {
            (true, true) => {
                let ct = CancellationToken::new();
                let http_handler = Arc::clone(&handler);
                let http_ct = ct.clone();
                let http_task =
                    tokio::spawn(
                        async move { serve_proxy_http(http_handler, port, http_ct).await },
                    );
                let stdio_result = serve_proxy(handler, rmcp::transport::io::stdio()).await;
                ct.cancel();
                let _ = http_task.await;
                stdio_result
            }
            (true, false) => serve_proxy(handler, rmcp::transport::io::stdio()).await,
            (_, true) => {
                let ct = CancellationToken::new();
                serve_proxy_http(handler, port, ct).await
            }
            (false, false) => Ok(()),
        }
    })
    .await
}

async fn build_handler(
    servers: BTreeMap<String, McpServerEntry>,
    cli_tools: Option<CliToolExecutor>,
) -> Result<ProxyHandler, ProxyError> {
    let mut upstreams = BTreeMap::new();
    for (name, entry) in servers {
        let filter = CompoundFilter::new(
            AllowlistFilter::new(entry.allowed_tools().to_vec()),
            DenylistFilter::new(entry.denied_tools().to_vec()),
        );
        let service = connect_upstream(&name, entry).await?;
        upstreams.insert(name, UpstreamEntry { service, filter });
    }
    Ok(ProxyHandler::new(upstreams, cli_tools))
}

fn start_daemon(
    config_path: &std::path::Path,
    port: u16,
) -> Result<(), mcp_gateway::daemon::error::DaemonError> {
    let pid_path = pid::default_pid_path().unwrap_or_default();
    if let Some(existing_pid) = pid::check_already_running(&pid_path)? {
        return Err(mcp_gateway::daemon::error::DaemonError::AlreadyRunning { pid: existing_pid });
    }
    // Port check is sync here — we just try to bind in the parent before spawning
    let std_listener = std::net::TcpListener::bind(("127.0.0.1", port))
        .map_err(|_| mcp_gateway::daemon::error::DaemonError::PortInUse { port })?;
    drop(std_listener);

    let exe =
        std::env::current_exe().map_err(|e| mcp_gateway::daemon::error::DaemonError::PidWrite {
            message: format!("cannot determine executable path: {e}"),
        })?;

    let mut cmd = std::process::Command::new(exe);
    cmd.args([
        "-c",
        &config_path.to_string_lossy(),
        "start",
        "--port",
        &port.to_string(),
        "--foreground",
    ]);
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());

    let child = cmd
        .spawn()
        .map_err(|e| mcp_gateway::daemon::error::DaemonError::PidWrite {
            message: format!("failed to spawn daemon: {e}"),
        })?;

    pid::write_pid(&pid_path, child.id())?;
    eprintln!("Gateway started on port {port} (PID {})", child.id());
    Ok(())
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
