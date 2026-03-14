use std::collections::BTreeMap;
use std::future::Future;
use std::io::Write;

use super::command::{
    AddArgs, AllowlistModifyArgs, AllowlistShowArgs, DenylistModifyArgs, DenylistShowArgs,
    RemoveArgs, RulesArgs, TransportType,
};
use crate::adapters::driven::configuration::model::{
    CliOperationDef, HttpConfig, McpServerEntry, StdioConfig,
};
use crate::adapters::driven::connectivity::mcp_protocol::error::ProxyError;
use crate::hexagon::ports::driven::operation_policy::OperationPolicy;
use crate::hexagon::ports::driven::provider_client::ProviderClient;
use crate::hexagon::ports::driven::provider_config_store::ProviderConfigStore;
use crate::hexagon::usecases::gateway::ProviderHandle;
use crate::hexagon::usecases::registry_error::RegistryError;
use crate::hexagon::usecases::registry_service::RegistryService;

pub fn run_rules<S: ProviderConfigStore<Entry = McpServerEntry>>(
    service: &RegistryService<S>,
    cli_operations: &BTreeMap<String, CliOperationDef>,
    args: RulesArgs,
    out: &mut impl Write,
) -> Result<(), RegistryError> {
    let servers = service.list_providers()?;
    let filter_name = args.name.as_deref();
    let server_count = servers
        .iter()
        .filter(|(server_name, _)| filter_name.is_none_or(|name| name == server_name.as_str()))
        .map(|(server_name, entry)| {
            let (server_type, target) = describe_entry(entry);
            let allowed = entry.allowed_operations();
            let denied = entry.denied_operations();
            let policy = policy_label(allowed, denied);
            let _ = writeln!(out, "{server_name} ({server_type} → {target})");
            let _ = writeln!(out, "  policy: {policy}");
            for tool in allowed {
                let prefixed = crate::hexagon::usecases::mapping::encode(server_name, tool);
                let _ = writeln!(out, "  ALLOW  {tool:<40} → {prefixed}");
            }
            for tool in denied {
                let _ = writeln!(out, "  DENY   {tool}");
            }
            if allowed.is_empty() && denied.is_empty() {
                let _ = writeln!(out, "  (no rules — all upstream tools forwarded)");
            }
            let _ = writeln!(out);
        })
        .count();
    let cli_count = cli_operations
        .iter()
        .filter(|(name, _)| filter_name.is_none_or(|f| f == name.as_str()))
        .map(|(name, def)| {
            let desc = def.description.as_deref().unwrap_or_else(|| &def.command);
            let _ = writeln!(out, "{name} (cli → {cmd})", cmd = def.command);
            let _ = writeln!(out, "  policy: open");
            let _ = writeln!(out, "  ALLOW  {desc:<40} → {name}");
            let _ = writeln!(out);
        })
        .count();
    if let Some(name) = filter_name {
        if server_count + cli_count == 0 {
            return Err(RegistryError::NotFound {
                name: name.to_string(),
            });
        }
    }
    Ok(())
}

pub async fn run_tools<U: ProviderClient, F: OperationPolicy>(
    providers: &BTreeMap<String, ProviderHandle<U, F>>,
    name_filter: Option<&str>,
    out: &mut impl Write,
) -> Result<(), ProxyError> {
    let entries: Vec<(&str, &ProviderHandle<U, F>)> = match name_filter {
        Some(name) => {
            let handle = providers
                .get(name)
                .ok_or_else(|| ProxyError::UpstreamInit {
                    message: format!("server '{name}' not found"),
                })?;
            vec![(name, handle)]
        }
        None => providers.iter().map(|(k, v)| (k.as_str(), v)).collect(),
    };
    if entries.is_empty() {
        return Ok(());
    }
    for (server_name, handle) in &entries {
        let upstream_tools = match handle.client.list_operations().await {
            Ok(tools) => tools,
            Err(e) => {
                let _ = writeln!(out, "{server_name}");
                let _ = writeln!(out, "  ERROR  {e}");
                let _ = writeln!(out);
                continue;
            }
        };
        let _ = writeln!(out, "{server_name}");
        if upstream_tools.is_empty() {
            let _ = writeln!(out, "  (no tools)");
        }
        for tool in &upstream_tools {
            if handle.filter.is_allowed(&tool.name) {
                let prefixed = crate::hexagon::usecases::mapping::encode(server_name, &tool.name);
                let _ = writeln!(out, "  ALLOW  {:<40} → {prefixed}", tool.name);
            } else {
                let _ = writeln!(out, "  BLOCK  {}", tool.name);
            }
        }
        let _ = writeln!(out);
    }
    Ok(())
}

