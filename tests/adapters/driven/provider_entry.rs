use std::collections::BTreeMap;

use mcp_gateway::adapters::driven::configuration::model::{
    HttpConfig, McpServerEntry, StdioConfig,
};
use mcp_gateway::hexagon::ports::driven::provider_entry::ProviderEntry;

#[test]
fn allowed_tools_accessor_returns_correct_slice() {
    let stdio = McpServerEntry::Stdio(StdioConfig {
        command: "cmd".to_string(),
        args: vec![],
        env: BTreeMap::new(),
        allowed_operations: vec!["a".to_string()],
        denied_operations: vec![],
    });
    assert_eq!(ProviderEntry::allowed_operations(&stdio), &["a"]);

    let http = McpServerEntry::Http(HttpConfig {
        url: "https://x.com".to_string(),
        headers: BTreeMap::new(),
        allowed_operations: vec!["b".to_string(), "c".to_string()],
        denied_operations: vec![],
        auth: None,
    });
    assert_eq!(ProviderEntry::allowed_operations(&http), &["b", "c"]);
}

#[test]
fn allowed_tools_mut_modifies_stdio() {
    let mut entry = McpServerEntry::Stdio(StdioConfig {
        command: "cmd".to_string(),
        args: vec![],
        env: BTreeMap::new(),
        allowed_operations: vec![],
        denied_operations: vec![],
    });
    ProviderEntry::allowed_operations_mut(&mut entry).push("new_tool".to_string());
    assert_eq!(ProviderEntry::allowed_operations(&entry), &["new_tool"]);
}

#[test]
fn allowed_tools_mut_modifies_http() {
    let mut entry = McpServerEntry::Http(HttpConfig {
        url: "https://x.com".to_string(),
        headers: BTreeMap::new(),
        allowed_operations: vec!["existing".to_string()],
        denied_operations: vec![],
        auth: None,
    });
    ProviderEntry::allowed_operations_mut(&mut entry).push("another".to_string());
    assert_eq!(
        ProviderEntry::allowed_operations(&entry),
        &["existing", "another"]
    );
}

#[test]
fn denied_tools_accessor_returns_correct_slice() {
    let stdio = McpServerEntry::Stdio(StdioConfig {
        command: "cmd".to_string(),
        args: vec![],
        env: BTreeMap::new(),
        allowed_operations: vec![],
        denied_operations: vec!["a".to_string()],
    });
    assert_eq!(ProviderEntry::denied_operations(&stdio), &["a"]);

    let http = McpServerEntry::Http(HttpConfig {
        url: "https://x.com".to_string(),
        headers: BTreeMap::new(),
        allowed_operations: vec![],
        denied_operations: vec!["b".to_string(), "c".to_string()],
        auth: None,
    });
    assert_eq!(ProviderEntry::denied_operations(&http), &["b", "c"]);
}

#[test]
fn denied_tools_mut_modifies_stdio() {
    let mut entry = McpServerEntry::Stdio(StdioConfig {
        command: "cmd".to_string(),
        args: vec![],
        env: BTreeMap::new(),
        allowed_operations: vec![],
        denied_operations: vec![],
    });
    ProviderEntry::denied_operations_mut(&mut entry).push("dangerous".to_string());
    assert_eq!(ProviderEntry::denied_operations(&entry), &["dangerous"]);
}

#[test]
fn denied_tools_mut_modifies_http() {
    let mut entry = McpServerEntry::Http(HttpConfig {
        url: "https://x.com".to_string(),
        headers: BTreeMap::new(),
        allowed_operations: vec![],
        denied_operations: vec!["existing".to_string()],
        auth: None,
    });
    ProviderEntry::denied_operations_mut(&mut entry).push("another".to_string());
    assert_eq!(
        ProviderEntry::denied_operations(&entry),
        &["existing", "another"]
    );
}
