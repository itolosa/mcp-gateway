#![allow(clippy::cognitive_complexity)]
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
use mcp_gateway::adapters::driven::connectivity::mcp_protocol::{McpAdapter, RmcpProviderClient};
use mcp_gateway::adapters::driven::storage::{ConfigStore, FileConfigStore};
use mcp_gateway::adapters::driving::execution::process::error::DaemonError;
use mcp_gateway::adapters::driving::execution::process::log_broadcast::BroadcastLayer;
use mcp_gateway::adapters::driving::execution::process::log_file;
use mcp_gateway::adapters::driving::execution::process::pid;
use mcp_gateway::adapters::driving::execution::process::status_socket::{
    self, GatewayStatusReport, ProviderStatus,
};
use mcp_gateway::adapters::driving::ui::command::{
    AllowlistAction, Cli, Command, DenylistAction, DownstreamTransport, ToolsArgs,
};
use mcp_gateway::adapters::driving::ui::runner::{
    run_add, run_allowlist_add, run_allowlist_remove, run_allowlist_show, run_denylist_add,
    run_denylist_remove, run_denylist_show, run_list, run_remove, run_rules, run_run, run_tools,
};
use mcp_gateway::hexagon::ports::driven::provider_config_store::ProviderConfigStore;
use mcp_gateway::hexagon::usecases::gateway::{
    create_policy, DefaultPolicy, Gateway, ProviderHandle,
};
use mcp_gateway::hexagon::usecases::registry_service::RegistryService;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let (log_sender, _) = broadcast::channel::<String>(1024);

    let default_filter = if cli.verbose {
        "info"
    } else {
        "mcp_gateway=info"
    };
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));
    let fmt_layer = tracing_subscriber::fmt::layer().with_writer(std::io::stderr);
    let broadcast_layer = BroadcastLayer::new(log_sender.clone());
    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .with(broadcast_layer);
    tracing::subscriber::set_global_default(subscriber)
        .unwrap_or_else(|_| eprintln!("failed to set tracing subscriber"));

    let config_path = cli.config.or_else(default_config_path).unwrap_or_default();
    let store = FileConfigStore::new(&config_path);
    let registry = RegistryService::new(store);

    let result = dispatch_command(
        cli.command,
        cli.verbose,
        &registry,
        &config_path,
        log_sender,
    )
    .await;

    if let Err(e) = result {
        print_error_and_exit(&e);
    }
}

async fn dispatch_command<S: ProviderConfigStore<Entry = McpServerEntry> + ConfigStore>(
    command: Option<Command>,
    verbose: bool,
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
        Some(Command::Run(args)) => {
            run_gateway(registry, args.transport, args.port, verbose, log_sender)
                .await
                .map_err(|e| e.to_string())
        }
        Some(Command::Start(args)) => {
            dispatch_start(registry, config_path, args, verbose, log_sender)
                .await
                .map_err(|e| e.to_string())
        }
        Some(Command::Stop(args)) => dispatch_stop(args.port, args.all).map_err(|e| e.to_string()),
        Some(Command::Status(args)) => dispatch_status(args.port).await.map_err(|e| e.to_string()),
        Some(Command::Restart(args)) => {
            dispatch_restart(config_path, args.port).map_err(|e| e.to_string())
        }
        Some(Command::Attach(args)) => run_attach(args.port).await.map_err(|e| e.to_string()),
        Some(Command::Logs(args)) => dispatch_logs(args).await.map_err(|e| e.to_string()),
        Some(Command::Oauth(args)) => dispatch_oauth(registry, args)
            .await
            .map_err(|e| e.to_string()),
        Some(Command::Rules(args)) => {
            let cli_ops = registry
                .store()
                .load()
                .map(|c| c.cli_operations)
                .unwrap_or_default();
            run_rules(registry, &cli_ops, args, &mut std::io::stdout()).map_err(|e| e.to_string())
        }
        Some(Command::Tools(args)) => dispatch_tools(registry, args, verbose)
            .await
            .map_err(|e| e.to_string()),
    }
}

