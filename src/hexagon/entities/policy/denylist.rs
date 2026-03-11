use std::collections::HashSet;

use crate::hexagon::ports::OperationPolicy;

pub struct DenylistPolicy {
    denied: HashSet<String>,
}

impl DenylistPolicy {
    pub fn new(tools: Vec<String>) -> Self {
        Self {
            denied: tools.into_iter().collect(),
        }
    }
}

impl OperationPolicy for DenylistPolicy {
    fn is_allowed(&self, tool_name: &str) -> bool {
        !self.denied.contains(tool_name)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn empty_denylist_allows_all_tools() {
        let filter = DenylistPolicy::new(vec![]);
        assert!(filter.is_allowed("anything"));
        assert!(filter.is_allowed("another_tool"));
    }

    #[test]
    fn non_empty_denylist_blocks_listed_tools() {
        let filter = DenylistPolicy::new(vec!["write".to_string(), "delete".to_string()]);
        assert!(filter.is_allowed("read"));
        assert!(!filter.is_allowed("write"));
        assert!(!filter.is_allowed("delete"));
    }

    #[test]
    fn single_tool_denylist() {
        let filter = DenylistPolicy::new(vec!["blocked".to_string()]);
        assert!(!filter.is_allowed("blocked"));
        assert!(filter.is_allowed("allowed"));
    }
}
