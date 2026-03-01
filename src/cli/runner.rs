use std::collections::BTreeMap;

use crate::cli::command::{AddArgs, TransportType};
use crate::config::model::{HttpConfig, McpServerEntry, StdioConfig};
use crate::config::store::ConfigStore;
use crate::registry::error::RegistryError;
use crate::registry::service::RegistryService;

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
        }),
        TransportType::Http => McpServerEntry::Http(HttpConfig {
            url: url.unwrap_or_default(),
            headers: headers.into_iter().collect::<BTreeMap<_, _>>(),
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

    struct FakeConfigStore {
        config: RefCell<GatewayConfig>,
    }

    impl FakeConfigStore {
        fn new(config: GatewayConfig) -> Self {
            Self {
                config: RefCell::new(config),
            }
        }
    }

    impl ConfigStore for FakeConfigStore {
        fn load(&self) -> Result<GatewayConfig, ConfigError> {
            Ok(self.config.borrow().clone())
        }

        fn save(&self, config: &GatewayConfig) -> Result<(), ConfigError> {
            *self.config.borrow_mut() = config.clone();
            Ok(())
        }
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
            })
        );
    }
}