fn dispatch_allowlist<S: ProviderConfigStore<Entry = McpServerEntry> + ConfigStore>(
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

fn dispatch_denylist<S: ProviderConfigStore<Entry = McpServerEntry> + ConfigStore>(
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

fn check_single_instance(instances: &[pid::InstanceInfo]) -> Result<(), DaemonError> {
    if let Some(first) = instances.first() {
        return Err(DaemonError::AlreadyRunning {
            pid: first.pid,
            port: first.port.unwrap_or(0),
        });
    }
    Ok(())
}

fn resolve_instance(
    port_arg: Option<u16>,
    instances: &[pid::InstanceInfo],
) -> Result<pid::InstanceInfo, DaemonError> {
    match (port_arg, instances) {
        (_, []) => Err(DaemonError::NotRunning),
        (Some(p), _) => instances
            .iter()
            .find(|i| i.port == Some(p))
            .cloned()
            .ok_or(DaemonError::NotRunning),
        (None, [single]) => Ok(single.clone()),
        (None, _) => prompt_select_instance(instances),
    }
}

fn prompt_select_instance(
    instances: &[pid::InstanceInfo],
) -> Result<pid::InstanceInfo, DaemonError> {
    eprintln!("Multiple instances running:");
    for (i, instance) in instances.iter().enumerate() {
        let label = match instance.port {
            Some(p) => format!("{} port {}", instance.transport, p),
            None => instance.transport.clone(),
        };
        eprintln!("  {}) {} (PID {})", i + 1, label, instance.pid);
    }
    eprint!("Select instance [1-{}]: ", instances.len());
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .map_err(|e| DaemonError::UserInput(format!("failed to read input: {e}")))?;
    let choice: usize = input
        .trim()
        .parse()
        .map_err(|_| DaemonError::UserInput("invalid selection".to_string()))?;
    if choice < 1 || choice > instances.len() {
        return Err(DaemonError::UserInput("selection out of range".to_string()));
    }
    Ok(instances
        .get(choice - 1)
        .ok_or_else(|| DaemonError::UserInput("selection out of range".to_string()))?
        .clone())
}

async fn dispatch_start<S: ProviderConfigStore<Entry = McpServerEntry> + ConfigStore>(
    registry: &RegistryService<S>,
    config_path: &std::path::Path,
    args: mcp_gateway::adapters::driving::ui::command::StartArgs,
    verbose: bool,
    log_sender: broadcast::Sender<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if args.transport != DownstreamTransport::Http {
        return Err("daemon mode only supports --transport http".into());
    }
    let gateway_config = registry.store().load()?;
    if gateway_config.single_instance {
        let run_dir = pid::default_run_dir().unwrap_or_default();
        let instances = pid::list_instances(&run_dir)?;
        check_single_instance(&instances)?;
    }
    if args.foreground {
        run_foreground_daemon(registry, args.port, verbose, log_sender)
            .await
            .map_err(Into::into)
    } else {
        start_daemon(config_path, args.port).map_err(Into::into)
    }
}

fn dispatch_stop(port_arg: Option<u16>, all: bool) -> Result<(), DaemonError> {
    let run_dir = pid::default_run_dir().unwrap_or_default();
    let instances = pid::list_instances(&run_dir)?;
    if all {
        if instances.is_empty() {
            return Err(DaemonError::NotRunning);
        }
        eprintln!("Stop all {} running instances?", instances.len());
        for instance in &instances {
            let label = match instance.port {
                Some(p) => format!("{} port {p}", instance.transport),
                None => instance.transport.clone(),
            };
            eprintln!("  {} (PID {})", label, instance.pid);
        }
        eprint!("Confirm [y/N]: ");
        let mut answer = String::new();
        std::io::stdin()
            .read_line(&mut answer)
            .map_err(|e| DaemonError::UserInput(format!("failed to read input: {e}")))?;
        if !answer.trim().eq_ignore_ascii_case("y") {
            tracing::info!("aborted");
            return Ok(());
        }
        for instance in &instances {
            let _ = pid::stop_instance(&run_dir, instance.pid);
            match instance.port {
                Some(p) => tracing::info!("gateway on port {p} stopped"),
                None => tracing::info!("gateway (PID {}) stopped", instance.pid),
            }
        }
        return Ok(());
    }
    let instance = resolve_instance(port_arg, &instances)?;
    pid::stop_instance(&run_dir, instance.pid)?;
    match instance.port {
        Some(p) => tracing::info!("gateway on port {p} stopped"),
        None => tracing::info!("gateway (PID {}) stopped", instance.pid),
    }
    Ok(())
}

async fn dispatch_status(port_arg: Option<u16>) -> Result<(), DaemonError> {
    let run_dir = pid::default_run_dir().unwrap_or_default();
    let instances = pid::list_instances(&run_dir)?;
    if instances.is_empty() {
        tracing::info!("no instances running");
        return Ok(());
    }
    let instance = resolve_instance(port_arg, &instances)?;
    let label = match instance.port {
        Some(p) => format!("{} port {p}", instance.transport),
        None => instance.transport.clone(),
    };
    let sock_path = pid::sock_path(&run_dir, instance.pid);
    match status_socket::query_status(&sock_path).await {
        Ok(report) => {
            tracing::info!("{label} (PID {}) — {}", instance.pid, report.state);
            if report.providers.is_empty() {
                tracing::info!("no providers configured");
            } else {
                for provider in &report.providers {
                    let status = if provider.connected {
                        "connected"
                    } else {
                        "disconnected"
                    };
                    tracing::info!(
                        "  {} ({} {}) — {}",
                        provider.name,
                        provider.provider_type,
                        provider.target,
                        status
                    );
                }
            }
        }
        Err(_) => {
            tracing::info!("{label} (PID {})", instance.pid);
            tracing::info!("provider status unavailable");
        }
    }
    Ok(())
}

fn dispatch_restart(config_path: &std::path::Path, port: u16) -> Result<(), DaemonError> {
    let run_dir = pid::default_run_dir().unwrap_or_default();
    // Find existing instance on this port and stop it
    let instances = pid::list_instances(&run_dir)?;
    if let Some(existing) = instances.iter().find(|i| i.port == Some(port)) {
        let _ = pid::stop_instance(&run_dir, existing.pid);
    }
    start_daemon(config_path, port)
}

fn print_error_and_exit(message: &str) {
    tracing::error!("{message}");
    std::process::exit(1);
}

fn transport_label(transport: &DownstreamTransport) -> &'static str {
    match transport {
        DownstreamTransport::Stdio => "stdio",
        DownstreamTransport::Http => "http",
    }
}

async fn dispatch_tools<S: ProviderConfigStore<Entry = McpServerEntry> + ConfigStore>(
    registry: &RegistryService<S>,
    args: ToolsArgs,
    verbose: bool,
) -> Result<(), ProxyError> {
    run_run(registry, |servers| async move {
        let (upstreams, _statuses) = build_upstreams(servers, verbose).await?;
        run_tools(&upstreams, args.name.as_deref(), &mut std::io::stdout()).await
    })
    .await
}

async fn run_gateway<S: ProviderConfigStore<Entry = McpServerEntry> + ConfigStore>(
    registry: &RegistryService<S>,
    transport: DownstreamTransport,
    port: u16,
    verbose: bool,
    log_sender: broadcast::Sender<String>,
) -> Result<(), ProxyError> {
    let gateway_config = registry.store().load()?;
    if gateway_config.single_instance {
        let run_dir = pid::default_run_dir().unwrap_or_default();
        let instances = pid::list_instances(&run_dir).unwrap_or_default();
        if let Some(first) = instances.first() {
            return Err(ProxyError::UpstreamInit {
                message: format!(
                    "single-instance mode: gateway already running (PID {}), use 'stop' first",
                    first.pid
                ),
            });
        }
    }
    let has_cli_tools = !gateway_config.cli_operations.is_empty();
    let cli_tools = gateway_config.cli_operations;
    let transport_str = transport_label(&transport).to_string();
    let is_http = transport == DownstreamTransport::Http;
    run_run(registry, |servers| async move {
        let run_dir = pid::ensure_run_dir().map_err(|e| ProxyError::UpstreamInit {
            message: e.to_string(),
        })?;
        let own_pid = std::process::id();
        let instance_info = pid::InstanceInfo {
            pid: own_pid,
            transport: transport_str,
            port: if is_http { Some(port) } else { None },
        };
        pid::write_instance(&run_dir, &instance_info).map_err(|e| ProxyError::UpstreamInit {
            message: e.to_string(),
        })?;
        let sock_path = pid::sock_path(&run_dir, own_pid);
        let log_path = pid::log_path(&run_dir, own_pid);
        let _log_handle = log_file::spawn_log_writer(log_path, &log_sender);
        let initializing_report = GatewayStatusReport {
            state: "Initializing".to_string(),
            providers: vec![],
        };
        let (report_tx, report_rx) = tokio::sync::watch::channel(initializing_report);
        let _status_handle = status_socket::start_status_listener(sock_path.clone(), report_rx);
        let (upstreams, statuses) = build_upstreams(servers, verbose).await?;
        let _ = report_tx.send(GatewayStatusReport {
            state: "Listening".to_string(),
            providers: statuses,
        });
        let result = if has_cli_tools {
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
        };
        // In stdio mode, downstream disconnect is a normal exit, not an error.
        let result = if transport == DownstreamTransport::Stdio {
            result.or_else(|e| match e {
                ProxyError::DownstreamInit { .. } => Ok(()),
                other => Err(other),
            })
        } else {
            result
        };
        pid::remove_instance(&run_dir, own_pid);
        result
    })
    .await
}

fn describe_server_entry(entry: &McpServerEntry) -> (&str, &str) {
    match entry {
        McpServerEntry::Stdio(c) => ("stdio", &c.command),
        McpServerEntry::Http(c) => ("http", &c.url),
    }
}

async fn build_upstreams(
    servers: BTreeMap<String, McpServerEntry>,
    verbose: bool,
) -> Result<
    (
        BTreeMap<String, ProviderHandle<RmcpProviderClient, DefaultPolicy>>,
        Vec<ProviderStatus>,
    ),
    ProxyError,
> {
    let mut upstreams = BTreeMap::new();
    let mut statuses = Vec::new();
    let total = servers.len();
    if total > 0 {
        tracing::info!("Connecting to {total} servers...");
    }
    for (name, entry) in servers {
        let (provider_type, target) = describe_server_entry(&entry);
        let provider_type = provider_type.to_string();
        let target = target.to_string();
        let filter = create_policy(
            entry.allowed_operations().to_vec(),
            entry.denied_operations().to_vec(),
        );
        match connect_upstream(&name, entry, verbose).await {
            Ok(service) => {
                statuses.push(ProviderStatus {
                    name: name.clone(),
                    connected: true,
                    provider_type: provider_type.clone(),
                    target,
                });
                upstreams.insert(
                    name.clone(),
                    ProviderHandle {
                        client: RmcpProviderClient::new(service),
                        filter,
                    },
                );
                tracing::info!("  \u{2714} {name} ({provider_type})");
            }
            Err(e) => {
                statuses.push(ProviderStatus {
                    name: name.clone(),
                    connected: false,
                    provider_type,
                    target,
                });
                let reason = match &e {
                    ProxyError::UpstreamInit { message } => message
                        .strip_prefix(&format!("{name}: "))
                        .unwrap_or(message)
                        .to_string(),
                    other => other.to_string(),
                };
                tracing::warn!("  \u{2718} {name} \u{2014} {}", simplify_error(&reason));
            }
        }
    }
    let connected = upstreams.len();
    if total > 0 {
        tracing::info!("Ready ({connected}/{total} connected)");
    }
    Ok((upstreams, statuses))
}

async fn run_foreground_daemon<S: ProviderConfigStore<Entry = McpServerEntry> + ConfigStore>(
    registry: &RegistryService<S>,
    port: u16,
    verbose: bool,
    log_sender: broadcast::Sender<String>,
) -> Result<(), ProxyError> {
    let gateway_config = registry.store().load()?;
    let has_cli_tools = !gateway_config.cli_operations.is_empty();
    let cli_tools = gateway_config.cli_operations;
    run_run(registry, |servers| async move {
        let run_dir = pid::ensure_run_dir().map_err(|e| ProxyError::UpstreamInit {
            message: e.to_string(),
        })?;
        let own_pid = std::process::id();
        let sock_path = pid::sock_path(&run_dir, own_pid);
        let log_path = pid::log_path(&run_dir, own_pid);
        let _log_handle = log_file::spawn_log_writer(log_path, &log_sender);
        let initializing_report = GatewayStatusReport {
            state: "Initializing".to_string(),
            providers: vec![],
        };
        let (report_tx, report_rx) = tokio::sync::watch::channel(initializing_report);
        let _status_handle = status_socket::start_status_listener(sock_path.clone(), report_rx);
        let (upstreams, statuses) = build_upstreams(servers, verbose).await?;
        let _ = report_tx.send(GatewayStatusReport {
            state: "Listening".to_string(),
            providers: statuses,
        });
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
        let result = if has_cli_tools {
            let cli_runner = ProcessCliRunner::new(cli_tools);
            let gateway = Gateway::new(upstreams, cli_runner);
            let adapter = Arc::new(McpAdapter::new(gateway));
            serve_proxy_http(adapter, port, ct, log_sender).await
        } else {
            let gateway = Gateway::new(upstreams, NullCliRunner);
            let adapter = Arc::new(McpAdapter::new(gateway));
            serve_proxy_http(adapter, port, ct, log_sender).await
        };
        status_socket::remove_sock_file(&sock_path);
        result
    })
    .await
}

async fn dispatch_logs(
    args: mcp_gateway::adapters::driving::ui::command::LogsArgs,
) -> Result<(), DaemonError> {
    let run_dir = pid::default_run_dir().unwrap_or_default();
    // Try live instances first
    let instances = pid::list_instances(&run_dir)?;
    if !instances.is_empty() {
        let instance = resolve_instance(args.port, &instances)?;
        let path = pid::log_path(&run_dir, instance.pid);
        return log_file::read_log(&path, args.follow).await;
    }
    // No live instances — look for log files from dead instances
    let log_path = find_latest_log(&run_dir)?;
    log_file::read_log(&log_path, args.follow).await
}

fn find_latest_log(run_dir: &std::path::Path) -> Result<std::path::PathBuf, DaemonError> {
    let entries = std::fs::read_dir(run_dir).map_err(|_| DaemonError::NotRunning)?;
    let mut logs: Vec<_> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "log"))
        .collect();
    logs.sort_by(|a, b| {
        let t_a = a.metadata().and_then(|m| m.modified()).ok();
        let t_b = b.metadata().and_then(|m| m.modified()).ok();
        t_b.cmp(&t_a)
    });
    logs.first()
        .map(|e| e.path())
        .ok_or(DaemonError::NotRunning)
}

