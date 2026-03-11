use std::collections::BTreeMap;
use std::sync::Arc;

use clap::Parser;
use rmcp::ServiceExt;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::EnvFilter;

use mcp_gateway::adapters::driven::configuration::default_config_path;
use mcp_gateway::adapters::driven::configuration::model::McpServerEntry;
use mcp_gateway::adapters::driven::connectivity::cli_execution::{NullCliRunner, ProcessCliRunner};
use mcp_gateway::adapters::driven::connectivity::mcp_protocol::error::ProxyError;
use mcp_gateway::adapters::driven::connectivity::mcp_protocol::proxy::{
    serve_proxy, serve_proxy_http,
};
use mcp_gateway::adapters::driven::connectivity::mcp_protocol::{McpAdapter, RmcpUpstreamClient};
use mcp_gateway::adapters::driven::storage::{ConfigStore, FileConfigStore};
use mcp_gateway::adapters::driving::execution::process::log_broadcast::BroadcastLayer;
use mcp_gateway::adapters::driving::execution::process::pid;
use mcp_gateway::adapters::driving::ui::command::{
    AllowlistAction, Cli, Command, DenylistAction, DownstreamTransport,
};
use mcp_gateway::adapters::driving::ui::runner::{
    run_add, run_allowlist_add, run_allowlist_remove, run_allowlist_show, run_denylist_add,
    run_denylist_remove, run_denylist_show, run_list, run_remove, run_run,
};
use mcp_gateway::hexagon::entities::policy::allowlist::AllowlistFilter;
use mcp_gateway::hexagon::entities::policy::compound::CompoundFilter;
use mcp_gateway::hexagon::entities::policy::denylist::DenylistFilter;
use mcp_gateway::hexagon::entities::policy::DefaultFilter;
use mcp_gateway::hexagon::ports::ServerConfigStore;
use mcp_gateway::hexagon::usecases::gateway::{Gateway, UpstreamEntry};
use mcp_gateway::hexagon::usecases::registry_service::RegistryService;

#[tokio::main]
async fn main() {
    let (log_sender, _) = broadcast::channel::<String>(1024);

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt_layer = tracing_subscriber::fmt::layer().with_writer(std::io::stderr);
    let broadcast_layer = BroadcastLayer::new(log_sender.clone());
    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .with(broadcast_layer);
    tracing::subscriber::set_global_default(subscriber)
        .unwrap_or_else(|_| eprintln!("failed to set tracing subscriber"));

    let cli = Cli::parse();

    let config_path = cli.config.or_else(default_config_path).unwrap_or_default();
    let store = FileConfigStore::new(&config_path);
    let registry = RegistryService::new(store);

    let result = dispatch_command(cli.command, &registry, &config_path, log_sender).await;

    if let Err(e) = result {
        print_error_and_exit(&e);
    }
}

async fn dispatch_command<S: ServerConfigStore<Entry = McpServerEntry> + ConfigStore>(
    command: Option<Command>,
    registry: &RegistryService<S>,
    config_path: &std::path::Path,
    log_sender: broadcast::Sender<String>,
) -> Result<(), String> {
    match command {
        None => {
            use clap::CommandFactory;
            Cli::command().print_help().ok();
            Ok(())
        }
        Some(Command::Add(args)) => run_add(registry, args).map_err(|e| e.to_string()),
        Some(Command::List) => {
            run_list(registry, &mut std::io::stdout()).map_err(|e| e.to_string())
        }
        Some(Command::Remove(args)) => run_remove(registry, args).map_err(|e| e.to_string()),
        Some(Command::Allowlist(args)) => {
            dispatch_allowlist(registry, args.action).map_err(|e| e.to_string())
        }
        Some(Command::Denylist(args)) => {
            dispatch_denylist(registry, args.action).map_err(|e| e.to_string())
        }
        Some(Command::Run(args)) => run_gateway(registry, args.transport, args.port, log_sender)
            .await
            .map_err(|e| e.to_string()),
        Some(Command::Start(args)) => dispatch_start(registry, config_path, args, log_sender)
            .await
            .map_err(|e| e.to_string()),
        Some(Command::Stop) => dispatch_stop().map_err(|e| e.to_string()),
        Some(Command::Status) => dispatch_status().map_err(|e| e.to_string()),
        Some(Command::Restart(args)) => {
            dispatch_restart(config_path, args.port).map_err(|e| e.to_string())
        }
        Some(Command::Attach(args)) => run_attach(args.port).await.map_err(|e| e.to_string()),
        Some(Command::Oauth(args)) => dispatch_oauth(registry, args)
            .await
            .map_err(|e| e.to_string()),
    }
}

