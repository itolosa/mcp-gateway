use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

fn is_false(v: &bool) -> bool {
    !v
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatewayConfig {
    #[serde(default, rename = "mcpServers")]
    pub mcp_servers: BTreeMap<String, McpServerEntry>,
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        rename = "cliTools"
    )]
    pub cli_operations: BTreeMap<String, CliOperationDef>,
    #[serde(default, skip_serializing_if = "is_false", rename = "singleInstance")]
    pub single_instance: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliOperationDef {
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum McpServerEntry {
    Stdio(StdioConfig),
    Http(HttpConfig),
}

impl McpServerEntry {
    pub fn allowed_operations(&self) -> &[String] {
        match self {
            McpServerEntry::Stdio(c) => &c.allowed_operations,
            McpServerEntry::Http(c) => &c.allowed_operations,
        }
    }

    pub fn allowed_operations_mut(&mut self) -> &mut Vec<String> {
        match self {
            McpServerEntry::Stdio(c) => &mut c.allowed_operations,
            McpServerEntry::Http(c) => &mut c.allowed_operations,
        }
    }

    pub fn denied_operations(&self) -> &[String] {
        match self {
            McpServerEntry::Stdio(c) => &c.denied_operations,
            McpServerEntry::Http(c) => &c.denied_operations,
        }
    }

    pub fn denied_operations_mut(&mut self) -> &mut Vec<String> {
        match self {
            McpServerEntry::Stdio(c) => &mut c.denied_operations,
            McpServerEntry::Http(c) => &mut c.denied_operations,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StdioConfig {
    pub command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        rename = "allowedTools"
    )]
    pub allowed_operations: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty", rename = "deniedTools")]
    pub denied_operations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpConfig {
    pub url: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        rename = "allowedTools"
    )]
    pub allowed_operations: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty", rename = "deniedTools")]
    pub denied_operations: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<OAuthConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
    #[serde(default = "default_redirect_port")]
    pub redirect_port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credentials_file: Option<String>,
}

impl Default for OAuthConfig {
    fn default() -> Self {
        Self {
            client_id: None,
            client_secret: None,
            scopes: vec![],
            redirect_port: default_redirect_port(),
            credentials_file: None,
        }
    }
}

fn default_redirect_port() -> u16 {
    9876
}

impl crate::hexagon::ports::ProviderEntry for McpServerEntry {
    fn allowed_operations(&self) -> &[String] {
        McpServerEntry::allowed_operations(self)
    }

    fn allowed_operations_mut(&mut self) -> &mut Vec<String> {
        McpServerEntry::allowed_operations_mut(self)
    }

    fn denied_operations(&self) -> &[String] {
        McpServerEntry::denied_operations(self)
    }

    fn denied_operations_mut(&mut self) -> &mut Vec<String> {
        McpServerEntry::denied_operations_mut(self)
    }
}