async fn run_attach(port_override: Option<u16>) -> Result<(), DaemonError> {
    let port = match port_override {
        Some(p) => p,
        None => {
            let run_dir = pid::default_run_dir().unwrap_or_default();
            let instances = pid::list_instances(&run_dir)?;
            let instance = resolve_instance(None, &instances)?;
            instance.port.ok_or_else(|| DaemonError::AttachFailed {
                message: "selected instance has no port (stdio transport)".to_string(),
            })?
        }
    };
    mcp_gateway::adapters::driving::execution::process::attach::attach(port, &mut std::io::stdout())
        .await
}

fn start_daemon(config_path: &std::path::Path, port: u16) -> Result<(), DaemonError> {
    let run_dir = pid::ensure_run_dir()?;
    // Check if any existing instance already uses this port
    let instances = pid::list_instances(&run_dir)?;
    if let Some(existing) = instances.iter().find(|i| i.port == Some(port)) {
        return Err(DaemonError::AlreadyRunning {
            pid: existing.pid,
            port,
        });
    }
    check_port_available(port)?;
    let child = spawn_daemon_process(config_path, port)?;
    let info = pid::InstanceInfo {
        pid: child.id(),
        transport: "http".to_string(),
        port: Some(port),
    };
    pid::write_instance(&run_dir, &info)?;
    tracing::info!("gateway started on port {port} (PID {})", child.id());
    Ok(())
}

