use std::collections::BTreeMap;
use std::path::PathBuf;

use mcp_gateway::adapters::driven::configuration::default_config_path;
use mcp_gateway::adapters::driven::configuration::error::ConfigError;
use mcp_gateway::adapters::driven::configuration::model::{
    CliOperationDef, GatewayConfig, HttpConfig, McpServerEntry, OAuthConfig, StdioConfig,
};

// -- default_config_path tests (from configuration/mod.rs) --

#[test]
fn default_config_path_ends_with_expected_filename() {
    let path = default_config_path();
    assert!(path.is_some());
    let path = path.unwrap_or_default();
    assert!(path.ends_with(".mcp-gateway.json"));
}

// -- ConfigError tests (from configuration/error.rs) --

#[test]
fn io_error_display_contains_path() {
    let err = ConfigError::Io {
        path: PathBuf::from("/tmp/test.json"),
        source: std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
    };
    let msg = err.to_string();
    assert!(msg.contains("/tmp/test.json"));
    assert!(msg.contains("not found"));
}

#[test]
fn parse_error_display_contains_path() {
    let serde_err = serde_json::from_str::<serde_json::Value>("bad").unwrap_err();
    let err = ConfigError::Parse {
        path: PathBuf::from("/tmp/bad.json"),
        source: serde_err,
    };
    let msg = err.to_string();
    assert!(msg.contains("/tmp/bad.json"));
}

// -- GatewayConfig model tests (from configuration/model.rs) --

#[test]
fn deserialize_empty_json_gives_empty_config() {
    let config: GatewayConfig = serde_json::from_str("{}").unwrap();
    assert!(config.mcp_servers.is_empty());
    assert!(!config.single_instance);
}

#[test]
fn deserialize_stdio_server() {
    let json = r#"{
        "mcpServers": {
            "test": {
                "type": "stdio",
                "command": "node",
                "args": ["server.js"],
                "env": {"KEY": "val"}
            }
        }
    }"#;
    let config: GatewayConfig = serde_json::from_str(json).unwrap();
    assert_eq!(
        config.mcp_servers.get("test").unwrap(),
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
fn deserialize_http_server() {
    let json = r#"{
        "mcpServers": {
            "remote": {
                "type": "http",
                "url": "https://example.com/mcp",
                "headers": {"Authorization": "Bearer tok"}
            }
        }
    }"#;
    let config: GatewayConfig = serde_json::from_str(json).unwrap();
    assert_eq!(
        config.mcp_servers.get("remote").unwrap(),
        &McpServerEntry::Http(HttpConfig {
            url: "https://example.com/mcp".to_string(),
            headers: BTreeMap::from([("Authorization".to_string(), "Bearer tok".to_string())]),
            allowed_operations: vec![],
            denied_operations: vec![],
            auth: None,
        })
    );
}

#[test]
fn serialize_deserialize_roundtrip() {
    let mut config = GatewayConfig::default();
    config.mcp_servers.insert(
        "s1".to_string(),
        McpServerEntry::Stdio(StdioConfig {
            command: "cmd".to_string(),
            args: vec!["a".to_string()],
            env: BTreeMap::from([("K".to_string(), "V".to_string())]),
            allowed_operations: vec![],
            denied_operations: vec![],
        }),
    );
    config.mcp_servers.insert(
        "h1".to_string(),
        McpServerEntry::Http(HttpConfig {
            url: "https://x.com".to_string(),
            headers: BTreeMap::new(),
            allowed_operations: vec![],
            denied_operations: vec![],
            auth: None,
        }),
    );

    let json = serde_json::to_string(&config).unwrap();
    let roundtrip: GatewayConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(roundtrip, config);
}

#[test]
fn stdio_omits_empty_args_and_env() {
    let entry = McpServerEntry::Stdio(StdioConfig {
        command: "echo".to_string(),
        args: vec![],
        env: BTreeMap::new(),
        allowed_operations: vec![],
        denied_operations: vec![],
    });
    let json = serde_json::to_string(&entry).unwrap();
    assert!(!json.contains("args"));
    assert!(!json.contains("env"));
    assert!(!json.contains("allowedTools"));
}

#[test]
fn http_omits_empty_headers() {
    let entry = McpServerEntry::Http(HttpConfig {
        url: "https://x.com".to_string(),
        headers: BTreeMap::new(),
        allowed_operations: vec![],
        denied_operations: vec![],
        auth: None,
    });
    let json = serde_json::to_string(&entry).unwrap();
    assert!(!json.contains("headers"));
    assert!(!json.contains("allowedTools"));
}

