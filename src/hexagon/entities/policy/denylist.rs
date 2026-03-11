use std::collections::HashSet;

use crate::hexagon::ports::ToolFilter;

pub struct DenylistFilter {
    denied: HashSet<String>,
}

impl DenylistFilter {
    pub fn new(tools: Vec<String>) -> Self {
        Self {
            denied: tools.into_iter().collect(),
        }
    }
}

impl ToolFilter for DenylistFilter {
    fn is_tool_allowed(&self, tool_name: &str) -> bool {
        !self.denied.contains(tool_name)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn empty_denylist_allows_all_tools() {
        let filter = DenylistFilter::new(vec![]);
        assert!(filter.is_tool_allowed("anything"));
        assert!(filter.is_tool_allowed("another_tool"));
    }

    #[test]
    fn non_empty_denylist_blocks_listed_tools() {
        let filter = DenylistFilter::new(vec!["write".to_string(), "delete".to_string()]);
        assert!(filter.is_tool_allowed("read"));
        assert!(!filter.is_tool_allowed("write"));
        assert!(!filter.is_tool_allowed("delete"));
    }

    #[test]
    fn single_tool_denylist() {
        let filter = DenylistFilter::new(vec!["blocked".to_string()]);
        assert!(!filter.is_tool_allowed("blocked"));
        assert!(filter.is_tool_allowed("allowed"));
    }
}
