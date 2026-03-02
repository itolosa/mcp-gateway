use std::collections::BTreeMap;
use std::future::Future;
use std::io::Write;

use crate::cli::command::{
    AddArgs, AllowlistModifyArgs, AllowlistShowArgs, DenylistModifyArgs, DenylistShowArgs,
    RemoveArgs, TransportType,
};
use crate::config::model::{HttpConfig, McpServerEntry, StdioConfig};
use crate::config::store::ConfigStore;
use crate::proxy::error::ProxyError;
use crate::registry::error::RegistryError;
use crate::registry::service::RegistryService;

pub fn run_list<S: ConfigStore>(
    service: &RegistryService<S>,
    out: &mut impl Write,
) -> Result<(), RegistryError> {
    let servers = service.list_servers()?;
    if servers.is_empty() {
        return Ok(());
    }
    let _ = writeln!(out, "{:<20} {:<10} TARGET", "NAME", "TYPE");
    for (name, entry) in &servers {
        let (server_type, target) = describe_entry(entry);
        let _ = writeln!(out, "{:<20} {:<10} {}", name, server_type, target);
    }
    Ok(())
}

fn describe_entry(entry: &McpServerEntry) -> (&str, &str) {
    match entry {
        McpServerEntry::Stdio(config) => ("stdio", &config.command),
        McpServerEntry::Http(config) => ("http", &config.url),
    }
}

pub async fn run_run<S, F, Fut>(
    service: &RegistryService<S>,
    run_proxy: F,
) -> Result<(), ProxyError>
where
    S: ConfigStore,
    F: FnOnce(BTreeMap<String, McpServerEntry>) -> Fut,
    Fut: Future<Output = Result<(), ProxyError>>,
{
    let servers = service.list_servers()?;
    run_proxy(servers).await
}

pub fn run_remove<S: ConfigStore>(
    service: &RegistryService<S>,
    args: RemoveArgs,
) -> Result<(), RegistryError> {
    service.remove_server(&args.name)
}

pub fn run_allowlist_add<S: ConfigStore>(
    service: &RegistryService<S>,
    args: AllowlistModifyArgs,
) -> Result<(), RegistryError> {
    service.add_allowed_tools(&args.name, &args.tools)
}

pub fn run_allowlist_remove<S: ConfigStore>(
    service: &RegistryService<S>,
    args: AllowlistModifyArgs,
) -> Result<(), RegistryError> {
    service.remove_allowed_tools(&args.name, &args.tools)
}

pub fn run_allowlist_show<S: ConfigStore>(
    service: &RegistryService<S>,
    args: AllowlistShowArgs,
    out: &mut impl Write,
) -> Result<(), RegistryError> {
    let tools = service.get_allowed_tools(&args.name)?;
    for tool in &tools {
        let _ = writeln!(out, "{tool}");
    }
    Ok(())
}

pub fn run_denylist_add<S: ConfigStore>(
    service: &RegistryService<S>,
    args: DenylistModifyArgs,
) -> Result<(), RegistryError> {
    service.add_denied_tools(&args.name, &args.tools)
}

pub fn run_denylist_remove<S: ConfigStore>(
    service: &RegistryService<S>,
    args: DenylistModifyArgs,
) -> Result<(), RegistryError> {
    service.remove_denied_tools(&args.name, &args.tools)
}

pub fn run_denylist_show<S: ConfigStore>(
    service: &RegistryService<S>,
    args: DenylistShowArgs,
    out: &mut impl Write,
) -> Result<(), RegistryError> {
    let tools = service.get_denied_tools(&args.name)?;
    for tool in &tools {
        let _ = writeln!(out, "{tool}");
    }
    Ok(())
}