#[test]
fn stdio_serializes_allowed_tools_as_camel_case() {
    let entry = McpServerEntry::Stdio(StdioConfig {
        command: "echo".to_string(),
        args: vec![],
        env: BTreeMap::new(),
        allowed_operations: vec!["tool_a".to_string()],
        denied_operations: vec![],
    });
    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("allowedTools"));
    assert!(json.contains("tool_a"));
}

#[test]
fn http_deserializes_allowed_tools() {
    let json = r#"{
        "mcpServers": {
            "remote": {
                "type": "http",
                "url": "https://example.com/mcp",
                "allowedTools": ["read", "search"]
            }
        }
    }"#;
    let config: GatewayConfig = serde_json::from_str(json).unwrap();
    let entry = config.mcp_servers.get("remote").unwrap();
    assert_eq!(entry.allowed_operations(), &["read", "search"]);
}

#[test]
fn stdio_serializes_denied_tools_as_camel_case() {
    let entry = McpServerEntry::Stdio(StdioConfig {
        command: "echo".to_string(),
        args: vec![],
        env: BTreeMap::new(),
        allowed_operations: vec![],
        denied_operations: vec!["dangerous".to_string()],
    });
    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("deniedTools"));
    assert!(json.contains("dangerous"));
}

#[test]
fn stdio_omits_empty_denied_tools() {
    let entry = McpServerEntry::Stdio(StdioConfig {
        command: "echo".to_string(),
        args: vec![],
        env: BTreeMap::new(),
        allowed_operations: vec![],
        denied_operations: vec![],
    });
    let json = serde_json::to_string(&entry).unwrap();
    assert!(!json.contains("deniedTools"));
}

#[test]
fn http_deserializes_denied_tools() {
    let json = r#"{
        "mcpServers": {
            "remote": {
                "type": "http",
                "url": "https://example.com/mcp",
                "deniedTools": ["delete", "exec"]
            }
        }
    }"#;
    let config: GatewayConfig = serde_json::from_str(json).unwrap();
    let entry = config.mcp_servers.get("remote").unwrap();
    assert_eq!(entry.denied_operations(), &["delete", "exec"]);
}

#[test]
fn http_omits_empty_denied_tools() {
    let entry = McpServerEntry::Http(HttpConfig {
        url: "https://x.com".to_string(),
        headers: BTreeMap::new(),
        allowed_operations: vec![],
        denied_operations: vec![],
        auth: None,
    });
    let json = serde_json::to_string(&entry).unwrap();
    assert!(!json.contains("deniedTools"));
}

#[test]
fn roundtrip_with_denied_tools() {
    let mut config = GatewayConfig::default();
    config.mcp_servers.insert(
        "s1".to_string(),
        McpServerEntry::Stdio(StdioConfig {
            command: "cmd".to_string(),
            args: vec![],
            env: BTreeMap::new(),
            allowed_operations: vec!["read".to_string()],
            denied_operations: vec!["delete".to_string()],
        }),
    );

    let json = serde_json::to_string(&config).unwrap();
    let roundtrip: GatewayConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(roundtrip, config);
}

#[test]
fn deserialize_cli_tools() {
    let json = r#"{
        "cliTools": {
            "gh-pr": {
                "command": "/path/to/gh-pr.sh",
                "description": "List pull requests"
            }
        }
    }"#;
    let config: GatewayConfig = serde_json::from_str(json).unwrap();
    let tool = config.cli_operations.get("gh-pr").unwrap();
    assert_eq!(tool.command, "/path/to/gh-pr.sh");
    assert_eq!(tool.description.as_deref(), Some("List pull requests"));
}

#[test]
fn cli_tools_roundtrip() {
    let mut config = GatewayConfig::default();
    config.cli_operations.insert(
        "docker-ps".to_string(),
        CliOperationDef {
            command: "/scripts/docker-ps.sh".to_string(),
            description: None,
        },
    );
    let json = serde_json::to_string(&config).unwrap();
    let roundtrip: GatewayConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(roundtrip, config);
}

#[test]
fn empty_cli_tools_omitted_from_json() {
    let config = GatewayConfig::default();
    let json = serde_json::to_string(&config).unwrap();
    assert!(!json.contains("cliTools"));
}

#[test]
fn single_instance_defaults_to_false() {
    let config: GatewayConfig = serde_json::from_str("{}").unwrap();
    assert!(!config.single_instance);
}

#[test]
fn single_instance_true_deserializes() {
    let json = r#"{"singleInstance": true}"#;
    let config: GatewayConfig = serde_json::from_str(json).unwrap();
    assert!(config.single_instance);
}

#[test]
fn single_instance_false_omitted_from_json() {
    let config = GatewayConfig::default();
    let json = serde_json::to_string(&config).unwrap();
    assert!(!json.contains("singleInstance"));
}