fn dispatch_allowlist<S: ServerConfigStore<Entry = McpServerEntry> + ConfigStore>(
    registry: &RegistryService<S>,
    action: AllowlistAction,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        AllowlistAction::Add(args) => run_allowlist_add(registry, args).map_err(Into::into),
        AllowlistAction::Remove(args) => run_allowlist_remove(registry, args).map_err(Into::into),
        AllowlistAction::Show(args) => {
            run_allowlist_show(registry, args, &mut std::io::stdout()).map_err(Into::into)
        }
    }
}

fn dispatch_denylist<S: ServerConfigStore<Entry = McpServerEntry> + ConfigStore>(
    registry: &RegistryService<S>,
    action: DenylistAction,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        DenylistAction::Add(args) => run_denylist_add(registry, args).map_err(Into::into),
        DenylistAction::Remove(args) => run_denylist_remove(registry, args).map_err(Into::into),
        DenylistAction::Show(args) => {
            run_denylist_show(registry, args, &mut std::io::stdout()).map_err(Into::into)
        }
    }
}

async fn dispatch_start<S: ServerConfigStore<Entry = McpServerEntry> + ConfigStore>(
    registry: &RegistryService<S>,
    config_path: &std::path::Path,
    args: mcp_gateway::adapters::driving::ui::command::StartArgs,
    log_sender: broadcast::Sender<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if args.transport != DownstreamTransport::Http {
        return Err("daemon mode only supports --transport http".into());
    }
    if args.foreground {
        run_foreground_daemon(registry, args.port, log_sender)
            .await
            .map_err(Into::into)
    } else {
        start_daemon(config_path, args.port).map_err(Into::into)
    }
}

fn dispatch_stop(
) -> Result<(), mcp_gateway::adapters::driving::execution::process::error::DaemonError> {
    let pid_path = pid::default_pid_path().unwrap_or_default();
    pid::stop_daemon(&pid_path)?;
    tracing::info!("gateway stopped");
    Ok(())
}

fn dispatch_status(
) -> Result<(), mcp_gateway::adapters::driving::execution::process::error::DaemonError> {
    let pid_path = pid::default_pid_path().unwrap_or_default();
    match pid::daemon_status(&pid_path)? {
        Some(p) => tracing::info!("gateway is running (PID {p})"),
        None => tracing::info!("gateway is not running"),
    }
    Ok(())
}

fn dispatch_restart(
    config_path: &std::path::Path,
    port: u16,
) -> Result<(), mcp_gateway::adapters::driving::execution::process::error::DaemonError> {
    let pid_path = pid::default_pid_path().unwrap_or_default();
    match pid::stop_daemon(&pid_path) {
        Ok(())
        | Err(mcp_gateway::adapters::driving::execution::process::error::DaemonError::NotRunning) =>
            {}
        Err(e) => return Err(e),
    }
    start_daemon(config_path, port)
}

fn print_error_and_exit(message: &str) {
    tracing::error!("{message}");
    std::process::exit(1);
}