pub fn run_add<S: ConfigStore>(
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
    service.add_server(args.name, entry)
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
            allowed_tools: vec![],
            denied_tools: vec![],
        }),
        TransportType::Http => McpServerEntry::Http(HttpConfig {
            url: url.unwrap_or_default(),
            headers: headers.into_iter().collect::<BTreeMap<_, _>>(),
            allowed_tools: vec![],
            denied_tools: vec![],
            auth: None,
        }),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::config::error::ConfigError;
    use crate::config::model::GatewayConfig;
    use std::cell::RefCell;
    use std::path::PathBuf;

    struct FakeConfigStore {
        config: RefCell<GatewayConfig>,
        fail_load: bool,
    }

    impl FakeConfigStore {
        fn new(config: GatewayConfig) -> Self {
            Self {
                config: RefCell::new(config),
                fail_load: false,
            }
        }

        fn failing() -> Self {
            Self {
                config: RefCell::new(GatewayConfig::default()),
                fail_load: true,
            }
        }
    }

    fn io_error() -> ConfigError {
        ConfigError::Io {
            path: PathBuf::from("/fail"),
            source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
        }
    }

    impl ConfigStore for FakeConfigStore {
        fn load(&self) -> Result<GatewayConfig, ConfigError> {
            if self.fail_load {
                return Err(io_error());
            }
            Ok(self.config.borrow().clone())
        }

        fn save(&self, config: &GatewayConfig) -> Result<(), ConfigError> {
            *self.config.borrow_mut() = config.clone();
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
                allowed_tools: vec![],
                denied_tools: vec![],
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

        let config = service.store().load().unwrap();
        let entry = config.mcp_servers.get("test").unwrap();
        assert_eq!(
            entry,
            &McpServerEntry::Stdio(StdioConfig {
                command: "node".to_string(),
                args: vec!["server.js".to_string()],
                env: BTreeMap::from([("KEY".to_string(), "val".to_string())]),
                allowed_tools: vec![],
                denied_tools: vec![],
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

        let config = service.store().load().unwrap();
        let entry = config.mcp_servers.get("remote").unwrap();
        assert_eq!(
            entry,
            &McpServerEntry::Http(HttpConfig {
                url: "https://example.com".to_string(),
                headers: BTreeMap::from([("Auth".to_string(), "tok".to_string())]),
                allowed_tools: vec![],
                denied_tools: vec![],
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
                allowed_tools: vec![],
                denied_tools: vec![],
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
                allowed_tools: vec![],
                denied_tools: vec![],
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
                allowed_tools: vec![],
                denied_tools: vec![],
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
            allowed_tools: vec![],
            denied_tools: vec![],
        });
        assert_eq!(describe_entry(&entry), ("stdio", "node"));
    }

    #[test]
    fn describe_http_entry_returns_type_and_url() {
        let entry = McpServerEntry::Http(HttpConfig {
            url: "https://example.com".to_string(),
            headers: BTreeMap::new(),
            allowed_tools: vec![],
            denied_tools: vec![],
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
                allowed_tools: vec![],
                denied_tools: vec![],
            }),
        );
        config.mcp_servers.insert(
            "remote".to_string(),
            McpServerEntry::Http(HttpConfig {
                url: "https://example.com".to_string(),
                headers: BTreeMap::new(),
                allowed_tools: vec![],
                denied_tools: vec![],
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
        assert!(matches!(result, Err(RegistryError::Config(_))));
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
                allowed_tools: vec![],
                denied_tools: vec![],
            }),
        );
        let store = FakeConfigStore::new(config);
        let service = RegistryService::new(store);

        let args = RemoveArgs {
            name: "target".to_string(),
        };
        run_remove(&service, args).unwrap();

        let config = service.store().load().unwrap();
        assert!(!config.mcp_servers.contains_key("target"));
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

        let config = service.store().load().unwrap();
        let entry = config.mcp_servers.get("test").unwrap();
        assert_eq!(entry.allowed_tools(), &["read", "write"]);
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
                allowed_tools: vec!["read".to_string(), "write".to_string()],
                denied_tools: vec![],
            }),
        );
        let store = FakeConfigStore::new(config);
        let service = RegistryService::new(store);

        let args = AllowlistModifyArgs {
            name: "test".to_string(),
            tools: vec!["read".to_string()],
        };
        run_allowlist_remove(&service, args).unwrap();

        let config = service.store().load().unwrap();
        let entry = config.mcp_servers.get("test").unwrap();
        assert_eq!(entry.allowed_tools(), &["write"]);
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
                allowed_tools: vec!["read".to_string(), "write".to_string()],
                denied_tools: vec![],
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
        assert!(matches!(result, Err(RegistryError::Config(_))));
    }

    #[test]
    fn run_allowlist_remove_store_error_propagates() {
        let service = RegistryService::new(FakeConfigStore::failing());
        let args = AllowlistModifyArgs {
            name: "test".to_string(),
            tools: vec!["read".to_string()],
        };
        let result = run_allowlist_remove(&service, args);
        assert!(matches!(result, Err(RegistryError::Config(_))));
    }

    #[test]
    fn run_allowlist_show_store_error_propagates() {
        let service = RegistryService::new(FakeConfigStore::failing());
        let args = AllowlistShowArgs {
            name: "test".to_string(),
        };
        let mut buf = Vec::new();
        let result = run_allowlist_show(&service, args, &mut buf);
        assert!(matches!(result, Err(RegistryError::Config(_))));
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

        let config = service.store().load().unwrap();
        let entry = config.mcp_servers.get("test").unwrap();
        assert_eq!(entry.denied_tools(), &["delete", "exec"]);
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
                allowed_tools: vec![],
                denied_tools: vec!["delete".to_string(), "exec".to_string()],
            }),
        );
        let store = FakeConfigStore::new(config);
        let service = RegistryService::new(store);

        let args = DenylistModifyArgs {
            name: "test".to_string(),
            tools: vec!["delete".to_string()],
        };
        run_denylist_remove(&service, args).unwrap();

        let config = service.store().load().unwrap();
        let entry = config.mcp_servers.get("test").unwrap();
        assert_eq!(entry.denied_tools(), &["exec"]);
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
                allowed_tools: vec![],
                denied_tools: vec!["delete".to_string(), "exec".to_string()],
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
        assert!(matches!(result, Err(RegistryError::Config(_))));
    }

    #[test]
    fn run_denylist_remove_store_error_propagates() {
        let service = RegistryService::new(FakeConfigStore::failing());
        let args = DenylistModifyArgs {
            name: "test".to_string(),
            tools: vec!["delete".to_string()],
        };
        let result = run_denylist_remove(&service, args);
        assert!(matches!(result, Err(RegistryError::Config(_))));
    }

    #[test]
    fn run_denylist_show_store_error_propagates() {
        let service = RegistryService::new(FakeConfigStore::failing());
        let args = DenylistShowArgs {
            name: "test".to_string(),
        };
        let mut buf = Vec::new();
        let result = run_denylist_show(&service, args, &mut buf);
        assert!(matches!(result, Err(RegistryError::Config(_))));
    }

    async fn failing_proxy(_entries: BTreeMap<String, McpServerEntry>) -> Result<(), ProxyError> {
        Err(ProxyError::UpstreamSpawn {
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "test"),
        })
    }

    async fn e2e_proxy(_entries: BTreeMap<String, McpServerEntry>) -> Result<(), ProxyError> {
        use crate::proxy::handler::{ProxyHandler, UpstreamEntry};
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
            UpstreamEntry {
                service: upstream,
                filter: crate::filter::CompoundFilter::new(
                    crate::filter::AllowlistFilter::new(vec![]),
                    crate::filter::DenylistFilter::new(vec![]),
                ),
            },
        );
        let handler = ProxyHandler::new(upstreams, None);

        tokio::spawn(async move {
            let client = ().serve(downstream_client_t).await.unwrap();
            let tools = client.list_tools(None).await.unwrap();
            assert!(tools.tools.is_empty());
            drop(client);
        });

        let result = crate::proxy::runner::serve_proxy(handler, downstream_server_t).await;

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
            ServerInfo {
                capabilities: ServerCapabilities::builder().enable_tools().build(),
                ..Default::default()
            }
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