fn policy_label(allowed: &[String], denied: &[String]) -> &'static str {
    match (allowed.is_empty(), denied.is_empty()) {
        (true, true) => "open",
        (false, true) => "allowlist",
        (true, false) => "denylist",
        (false, false) => "allowlist + denylist",
    }
}

pub fn run_list<S: ProviderConfigStore<Entry = McpServerEntry>>(
    service: &RegistryService<S>,
    out: &mut impl Write,
) -> Result<(), RegistryError> {
    let servers = service.list_providers()?;
    if servers.is_empty() {
        return Ok(());
    }
    let _ = writeln!(out, "{:<20} {:<10} TARGET", "NAME", "TYPE");
    for (name, entry) in &servers {
        let (server_type, target) = describe_entry(entry);
        let _ = writeln!(out, "{name:<20} {server_type:<10} {target}");
    }
    Ok(())
}

fn describe_entry(entry: &McpServerEntry) -> (&str, &str) {
    match entry {
        McpServerEntry::Stdio(c) => ("stdio", &c.command),
        McpServerEntry::Http(c) => ("http", &c.url),
    }
}

pub async fn run_run<S, F, Fut>(
    service: &RegistryService<S>,
    run_proxy: F,
) -> Result<(), ProxyError>
where
    S: ProviderConfigStore<Entry = McpServerEntry>,
    F: FnOnce(BTreeMap<String, McpServerEntry>) -> Fut,
    Fut: Future<Output = Result<(), ProxyError>>,
{
    let servers = service.list_providers()?;
    run_proxy(servers).await
}

pub fn run_remove<S: ProviderConfigStore<Entry = McpServerEntry>>(
    service: &RegistryService<S>,
    args: RemoveArgs,
) -> Result<(), RegistryError> {
    service.remove_provider(&args.name)
}

pub fn run_allowlist_add<S: ProviderConfigStore<Entry = McpServerEntry>>(
    service: &RegistryService<S>,
    args: AllowlistModifyArgs,
) -> Result<(), RegistryError> {
    service.add_allowed_operations(&args.name, &args.tools)
}

pub fn run_allowlist_remove<S: ProviderConfigStore<Entry = McpServerEntry>>(
    service: &RegistryService<S>,
    args: AllowlistModifyArgs,
) -> Result<(), RegistryError> {
    service.remove_allowed_operations(&args.name, &args.tools)
}

pub fn run_allowlist_show<S: ProviderConfigStore<Entry = McpServerEntry>>(
    service: &RegistryService<S>,
    args: AllowlistShowArgs,
    out: &mut impl Write,
) -> Result<(), RegistryError> {
    let tools = service.get_allowed_operations(&args.name)?;
    for tool in &tools {
        let _ = writeln!(out, "{tool}");
    }
    Ok(())
}

pub fn run_denylist_add<S: ProviderConfigStore<Entry = McpServerEntry>>(
    service: &RegistryService<S>,
    args: DenylistModifyArgs,
) -> Result<(), RegistryError> {
    service.add_denied_operations(&args.name, &args.tools)
}

pub fn run_denylist_remove<S: ProviderConfigStore<Entry = McpServerEntry>>(
    service: &RegistryService<S>,
    args: DenylistModifyArgs,
) -> Result<(), RegistryError> {
    service.remove_denied_operations(&args.name, &args.tools)
}

pub fn run_denylist_show<S: ProviderConfigStore<Entry = McpServerEntry>>(
    service: &RegistryService<S>,
    args: DenylistShowArgs,
    out: &mut impl Write,
) -> Result<(), RegistryError> {
    let tools = service.get_denied_operations(&args.name)?;
    for tool in &tools {
        let _ = writeln!(out, "{tool}");
    }
    Ok(())
}

pub fn run_add<S: ProviderConfigStore<Entry = McpServerEntry>>(
    service: &RegistryService<S>,
    args: AddArgs,
) -> Result<(), RegistryError> {
    let entry = build_entry(
        args.transport,
        args.command,
        args.args,
        args.env_vars,
        args.url,
        args.headers,
    );
    service.add_provider(args.name, entry)
}

fn build_entry(
    transport: TransportType,
    command: Option<String>,
    args: Vec<String>,
    env_vars: Vec<(String, String)>,
    url: Option<String>,
    headers: Vec<(String, String)>,
) -> McpServerEntry {
    match transport {
        TransportType::Stdio => McpServerEntry::Stdio(StdioConfig {
            command: command.unwrap_or_default(),
            args,
            env: env_vars.into_iter().collect::<BTreeMap<_, _>>(),
            allowed_operations: vec![],
            denied_operations: vec![],
        }),
        TransportType::Http => McpServerEntry::Http(HttpConfig {
            url: url.unwrap_or_default(),
            headers: headers.into_iter().collect::<BTreeMap<_, _>>(),
            allowed_operations: vec![],
            denied_operations: vec![],
            auth: None,
        }),
    }
}