async fn run_gateway<S: ServerConfigStore<Entry = McpServerEntry> + ConfigStore>(
    registry: &RegistryService<S>,
    transport: DownstreamTransport,
    port: u16,
    log_sender: broadcast::Sender<String>,
) -> Result<(), ProxyError> {
    let gateway_config = registry.store().load()?;
    let has_cli_tools = !gateway_config.cli_tools.is_empty();
    let cli_tools = gateway_config.cli_tools;
    run_run(registry, |servers| async move {
        let upstreams = build_upstreams(servers).await?;
        if has_cli_tools {
            let cli_runner = ProcessCliRunner::new(cli_tools);
            let gateway = Gateway::new(upstreams, cli_runner);
            let adapter = Arc::new(McpAdapter::new(gateway));
            match transport {
                DownstreamTransport::Stdio => {
                    serve_proxy(adapter, rmcp::transport::io::stdio()).await
                }
                DownstreamTransport::Http => {
                    let ct = CancellationToken::new();
                    serve_proxy_http(adapter, port, ct, log_sender).await
                }
            }
        } else {
            let gateway = Gateway::new(upstreams, NullCliRunner);
            let adapter = Arc::new(McpAdapter::new(gateway));
            match transport {
                DownstreamTransport::Stdio => {
                    serve_proxy(adapter, rmcp::transport::io::stdio()).await
                }
                DownstreamTransport::Http => {
                    let ct = CancellationToken::new();
                    serve_proxy_http(adapter, port, ct, log_sender).await
                }
            }
        }
    })
    .await
}

async fn build_upstreams(
    servers: BTreeMap<String, McpServerEntry>,
) -> Result<BTreeMap<String, UpstreamEntry<RmcpUpstreamClient, DefaultFilter>>, ProxyError> {
    let mut upstreams = BTreeMap::new();
    for (name, entry) in servers {
        let filter = CompoundFilter::new(
            AllowlistFilter::new(entry.allowed_tools().to_vec()),
            DenylistFilter::new(entry.denied_tools().to_vec()),
        );
        match connect_upstream(&name, entry).await {
            Ok(service) => {
                upstreams.insert(
                    name,
                    UpstreamEntry {
                        client: RmcpUpstreamClient::new(service),
                        filter,
                    },
                );
            }
            Err(e) => {
                eprintln!("'{name}' failed to connect: {e}, skipping");
            }
        }
    }
    Ok(upstreams)
}

async fn run_foreground_daemon<S: ServerConfigStore<Entry = McpServerEntry> + ConfigStore>(
    registry: &RegistryService<S>,
    port: u16,
    log_sender: broadcast::Sender<String>,
) -> Result<(), ProxyError> {
    let gateway_config = registry.store().load()?;
    let has_cli_tools = !gateway_config.cli_tools.is_empty();
    let cli_tools = gateway_config.cli_tools;
    run_run(registry, |servers| async move {
        let upstreams = build_upstreams(servers).await?;
        let ct = CancellationToken::new();
        let ct_signal = ct.clone();
        tokio::spawn(async move {
            let mut sigterm =
                match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                    Ok(s) => s,
                    Err(_) => return,
                };
            sigterm.recv().await;
            ct_signal.cancel();
        });
        if has_cli_tools {
            let cli_runner = ProcessCliRunner::new(cli_tools);
            let gateway = Gateway::new(upstreams, cli_runner);
            let adapter = Arc::new(McpAdapter::new(gateway));
            serve_proxy_http(adapter, port, ct, log_sender).await
        } else {
            let gateway = Gateway::new(upstreams, NullCliRunner);
            let adapter = Arc::new(McpAdapter::new(gateway));
            serve_proxy_http(adapter, port, ct, log_sender).await
        }
    })
    .await
}

async fn run_attach(
    port_override: Option<u16>,
) -> Result<(), mcp_gateway::adapters::driving::execution::process::error::DaemonError> {
    let port = match port_override {
        Some(p) => p,
        None => {
            let pid_path = pid::default_pid_path().unwrap_or_default();
            if pid::check_already_running(&pid_path)?.is_none() {
                return Err(mcp_gateway::adapters::driving::execution::process::error::DaemonError::NotRunning);
            }
            let port_path = pid::default_port_path().unwrap_or_default();
            pid::read_port(&port_path)?.ok_or_else(|| {
                mcp_gateway::adapters::driving::execution::process::error::DaemonError::AttachFailed {
                    message: "port file not found".to_string(),
                }
            })?
        }
    };
    mcp_gateway::adapters::driving::execution::process::attach::attach(port, &mut std::io::stdout())
        .await
}