fn check_port_available(port: u16) -> Result<(), DaemonError> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", port))
        .map_err(|_| DaemonError::PortInUse { port })?;
    drop(listener);
    Ok(())
}

fn spawn_daemon_process(
    config_path: &std::path::Path,
    port: u16,
) -> Result<std::process::Child, DaemonError> {
    let exe = std::env::args_os()
        .next()
        .map(std::path::PathBuf::from)
        .ok_or_else(|| DaemonError::PidWrite {
            message: "cannot determine executable path: argv[0] is empty".to_string(),
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
        .map_err(|e| DaemonError::PidWrite {
            message: format!("failed to spawn daemon: {e}"),
        })
}

async fn dispatch_oauth<S: ProviderConfigStore<Entry = McpServerEntry> + ConfigStore>(
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
    verbose: bool,
) -> Result<rmcp::service::RunningService<rmcp::RoleClient, ()>, ProxyError> {
    let fut = connect_upstream_inner(name, entry, verbose);
    tokio::time::timeout(UPSTREAM_CONNECT_TIMEOUT, fut)
        .await
        .map_err(|_| ProxyError::UpstreamInit {
            message: format!("{name}: connection timed out after {UPSTREAM_CONNECT_TIMEOUT:?}"),
        })?
}

async fn connect_upstream_inner(
    name: &str,
    entry: McpServerEntry,
    verbose: bool,
) -> Result<rmcp::service::RunningService<rmcp::RoleClient, ()>, ProxyError> {
    match entry {
        McpServerEntry::Stdio(config) => {
            let transport =
                mcp_gateway::adapters::driven::connectivity::mcp_protocol::proxy::spawn_transport(
                    &config, verbose,
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

fn simplify_error(msg: &str) -> String {
    // Strip verbose rmcp Transport type paths:
    // "Send message error Transport [...] error: Auth error: X" → "Auth error: X"
    if let Some(pos) = msg.find("] error: ") {
        return msg[pos + "] error: ".len()..].to_string();
    }
    msg.to_string()
}
