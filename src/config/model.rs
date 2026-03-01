use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatewayConfig {
    #[serde(default, rename = "mcpServers")]
    pub mcp_servers: BTreeMap<String, McpServerEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum McpServerEntry {
    Stdio(StdioConfig),
    Http(HttpConfig),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StdioConfig {
    pub command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpConfig {
    pub url: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_empty_json_gives_empty_config() {
        let config: GatewayConfig = serde_json::from_str("{}").unwrap();
        assert!(config.mcp_servers.is_empty());
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
            }),
        );
        config.mcp_servers.insert(
            "h1".to_string(),
            McpServerEntry::Http(HttpConfig {
                url: "https://x.com".to_string(),
                headers: BTreeMap::new(),
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
        });
        let json = serde_json::to_string(&entry).unwrap();
        assert!(!json.contains("args"));
        assert!(!json.contains("env"));
    }

    #[test]
    fn http_omits_empty_headers() {
        let entry = McpServerEntry::Http(HttpConfig {
            url: "https://x.com".to_string(),
            headers: BTreeMap::new(),
        });
        let json = serde_json::to_string(&entry).unwrap();
        assert!(!json.contains("headers"));
    }
}