fn start_daemon(
    config_path: &std::path::Path,
    port: u16,
) -> Result<(), mcp_gateway::adapters::driving::execution::process::error::DaemonError> {
    let pid_path = pid::default_pid_path().unwrap_or_default();
    if let Some(existing_pid) = pid::check_already_running(&pid_path)? {
        return Err(
            mcp_gateway::adapters::driving::execution::process::error::DaemonError::AlreadyRunning {
                pid: existing_pid,
            },
        );
    }
    check_port_available(port)?;
    let child = spawn_daemon_process(config_path, port)?;
    pid::write_pid(&pid_path, child.id())?;
    let port_path = pid::default_port_path().unwrap_or_default();
    pid::write_port(&port_path, port)?;
    tracing::info!("gateway started on port {port} (PID {})", child.id());
    Ok(())
}

fn check_port_available(
    port: u16,
) -> Result<(), mcp_gateway::adapters::driving::execution::process::error::DaemonError> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", port)).map_err(|_| {
        mcp_gateway::adapters::driving::execution::process::error::DaemonError::PortInUse { port }
    })?;
    drop(listener);
    Ok(())
}

fn spawn_daemon_process(
    config_path: &std::path::Path,
    port: u16,
) -> Result<
    std::process::Child,
    mcp_gateway::adapters::driving::execution::process::error::DaemonError,
