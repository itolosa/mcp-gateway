use std::collections::HashSet;

use crate::hexagon::ports::ToolFilter;

pub struct AllowlistFilter {
    allowed: HashSet<String>,
}

impl AllowlistFilter {
    pub fn new(tools: Vec<String>) -> Self {
        Self {
            allowed: tools.into_iter().collect(),
        }
    }
}

impl ToolFilter for AllowlistFilter {
    fn is_tool_allowed(&self, tool_name: &str) -> bool {
        self.allowed.is_empty() || self.allowed.contains(tool_name)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn empty_allowlist_allows_all_tools() {
        let filter = AllowlistFilter::new(vec![]);
        assert!(filter.is_tool_allowed("anything"));
        assert!(filter.is_tool_allowed("another_tool"));
    }

    #[test]
    fn non_empty_allowlist_allows_only_listed_tools() {
        let filter = AllowlistFilter::new(vec!["read".to_string(), "search".to_string()]);
        assert!(filter.is_tool_allowed("read"));
        assert!(filter.is_tool_allowed("search"));
        assert!(!filter.is_tool_allowed("write"));
        assert!(!filter.is_tool_allowed("delete"));
    }

    #[test]
    fn single_tool_allowlist() {
        let filter = AllowlistFilter::new(vec!["only_this".to_string()]);
        assert!(filter.is_tool_allowed("only_this"));
        assert!(!filter.is_tool_allowed("not_this"));
    }
}