#[test]
fn single_instance_true_included_in_json() {
    let config = GatewayConfig {
        single_instance: true,
        ..Default::default()
    };
    let json = serde_json::to_string(&config).unwrap();
    assert!(json.contains("singleInstance"));
}

#[test]
fn single_instance_roundtrip() {
    let config = GatewayConfig {
        single_instance: true,
        ..Default::default()
    };
    let json = serde_json::to_string(&config).unwrap();
    let roundtrip: GatewayConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(roundtrip, config);
}

#[test]
fn cli_tool_def_omits_none_description() {
    let def = CliOperationDef {
        command: "echo".to_string(),
        description: None,
    };
    let json = serde_json::to_string(&def).unwrap();
    assert!(!json.contains("description"));
}

#[test]
fn http_omits_none_auth() {
    let entry = McpServerEntry::Http(HttpConfig {
        url: "https://x.com".to_string(),
        headers: BTreeMap::new(),
        allowed_operations: vec![],
        denied_operations: vec![],
        auth: None,
    });
    let json = serde_json::to_string(&entry).unwrap();
    assert!(!json.contains("auth"));
}

#[test]
fn deserialize_http_with_oauth_config() {
    let json = r#"{
        "mcpServers": {
            "remote": {
                "type": "http",
                "url": "https://example.com/mcp",
                "auth": {
                    "clientId": "my-app",
                    "scopes": ["read", "write"],
                    "redirectPort": 8080
                }
            }
        }
    }"#;
    let config: GatewayConfig = serde_json::from_str(json).unwrap();
    let entry = config.mcp_servers.get("remote").unwrap();
    assert!(matches!(
        entry,
        McpServerEntry::Http(http) if http.auth.as_ref().is_some_and(|a| {
            a.client_id.as_deref() == Some("my-app")
                && a.client_secret.is_none()
                && a.scopes == vec!["read", "write"]
                && a.redirect_port == 8080
                && a.credentials_file.is_none()
        })
    ));
}

#[test]
fn oauth_config_default_redirect_port() {
    let json = r#"{"scopes": []}"#;
    let config: OAuthConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.redirect_port, 9876);
}

#[test]
fn oauth_config_default_uses_standard_redirect_port() {
    let config = OAuthConfig::default();
    assert_eq!(config.redirect_port, 9876);
    assert!(config.client_id.is_none());
    assert!(config.client_secret.is_none());
    assert!(config.scopes.is_empty());
    assert!(config.credentials_file.is_none());
}

#[test]
fn oauth_config_roundtrip() {
    let config = OAuthConfig {
        client_id: Some("app".to_string()),
        client_secret: Some("secret".to_string()),
        scopes: vec!["read".to_string()],
        redirect_port: 7777,
        credentials_file: Some("/tmp/creds.json".to_string()),
    };
    let json = serde_json::to_string(&config).unwrap();
    let roundtrip: OAuthConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(roundtrip, config);
}

#[test]
fn oauth_config_omits_empty_fields() {
    let config = OAuthConfig {
        client_id: None,
        client_secret: None,
        scopes: vec![],
        redirect_port: 9876,
        credentials_file: None,
    };
    let json = serde_json::to_string(&config).unwrap();
    assert!(!json.contains("clientId"));
    assert!(!json.contains("clientSecret"));
    assert!(!json.contains("scopes"));
    assert!(!json.contains("credentialsFile"));
    assert!(json.contains("redirectPort"));
}

#[test]
fn oauth_config_camel_case_serialization() {
    let config = OAuthConfig {
        client_id: Some("id".to_string()),
        client_secret: Some("sec".to_string()),
        scopes: vec!["s".to_string()],
        redirect_port: 1234,
        credentials_file: Some("/f".to_string()),
    };
    let json = serde_json::to_string(&config).unwrap();
    assert!(json.contains("clientId"));
    assert!(json.contains("clientSecret"));
    assert!(json.contains("redirectPort"));
    assert!(json.contains("credentialsFile"));
}

#[test]
fn http_with_auth_roundtrip() {
    let mut config = GatewayConfig::default();
    config.mcp_servers.insert(
        "remote".to_string(),
        McpServerEntry::Http(HttpConfig {
            url: "https://example.com/mcp".to_string(),
            headers: BTreeMap::new(),
            allowed_operations: vec![],
            denied_operations: vec![],
            auth: Some(OAuthConfig {
                client_id: Some("app".to_string()),
                client_secret: None,
                scopes: vec!["read".to_string()],
                redirect_port: 9876,
                credentials_file: None,
            }),
        }),
    );
    let json = serde_json::to_string(&config).unwrap();
    let roundtrip: GatewayConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(roundtrip, config);
}