> {
    // Use argv[0] to locate our own executable for re-exec
    let exe = std::env::args_os()
        .next()
        .map(std::path::PathBuf::from)
        .ok_or_else(|| {
            mcp_gateway::adapters::driving::execution::process::error::DaemonError::PidWrite {
                message: "cannot determine executable path: argv[0] is empty".to_string(),
            }
        })?;

    std::process::Command::new(exe)
        .args([
            "-c",
            &config_path.to_string_lossy(),
            "start",
            "--port",
            &port.to_string(),
            "--foreground",
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| {
            mcp_gateway::adapters::driving::execution::process::error::DaemonError::PidWrite {
                message: format!("failed to spawn daemon: {e}"),
            }
        })
}

async fn dispatch_oauth<S: ServerConfigStore<Entry = McpServerEntry> + ConfigStore>(
    registry: &RegistryService<S>,
    args: mcp_gateway::adapters::driving::ui::command::OAuthArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    use mcp_gateway::adapters::driven::connectivity::oauth::FileCredentialStore;
    use mcp_gateway::adapters::driving::ui::command::OAuthAction;

    match args.action {
        OAuthAction::Clear(clear_args) => {
            match clear_args.name {
                Some(name) => {
                    let path = FileCredentialStore::default_path(&name)
                        .ok_or("cannot determine credentials path")?;
                    if path.exists() {
                        std::fs::remove_file(&path)?;
                        tracing::info!("cleared credentials for '{name}'");
                    } else {
                        tracing::info!("no credentials found for '{name}'");
                    }
                }
                None => {
                    if !clear_args.force {
                        eprint!("clear all stored OAuth credentials? [y/N] ");
                        let mut answer = String::new();
                        std::io::stdin().read_line(&mut answer)?;
                        if !answer.trim().eq_ignore_ascii_case("y") {
                            tracing::info!("aborted");
                            return Ok(());
                        }
                    }
                    let creds_dir = dirs::home_dir()
                        .map(|h| h.join(".mcp-gateway").join("credentials"))
                        .ok_or("cannot determine credentials directory")?;
                    if creds_dir.exists() {
                        std::fs::remove_dir_all(&creds_dir)?;
                        tracing::info!("cleared all stored credentials");
                    } else {
                        tracing::info!("no stored credentials found");
                    }
                }
            }
            Ok(())
        }
        OAuthAction::Login(login_args) => {
            let config = registry.store().load()?;
            let servers: Vec<_> = match login_args.name {
                Some(ref name) => {
                    let entry = config
                        .mcp_servers
                        .get(name)
                        .ok_or_else(|| format!("server '{name}' not found"))?;
                    vec![(name.clone(), entry.clone())]
                }
                None => config
                    .mcp_servers
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            };
            for (name, entry) in servers {
                if let McpServerEntry::Http(ref http_config) = entry {
                    let oauth_config = http_config.auth.clone().unwrap_or_default();
                    let cred_path = FileCredentialStore::default_path(&name);
                    let has_creds = cred_path.as_ref().is_some_and(|p| p.exists());
                    if has_creds && login_args.name.is_none() {
                        eprintln!("'{name}' already has stored credentials, skipping");
                        continue;
                    }
                    eprintln!("authenticating '{name}'...");
                    let headers =
                        http_config
                            .headers
                            .iter()
                            .map(|(k, v)| {
                                Ok((
                                    http::HeaderName::from_bytes(k.as_bytes())?,
                                    http::HeaderValue::from_str(v)?,
                                ))
                            })
                            .collect::<Result<
                                std::collections::HashMap<_, _>,
                                Box<dyn std::error::Error>,
                            >>()?;
                    match mcp_gateway::adapters::driven::connectivity::oauth::create_oauth_transport(
                        &http_config.url,
                        &oauth_config,
                        &name,
                        headers,
                    )
                    .await
                    {
                        Ok(_) => eprintln!("'{name}' authenticated successfully"),
                        Err(e) => {
                            if login_args.name.is_some() {
                                return Err(e.into());
                            }
                            eprintln!("'{name}' does not support OAuth: {e}");
                        }
                    }
                }
            }
            Ok(())
        }
    }
}

const UPSTREAM_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

async fn connect_upstream(
    name: &str,
    entry: McpServerEntry,
) -> Result<rmcp::service::RunningService<rmcp::RoleClient, ()>, ProxyError> {
    let fut = connect_upstream_inner(name, entry);
    tokio::time::timeout(UPSTREAM_CONNECT_TIMEOUT, fut)
        .await
        .map_err(|_| ProxyError::UpstreamInit {
            message: format!("{name}: connection timed out after {UPSTREAM_CONNECT_TIMEOUT:?}"),
        })?
}

async fn connect_upstream_inner(
    name: &str,
    entry: McpServerEntry,
) -> Result<rmcp::service::RunningService<rmcp::RoleClient, ()>, ProxyError> {
    match entry {
        McpServerEntry::Stdio(config) => {
            let transport =
                mcp_gateway::adapters::driven::connectivity::mcp_protocol::proxy::spawn_transport(
                    &config,
                )?;
            ().serve(transport)
                .await
                .map_err(|e| ProxyError::UpstreamInit {
                    message: format!("{name}: {e}"),
                })
        }
        McpServerEntry::Http(ref config) => {
            let has_stored_creds =
                mcp_gateway::adapters::driven::connectivity::oauth::FileCredentialStore::default_path(name)
                    .is_some_and(|p: std::path::PathBuf| p.exists());
            if config.auth.is_some() || has_stored_creds {
                let transport =
                    mcp_gateway::adapters::driven::connectivity::mcp_protocol::proxy::create_oauth_http_transport(
                        config, name,
                    )
                    .await?;
                ().serve(transport)
                    .await
                    .map_err(|e| ProxyError::UpstreamInit {
                        message: format!("{name}: {e}"),
                    })
            } else {
                let transport =
                    mcp_gateway::adapters::driven::connectivity::mcp_protocol::proxy::create_http_transport(config)?;
                ().serve(transport)
                    .await
                    .map_err(|e| ProxyError::UpstreamInit {
                        message: format!("{name}: {e}"),
                    })
            }
        }
    }
}
