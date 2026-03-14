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
use crate::hexagon::ports::{OperationPolicy, ProviderClient, ProviderConfigStore};
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
    let mut printed = false;
    for (server_name, entry) in &servers {
        if let Some(name) = filter_name {
            if name != server_name.as_str() {
                continue;
            }
        }
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
        printed = true;
    }
    for (name, def) in cli_operations {
        if let Some(filter) = filter_name {
            if filter != name.as_str() {
                continue;
            }
        }
        let desc = def.description.as_deref().unwrap_or_else(|| &def.command);
        let _ = writeln!(out, "{name} (cli → {cmd})", cmd = def.command);
        let _ = writeln!(out, "  policy: open");
        let _ = writeln!(out, "  ALLOW  {desc:<40} → {name}");
        let _ = writeln!(out);
        printed = true;
    }
    if let Some(name) = filter_name {
        if !printed {
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::adapters::driven::configuration::model::GatewayConfig;
    use std::sync::Mutex;

    struct FakeConfigStore {
        entries: Mutex<BTreeMap<String, McpServerEntry>>,
        fail_load: bool,
    }

    impl FakeConfigStore {
        fn new(config: GatewayConfig) -> Self {
            Self {
                entries: Mutex::new(config.mcp_servers),
                fail_load: false,
            }
        }

        fn failing() -> Self {
            Self {
                entries: Mutex::new(BTreeMap::new()),
                fail_load: true,
            }
        }
    }

    impl ProviderConfigStore for FakeConfigStore {
        type Entry = McpServerEntry;

        fn load_entries(&self) -> Result<BTreeMap<String, McpServerEntry>, String> {
            if self.fail_load {
                return Err("denied".to_string());
            }
            Ok(self.entries.lock().unwrap().clone())
        }

        fn save_entries(&self, entries: BTreeMap<String, McpServerEntry>) -> Result<(), String> {
            *self.entries.lock().unwrap() = entries;
            Ok(())
        }
    }

    fn stdio_config(command: &str) -> GatewayConfig {
        let mut config = GatewayConfig::default();
        config.mcp_servers.insert(
            "test".to_string(),
            McpServerEntry::Stdio(StdioConfig {
                command: command.to_string(),
                args: vec![],
                env: BTreeMap::new(),
                allowed_operations: vec![],
                denied_operations: vec![],
            }),
        );
        config
    }

    async fn noop_proxy(_entries: BTreeMap<String, McpServerEntry>) -> Result<(), ProxyError> {
        Ok(())
    }

    #[test]
    fn run_add_stdio_creates_entry() {
        let store = FakeConfigStore::new(GatewayConfig::default());
        let service = RegistryService::new(store);

        let args = AddArgs {
            name: "test".to_string(),
            transport: TransportType::Stdio,
            command: Some("node".to_string()),
            args: vec!["server.js".to_string()],
            env_vars: vec![("KEY".to_string(), "val".to_string())],
            url: None,
            headers: vec![],
        };

        run_add(&service, args).unwrap();

        let entries = service.store().load_entries().unwrap();
        let entry = entries.get("test").unwrap();
        assert_eq!(
            entry,
            &McpServerEntry::Stdio(StdioConfig {
                command: "node".to_string(),
                args: vec!["server.js".to_string()],
                env: BTreeMap::from([("KEY".to_string(), "val".to_string())]),
                allowed_operations: vec![],
                denied_operations: vec![],
            })
        );
    }

    #[test]
    fn run_add_http_creates_entry() {
        let store = FakeConfigStore::new(GatewayConfig::default());
        let service = RegistryService::new(store);

        let args = AddArgs {
            name: "remote".to_string(),
            transport: TransportType::Http,
            command: None,
            args: vec![],
            env_vars: vec![],
            url: Some("https://example.com".to_string()),
            headers: vec![("Auth".to_string(), "tok".to_string())],
        };

        run_add(&service, args).unwrap();

        let entries = service.store().load_entries().unwrap();
        let entry = entries.get("remote").unwrap();
        assert_eq!(
            entry,
            &McpServerEntry::Http(HttpConfig {
                url: "https://example.com".to_string(),
                headers: BTreeMap::from([("Auth".to_string(), "tok".to_string())]),
                allowed_operations: vec![],
                denied_operations: vec![],
                auth: None,
            })
        );
    }

    #[test]
    fn run_add_duplicate_fails() {
        let mut initial = GatewayConfig::default();
        initial.mcp_servers.insert(
            "existing".to_string(),
            McpServerEntry::Stdio(StdioConfig {
                command: "echo".to_string(),
                args: vec![],
                env: BTreeMap::new(),
                allowed_operations: vec![],
                denied_operations: vec![],
            }),
        );
        let store = FakeConfigStore::new(initial);
        let service = RegistryService::new(store);

        let args = AddArgs {
            name: "existing".to_string(),
            transport: TransportType::Stdio,
            command: Some("echo".to_string()),
            args: vec![],
            env_vars: vec![],
            url: None,
            headers: vec![],
        };

        let result = run_add(&service, args);
        assert!(matches!(result, Err(RegistryError::AlreadyExists { .. })));
    }

    #[test]
    fn build_stdio_entry() {
        let entry = build_entry(
            TransportType::Stdio,
            Some("cmd".to_string()),
            vec!["arg".to_string()],
            vec![("K".to_string(), "V".to_string())],
            None,
            vec![],
        );
        assert_eq!(
            entry,
            McpServerEntry::Stdio(StdioConfig {
                command: "cmd".to_string(),
                args: vec!["arg".to_string()],
                env: BTreeMap::from([("K".to_string(), "V".to_string())]),
                allowed_operations: vec![],
                denied_operations: vec![],
            })
        );
    }

    #[test]
    fn build_http_entry() {
        let entry = build_entry(
            TransportType::Http,
            None,
            vec![],
            vec![],
            Some("https://x.com".to_string()),
            vec![("H".to_string(), "V".to_string())],
        );
        assert_eq!(
            entry,
            McpServerEntry::Http(HttpConfig {
                url: "https://x.com".to_string(),
                headers: BTreeMap::from([("H".to_string(), "V".to_string())]),
                allowed_operations: vec![],
                denied_operations: vec![],
                auth: None,
            })
        );
    }

    #[test]
    fn describe_stdio_entry_returns_type_and_command() {
        let entry = McpServerEntry::Stdio(StdioConfig {
            command: "node".to_string(),
            args: vec![],
            env: BTreeMap::new(),
            allowed_operations: vec![],
            denied_operations: vec![],
        });
        assert_eq!(describe_entry(&entry), ("stdio", "node"));
    }

    #[test]
    fn describe_http_entry_returns_type_and_url() {
        let entry = McpServerEntry::Http(HttpConfig {
            url: "https://example.com".to_string(),
            headers: BTreeMap::new(),
            allowed_operations: vec![],
            denied_operations: vec![],
            auth: None,
        });
        assert_eq!(describe_entry(&entry), ("http", "https://example.com"));
    }

    #[test]
    fn run_list_empty_config_writes_nothing() {
        let store = FakeConfigStore::new(GatewayConfig::default());
        let service = RegistryService::new(store);

        let mut buf = Vec::new();
        run_list(&service, &mut buf).unwrap();
        assert!(buf.is_empty());
    }

    #[test]
    fn run_list_populated_config_writes_table() {
        let mut config = GatewayConfig::default();
        config.mcp_servers.insert(
            "my-server".to_string(),
            McpServerEntry::Stdio(StdioConfig {
                command: "node".to_string(),
                args: vec![],
                env: BTreeMap::new(),
                allowed_operations: vec![],
                denied_operations: vec![],
            }),
        );
        config.mcp_servers.insert(
            "remote".to_string(),
            McpServerEntry::Http(HttpConfig {
                url: "https://example.com".to_string(),
                headers: BTreeMap::new(),
                allowed_operations: vec![],
                denied_operations: vec![],
                auth: None,
            }),
        );
        let store = FakeConfigStore::new(config);
        let service = RegistryService::new(store);

        let mut buf = Vec::new();
        run_list(&service, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("NAME"));
        assert!(output.contains("TYPE"));
        assert!(output.contains("TARGET"));
        assert!(output.contains("my-server"));
        assert!(output.contains("stdio"));
        assert!(output.contains("node"));
        assert!(output.contains("remote"));
        assert!(output.contains("http"));
        assert!(output.contains("https://example.com"));
    }

    #[test]
    fn run_list_with_store_error_propagates() {
        let service = RegistryService::new(FakeConfigStore::failing());
        let mut buf = Vec::new();
        let result = run_list(&service, &mut buf);
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }

    #[test]
    fn run_remove_existing_server_succeeds() {
        let mut config = GatewayConfig::default();
        config.mcp_servers.insert(
            "target".to_string(),
            McpServerEntry::Stdio(StdioConfig {
                command: "echo".to_string(),
                args: vec![],
                env: BTreeMap::new(),
                allowed_operations: vec![],
                denied_operations: vec![],
            }),
        );
        let store = FakeConfigStore::new(config);
        let service = RegistryService::new(store);

        let args = RemoveArgs {
            name: "target".to_string(),
        };
        run_remove(&service, args).unwrap();

        let entries = service.store().load_entries().unwrap();
        assert!(!entries.contains_key("target"));
    }

    #[test]
    fn run_remove_nonexistent_returns_not_found() {
        let store = FakeConfigStore::new(GatewayConfig::default());
        let service = RegistryService::new(store);

        let args = RemoveArgs {
            name: "nope".to_string(),
        };
        let result = run_remove(&service, args);
        assert!(matches!(result, Err(RegistryError::NotFound { .. })));
    }

    #[tokio::test]
    async fn run_run_empty_config_succeeds() {
        let store = FakeConfigStore::new(GatewayConfig::default());
        let service = RegistryService::new(store);

        let result = run_run(&service, noop_proxy).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn run_run_passes_all_servers_to_proxy() {
        let store = FakeConfigStore::new(stdio_config("node"));
        let service = RegistryService::new(store);

        let result = run_run(&service, |servers| async move {
            assert_eq!(servers.len(), 1);
            assert!(servers.contains_key("test"));
            Ok(())
        })
        .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn run_run_with_store_error_propagates() {
        let service = RegistryService::new(FakeConfigStore::failing());

        let result = run_run(&service, noop_proxy).await;
        assert!(matches!(result, Err(ProxyError::Registry(_))));
    }

    #[tokio::test]
    async fn run_run_proxy_error_propagates() {
        let store = FakeConfigStore::new(stdio_config("node"));
        let service = RegistryService::new(store);

        let result = run_run(&service, failing_proxy).await;
        assert!(matches!(result, Err(ProxyError::UpstreamSpawn { .. })));
    }

    #[tokio::test]
    async fn run_run_e2e_with_in_memory_proxy() {
        let store = FakeConfigStore::new(stdio_config("unused"));
        let service = RegistryService::new(store);

        let result = run_run(&service, e2e_proxy).await;
        assert!(result.is_ok());
    }

    #[test]
    fn run_allowlist_add_appends_tools() {
        let store = FakeConfigStore::new(stdio_config("echo"));
        let service = RegistryService::new(store);

        let args = AllowlistModifyArgs {
            name: "test".to_string(),
            tools: vec!["read".to_string(), "write".to_string()],
        };
        run_allowlist_add(&service, args).unwrap();

        let entries = service.store().load_entries().unwrap();
        let entry = entries.get("test").unwrap();
        assert_eq!(entry.allowed_operations(), &["read", "write"]);
    }

    #[test]
    fn run_allowlist_add_server_not_found() {
        let store = FakeConfigStore::new(GatewayConfig::default());
        let service = RegistryService::new(store);

        let args = AllowlistModifyArgs {
            name: "nope".to_string(),
            tools: vec!["read".to_string()],
        };
        let result = run_allowlist_add(&service, args);
        assert!(matches!(result, Err(RegistryError::NotFound { .. })));
    }

    #[test]
    fn run_allowlist_remove_removes_tools() {
        let mut config = GatewayConfig::default();
        config.mcp_servers.insert(
            "test".to_string(),
            McpServerEntry::Stdio(StdioConfig {
                command: "echo".to_string(),
                args: vec![],
                env: BTreeMap::new(),
                allowed_operations: vec!["read".to_string(), "write".to_string()],
                denied_operations: vec![],
            }),
        );
        let store = FakeConfigStore::new(config);
        let service = RegistryService::new(store);

        let args = AllowlistModifyArgs {
            name: "test".to_string(),
            tools: vec!["read".to_string()],
        };
        run_allowlist_remove(&service, args).unwrap();

        let entries = service.store().load_entries().unwrap();
        let entry = entries.get("test").unwrap();
        assert_eq!(entry.allowed_operations(), &["write"]);
    }

    #[test]
    fn run_allowlist_remove_server_not_found() {
        let store = FakeConfigStore::new(GatewayConfig::default());
        let service = RegistryService::new(store);

        let args = AllowlistModifyArgs {
            name: "nope".to_string(),
            tools: vec!["read".to_string()],
        };
        let result = run_allowlist_remove(&service, args);
        assert!(matches!(result, Err(RegistryError::NotFound { .. })));
    }

    #[test]
    fn run_allowlist_show_prints_tools() {
        let mut config = GatewayConfig::default();
        config.mcp_servers.insert(
            "test".to_string(),
            McpServerEntry::Stdio(StdioConfig {
                command: "echo".to_string(),
                args: vec![],
                env: BTreeMap::new(),
                allowed_operations: vec!["read".to_string(), "write".to_string()],
                denied_operations: vec![],
            }),
        );
        let store = FakeConfigStore::new(config);
        let service = RegistryService::new(store);

        let args = AllowlistShowArgs {
            name: "test".to_string(),
        };
        let mut buf = Vec::new();
        run_allowlist_show(&service, args, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("read"));
        assert!(output.contains("write"));
    }

    #[test]
    fn run_allowlist_show_empty_prints_nothing() {
        let store = FakeConfigStore::new(stdio_config("echo"));
        let service = RegistryService::new(store);

        let args = AllowlistShowArgs {
            name: "test".to_string(),
        };
        let mut buf = Vec::new();
        run_allowlist_show(&service, args, &mut buf).unwrap();
        assert!(buf.is_empty());
    }

    #[test]
    fn run_allowlist_show_server_not_found() {
        let store = FakeConfigStore::new(GatewayConfig::default());
        let service = RegistryService::new(store);

        let args = AllowlistShowArgs {
            name: "nope".to_string(),
        };
        let mut buf = Vec::new();
        let result = run_allowlist_show(&service, args, &mut buf);
        assert!(matches!(result, Err(RegistryError::NotFound { .. })));
    }

    #[test]
    fn run_allowlist_add_store_error_propagates() {
        let service = RegistryService::new(FakeConfigStore::failing());
        let args = AllowlistModifyArgs {
            name: "test".to_string(),
            tools: vec!["read".to_string()],
        };
        let result = run_allowlist_add(&service, args);
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }

    #[test]
    fn run_allowlist_remove_store_error_propagates() {
        let service = RegistryService::new(FakeConfigStore::failing());
        let args = AllowlistModifyArgs {
            name: "test".to_string(),
            tools: vec!["read".to_string()],
        };
        let result = run_allowlist_remove(&service, args);
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }

    #[test]
    fn run_allowlist_show_store_error_propagates() {
        let service = RegistryService::new(FakeConfigStore::failing());
        let args = AllowlistShowArgs {
            name: "test".to_string(),
        };
        let mut buf = Vec::new();
        let result = run_allowlist_show(&service, args, &mut buf);
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }

    #[test]
    fn run_denylist_add_appends_tools() {
        let store = FakeConfigStore::new(stdio_config("echo"));
        let service = RegistryService::new(store);

        let args = DenylistModifyArgs {
            name: "test".to_string(),
            tools: vec!["delete".to_string(), "exec".to_string()],
        };
        run_denylist_add(&service, args).unwrap();

        let entries = service.store().load_entries().unwrap();
        let entry = entries.get("test").unwrap();
        assert_eq!(entry.denied_operations(), &["delete", "exec"]);
    }

    #[test]
    fn run_denylist_add_server_not_found() {
        let store = FakeConfigStore::new(GatewayConfig::default());
        let service = RegistryService::new(store);

        let args = DenylistModifyArgs {
            name: "nope".to_string(),
            tools: vec!["delete".to_string()],
        };
        let result = run_denylist_add(&service, args);
        assert!(matches!(result, Err(RegistryError::NotFound { .. })));
    }

    #[test]
    fn run_denylist_remove_removes_tools() {
        let mut config = GatewayConfig::default();
        config.mcp_servers.insert(
            "test".to_string(),
            McpServerEntry::Stdio(StdioConfig {
                command: "echo".to_string(),
                args: vec![],
                env: BTreeMap::new(),
                allowed_operations: vec![],
                denied_operations: vec!["delete".to_string(), "exec".to_string()],
            }),
        );
        let store = FakeConfigStore::new(config);
        let service = RegistryService::new(store);

        let args = DenylistModifyArgs {
            name: "test".to_string(),
            tools: vec!["delete".to_string()],
        };
        run_denylist_remove(&service, args).unwrap();

        let entries = service.store().load_entries().unwrap();
        let entry = entries.get("test").unwrap();
        assert_eq!(entry.denied_operations(), &["exec"]);
    }

    #[test]
    fn run_denylist_remove_server_not_found() {
        let store = FakeConfigStore::new(GatewayConfig::default());
        let service = RegistryService::new(store);

        let args = DenylistModifyArgs {
            name: "nope".to_string(),
            tools: vec!["delete".to_string()],
        };
        let result = run_denylist_remove(&service, args);
        assert!(matches!(result, Err(RegistryError::NotFound { .. })));
    }

    #[test]
    fn run_denylist_show_prints_tools() {
        let mut config = GatewayConfig::default();
        config.mcp_servers.insert(
            "test".to_string(),
            McpServerEntry::Stdio(StdioConfig {
                command: "echo".to_string(),
                args: vec![],
                env: BTreeMap::new(),
                allowed_operations: vec![],
                denied_operations: vec!["delete".to_string(), "exec".to_string()],
            }),
        );
        let store = FakeConfigStore::new(config);
        let service = RegistryService::new(store);

        let args = DenylistShowArgs {
            name: "test".to_string(),
        };
        let mut buf = Vec::new();
        run_denylist_show(&service, args, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("delete"));
        assert!(output.contains("exec"));
    }

    #[test]
    fn run_denylist_show_empty_prints_nothing() {
        let store = FakeConfigStore::new(stdio_config("echo"));
        let service = RegistryService::new(store);

        let args = DenylistShowArgs {
            name: "test".to_string(),
        };
        let mut buf = Vec::new();
        run_denylist_show(&service, args, &mut buf).unwrap();
        assert!(buf.is_empty());
    }

    #[test]
    fn run_denylist_show_server_not_found() {
        let store = FakeConfigStore::new(GatewayConfig::default());
        let service = RegistryService::new(store);

        let args = DenylistShowArgs {
            name: "nope".to_string(),
        };
        let mut buf = Vec::new();
        let result = run_denylist_show(&service, args, &mut buf);
        assert!(matches!(result, Err(RegistryError::NotFound { .. })));
    }

    #[test]
    fn run_denylist_add_store_error_propagates() {
        let service = RegistryService::new(FakeConfigStore::failing());
        let args = DenylistModifyArgs {
            name: "test".to_string(),
            tools: vec!["delete".to_string()],
        };
        let result = run_denylist_add(&service, args);
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }

    #[test]
    fn run_denylist_remove_store_error_propagates() {
        let service = RegistryService::new(FakeConfigStore::failing());
        let args = DenylistModifyArgs {
            name: "test".to_string(),
            tools: vec!["delete".to_string()],
        };
        let result = run_denylist_remove(&service, args);
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }

    #[test]
    fn run_denylist_show_store_error_propagates() {
        let service = RegistryService::new(FakeConfigStore::failing());
        let args = DenylistShowArgs {
            name: "test".to_string(),
        };
        let mut buf = Vec::new();
        let result = run_denylist_show(&service, args, &mut buf);
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }

    #[test]
    fn policy_label_open_when_no_rules() {
        assert_eq!(policy_label(&[], &[]), "open");
    }

    #[test]
    fn policy_label_allowlist_when_only_allowed() {
        let allowed = vec!["read".to_string()];
        assert_eq!(policy_label(&allowed, &[]), "allowlist");
    }

    #[test]
    fn policy_label_denylist_when_only_denied() {
        let denied = vec!["delete".to_string()];
        assert_eq!(policy_label(&[], &denied), "denylist");
    }

    #[test]
    fn policy_label_both_when_allowed_and_denied() {
        let allowed = vec!["read".to_string()];
        let denied = vec!["delete".to_string()];
        assert_eq!(policy_label(&allowed, &denied), "allowlist + denylist");
    }

    #[test]
    fn run_rules_empty_config_writes_nothing() {
        let store = FakeConfigStore::new(GatewayConfig::default());
        let service = RegistryService::new(store);

        let args = RulesArgs { name: None };
        let mut buf = Vec::new();
        run_rules(&service, &BTreeMap::new(), args, &mut buf).unwrap();
        assert!(buf.is_empty());
    }

    #[test]
    fn run_rules_shows_open_policy_when_no_rules() {
        let store = FakeConfigStore::new(stdio_config("echo"));
        let service = RegistryService::new(store);

        let args = RulesArgs { name: None };
        let mut buf = Vec::new();
        run_rules(&service, &BTreeMap::new(), args, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("test (stdio"));
        assert!(output.contains("policy: open"));
        assert!(output.contains("no rules"));
    }

    #[test]
    fn run_rules_shows_allowlist_with_prefixed_names() {
        let mut config = GatewayConfig::default();
        config.mcp_servers.insert(
            "my-server".to_string(),
            McpServerEntry::Stdio(StdioConfig {
                command: "node".to_string(),
                args: vec![],
                env: BTreeMap::new(),
                allowed_operations: vec!["read".to_string(), "search".to_string()],
                denied_operations: vec![],
            }),
        );
        let store = FakeConfigStore::new(config);
        let service = RegistryService::new(store);

        let args = RulesArgs { name: None };
        let mut buf = Vec::new();
        run_rules(&service, &BTreeMap::new(), args, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("policy: allowlist"));
        assert!(output.contains("ALLOW  read"));
        assert!(output.contains("→ my-server__read"));
        assert!(output.contains("ALLOW  search"));
        assert!(output.contains("→ my-server__search"));
        assert!(!output.contains("no rules"));
    }

    #[test]
    fn run_rules_shows_denylist() {
        let mut config = GatewayConfig::default();
        config.mcp_servers.insert(
            "srv".to_string(),
            McpServerEntry::Http(HttpConfig {
                url: "https://example.com".to_string(),
                headers: BTreeMap::new(),
                allowed_operations: vec![],
                denied_operations: vec!["delete".to_string()],
                auth: None,
            }),
        );
        let store = FakeConfigStore::new(config);
        let service = RegistryService::new(store);

        let args = RulesArgs { name: None };
        let mut buf = Vec::new();
        run_rules(&service, &BTreeMap::new(), args, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("policy: denylist"));
        assert!(output.contains("DENY   delete"));
        assert!(!output.contains("no rules"));
    }

    #[test]
    fn run_rules_shows_combined_policy() {
        let mut config = GatewayConfig::default();
        config.mcp_servers.insert(
            "combo".to_string(),
            McpServerEntry::Stdio(StdioConfig {
                command: "cmd".to_string(),
                args: vec![],
                env: BTreeMap::new(),
                allowed_operations: vec!["read".to_string()],
                denied_operations: vec!["exec".to_string()],
            }),
        );
        let store = FakeConfigStore::new(config);
        let service = RegistryService::new(store);

        let args = RulesArgs { name: None };
        let mut buf = Vec::new();
        run_rules(&service, &BTreeMap::new(), args, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("policy: allowlist + denylist"));
        assert!(output.contains("ALLOW  read"));
        assert!(output.contains("DENY   exec"));
        assert!(!output.contains("no rules"));
    }

    #[test]
    fn run_rules_filters_by_server_name() {
        let mut config = GatewayConfig::default();
        config.mcp_servers.insert(
            "alpha".to_string(),
            McpServerEntry::Stdio(StdioConfig {
                command: "a".to_string(),
                args: vec![],
                env: BTreeMap::new(),
                allowed_operations: vec!["tool_a".to_string()],
                denied_operations: vec![],
            }),
        );
        config.mcp_servers.insert(
            "beta".to_string(),
            McpServerEntry::Stdio(StdioConfig {
                command: "b".to_string(),
                args: vec![],
                env: BTreeMap::new(),
                allowed_operations: vec!["tool_b".to_string()],
                denied_operations: vec![],
            }),
        );
        let store = FakeConfigStore::new(config);
        let service = RegistryService::new(store);

        let args = RulesArgs {
            name: Some("alpha".to_string()),
        };
        let mut buf = Vec::new();
        run_rules(&service, &BTreeMap::new(), args, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("alpha"));
        assert!(output.contains("tool_a"));
        assert!(!output.contains("beta"));
        assert!(!output.contains("tool_b"));
    }

    #[test]
    fn run_rules_server_not_found() {
        let store = FakeConfigStore::new(GatewayConfig::default());
        let service = RegistryService::new(store);

        let args = RulesArgs {
            name: Some("nope".to_string()),
        };
        let mut buf = Vec::new();
        let result = run_rules(&service, &BTreeMap::new(), args, &mut buf);
        assert!(matches!(result, Err(RegistryError::NotFound { .. })));
    }

    #[test]
    fn run_rules_store_error_propagates() {
        let service = RegistryService::new(FakeConfigStore::failing());
        let args = RulesArgs { name: None };
        let mut buf = Vec::new();
        let result = run_rules(&service, &BTreeMap::new(), args, &mut buf);
        assert!(matches!(result, Err(RegistryError::Storage(_))));
    }

    #[test]
    fn run_rules_shows_cli_tools() {
        let store = FakeConfigStore::new(GatewayConfig::default());
        let service = RegistryService::new(store);
        let mut cli = BTreeMap::new();
        cli.insert(
            "gh-pr-list".to_string(),
            CliOperationDef {
                command: "/scripts/gh-pr-list.sh".to_string(),
                description: Some("List pull requests".to_string()),
            },
        );
        let args = RulesArgs { name: None };
        let mut buf = Vec::new();
        run_rules(&service, &cli, args, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("gh-pr-list"));
        assert!(output.contains("cli"));
        assert!(output.contains("/scripts/gh-pr-list.sh"));
        assert!(output.contains("List pull requests"));
        assert!(output.contains("open"));
    }

    #[test]
    fn run_rules_cli_tool_uses_command_as_fallback_description() {
        let store = FakeConfigStore::new(GatewayConfig::default());
        let service = RegistryService::new(store);
        let mut cli = BTreeMap::new();
        cli.insert(
            "my-tool".to_string(),
            CliOperationDef {
                command: "/bin/my-tool".to_string(),
                description: None,
            },
        );
        let args = RulesArgs { name: None };
        let mut buf = Vec::new();
        run_rules(&service, &cli, args, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("/bin/my-tool"));
    }

    #[test]
    fn run_rules_filters_cli_tool_by_name() {
        let store = FakeConfigStore::new(GatewayConfig::default());
        let service = RegistryService::new(store);
        let mut cli = BTreeMap::new();
        cli.insert(
            "gh-pr-list".to_string(),
            CliOperationDef {
                command: "a".to_string(),
                description: None,
            },
        );
        cli.insert(
            "gh-run-list".to_string(),
            CliOperationDef {
                command: "b".to_string(),
                description: None,
            },
        );
        let args = RulesArgs {
            name: Some("gh-pr-list".to_string()),
        };
        let mut buf = Vec::new();
        run_rules(&service, &cli, args, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("gh-pr-list"));
        assert!(!output.contains("gh-run-list"));
    }

    #[test]
    fn run_rules_cli_tool_not_found_returns_error() {
        let store = FakeConfigStore::new(GatewayConfig::default());
        let service = RegistryService::new(store);
        let args = RulesArgs {
            name: Some("nonexistent".to_string()),
        };
        let mut buf = Vec::new();
        let result = run_rules(&service, &BTreeMap::new(), args, &mut buf);
        assert!(matches!(result, Err(RegistryError::NotFound { .. })));
    }

    use crate::hexagon::entities::policy::allowlist::AllowlistPolicy;
    use crate::hexagon::entities::policy::compound::CompoundPolicy;
    use crate::hexagon::entities::policy::denylist::DenylistPolicy;
    use crate::hexagon::ports::OperationDescriptor;
    use crate::hexagon::usecases::gateway::test_helpers::{FailingUpstream, TestProvider};
    use crate::hexagon::usecases::gateway::ProviderHandle;

    type TestFilter = CompoundPolicy<AllowlistPolicy, DenylistPolicy>;

    fn provider_with_tools(names: &[&str]) -> TestProvider {
        TestProvider {
            operations: names
                .iter()
                .map(|n| OperationDescriptor {
                    name: n.to_string(),
                    description: None,
                    schema: "{}".to_string(),
                })
                .collect(),
            resources: vec![],
            templates: vec![],
            prompts: vec![],
        }
    }

    fn passthrough() -> TestFilter {
        CompoundPolicy::new(AllowlistPolicy::new(vec![]), DenylistPolicy::new(vec![]))
    }

    #[tokio::test]
    async fn run_tools_shows_allowed_tools_from_provider() {
        let mut providers = BTreeMap::new();
        providers.insert(
            "srv".to_string(),
            ProviderHandle {
                client: provider_with_tools(&["read", "write"]),
                filter: passthrough(),
            },
        );
        let mut buf = Vec::new();
        run_tools(&providers, None, &mut buf).await.unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("srv"));
        assert!(output.contains("ALLOW  read"));
        assert!(output.contains("→ srv__read"));
        assert!(output.contains("ALLOW  write"));
        assert!(output.contains("→ srv__write"));
    }

    #[tokio::test]
    async fn run_tools_shows_blocked_tools_when_denied() {
        let mut providers = BTreeMap::new();
        providers.insert(
            "srv".to_string(),
            ProviderHandle {
                client: provider_with_tools(&["read", "delete"]),
                filter: CompoundPolicy::new(
                    AllowlistPolicy::new(vec![]),
                    DenylistPolicy::new(vec!["delete".to_string()]),
                ),
            },
        );
        let mut buf = Vec::new();
        run_tools(&providers, None, &mut buf).await.unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("ALLOW  read"));
        assert!(output.contains("BLOCK  delete"));
        assert!(!output.contains("ALLOW  delete"));
    }

    #[tokio::test]
    async fn run_tools_shows_blocked_tools_when_not_in_allowlist() {
        let mut providers = BTreeMap::new();
        providers.insert(
            "srv".to_string(),
            ProviderHandle {
                client: provider_with_tools(&["read", "write"]),
                filter: CompoundPolicy::new(
                    AllowlistPolicy::new(vec!["read".to_string()]),
                    DenylistPolicy::new(vec![]),
                ),
            },
        );
        let mut buf = Vec::new();
        run_tools(&providers, None, &mut buf).await.unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("ALLOW  read"));
        assert!(output.contains("BLOCK  write"));
    }

    #[tokio::test]
    async fn run_tools_empty_providers_writes_nothing() {
        let providers: BTreeMap<String, ProviderHandle<TestProvider, TestFilter>> = BTreeMap::new();
        let mut buf = Vec::new();
        run_tools(&providers, None, &mut buf).await.unwrap();
        assert!(buf.is_empty());
    }

    #[tokio::test]
    async fn run_tools_filters_by_server_name() {
        let mut providers = BTreeMap::new();
        providers.insert(
            "alpha".to_string(),
            ProviderHandle {
                client: provider_with_tools(&["tool_a"]),
                filter: passthrough(),
            },
        );
        providers.insert(
            "beta".to_string(),
            ProviderHandle {
                client: provider_with_tools(&["tool_b"]),
                filter: passthrough(),
            },
        );
        let mut buf = Vec::new();
        run_tools(&providers, Some("alpha"), &mut buf)
            .await
            .unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("alpha"));
        assert!(output.contains("tool_a"));
        assert!(!output.contains("beta"));
    }

    #[tokio::test]
    async fn run_tools_server_not_found_returns_error() {
        let providers: BTreeMap<String, ProviderHandle<TestProvider, TestFilter>> = BTreeMap::new();
        let mut buf = Vec::new();
        let result = run_tools(&providers, Some("nope"), &mut buf).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn run_tools_shows_error_for_failing_provider() {
        let mut providers = BTreeMap::new();
        providers.insert(
            "bad".to_string(),
            ProviderHandle {
                client: FailingUpstream,
                filter: passthrough(),
            },
        );
        let mut buf = Vec::new();
        run_tools(&providers, None, &mut buf).await.unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("bad"));
        assert!(output.contains("ERROR"));
        assert!(output.contains("connection closed"));
    }

    #[tokio::test]
    async fn run_tools_shows_no_tools_for_empty_provider() {
        let mut providers = BTreeMap::new();
        providers.insert(
            "empty".to_string(),
            ProviderHandle {
                client: provider_with_tools(&[]),
                filter: passthrough(),
            },
        );
        let mut buf = Vec::new();
        run_tools(&providers, None, &mut buf).await.unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("empty"));
        assert!(output.contains("(no tools)"));
    }

    async fn failing_proxy(_entries: BTreeMap<String, McpServerEntry>) -> Result<(), ProxyError> {
        Err(ProxyError::UpstreamSpawn {
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "test"),
        })
    }

    async fn e2e_proxy(_entries: BTreeMap<String, McpServerEntry>) -> Result<(), ProxyError> {
        use crate::adapters::driven::connectivity::cli_execution::NullCliRunner;
        use crate::adapters::driven::connectivity::mcp_protocol::McpAdapter;
        use crate::adapters::driven::connectivity::mcp_protocol::RmcpProviderClient;
        use crate::hexagon::usecases::gateway::{Gateway, ProviderHandle};
        use rmcp::ServiceExt;

        let (upstream_server_t, upstream_client_t) = tokio::io::duplex(4096);
        let (downstream_server_t, downstream_client_t) = tokio::io::duplex(4096);

        let upstream_handle = tokio::spawn(async move {
            let s = MinimalServer.serve(upstream_server_t).await.unwrap();
            let _ = s.waiting().await;
        });

        let upstream = ().serve(upstream_client_t).await.unwrap();

        let mut upstreams = BTreeMap::new();
        upstreams.insert(
            "test".to_string(),
            ProviderHandle {
                client: RmcpProviderClient::new(upstream),
                filter: crate::hexagon::entities::policy::compound::CompoundPolicy::new(
                    crate::hexagon::entities::policy::allowlist::AllowlistPolicy::new(vec![]),
                    crate::hexagon::entities::policy::denylist::DenylistPolicy::new(vec![]),
                ),
            },
        );
        let gateway = Gateway::new(upstreams, NullCliRunner);
        let adapter = std::sync::Arc::new(McpAdapter::new(gateway));

        tokio::spawn(async move {
            let client = ().serve(downstream_client_t).await.unwrap();
            let tools = client.list_tools(None).await.unwrap();
            assert!(tools.tools.is_empty());
            drop(client);
        });

        let result = crate::adapters::driven::connectivity::mcp_protocol::proxy::serve_proxy(
            adapter,
            downstream_server_t,
        )
        .await;

        let _ = upstream_handle.await;
        result
    }
}

#[cfg(test)]
mod test_support {
    use rmcp::model::*;
    use rmcp::ServerHandler;

    pub(crate) struct MinimalServer;

    impl ServerHandler for MinimalServer {
        fn get_info(&self) -> ServerInfo {
            ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
        }

        async fn list_tools(
            &self,
            _request: Option<PaginatedRequestParams>,
            _context: rmcp::service::RequestContext<rmcp::RoleServer>,
        ) -> Result<ListToolsResult, rmcp::ErrorData> {
            Ok(ListToolsResult {
                tools: vec![],
                next_cursor: None,
                meta: None,
            })
        }
    }
}

#[cfg(test)]
use test_support::MinimalServer;
