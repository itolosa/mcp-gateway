use std::collections::HashSet;

use crate::hexagon::ports::OperationPolicy;

pub struct AllowlistPolicy {
    allowed: HashSet<String>,
}

impl AllowlistPolicy {
    pub fn new(tools: Vec<String>) -> Self {
        Self {
            allowed: tools.into_iter().collect(),
        }
    }
}

impl OperationPolicy for AllowlistPolicy {
    fn is_allowed(&self, tool_name: &str) -> bool {
        self.allowed.is_empty() || self.allowed.contains(tool_name)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn empty_allowlist_allows_all_tools() {
        let filter = AllowlistPolicy::new(vec![]);
        assert!(filter.is_allowed("anything"));
        assert!(filter.is_allowed("another_tool"));
    }

    #[test]
    fn non_empty_allowlist_allows_only_listed_tools() {
        let filter = AllowlistPolicy::new(vec!["read".to_string(), "search".to_string()]);
        assert!(filter.is_allowed("read"));
        assert!(filter.is_allowed("search"));
        assert!(!filter.is_allowed("write"));
        assert!(!filter.is_allowed("delete"));
    }

    #[test]
    fn single_tool_allowlist() {
        let filter = AllowlistPolicy::new(vec!["only_this".to_string()]);
        assert!(filter.is_allowed("only_this"));
        assert!(!filter.is_allowed("not_this"));
    }
}
