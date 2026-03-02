use crate::filter::ToolFilter;

pub struct CompoundFilter<A: ToolFilter, D: ToolFilter> {
    allow: A,
    deny: D,
}

impl<A: ToolFilter, D: ToolFilter> CompoundFilter<A, D> {
    pub fn new(allow: A, deny: D) -> Self {
        Self { allow, deny }
    }
}

impl<A: ToolFilter, D: ToolFilter> ToolFilter for CompoundFilter<A, D> {
    fn is_tool_allowed(&self, tool_name: &str) -> bool {
        self.allow.is_tool_allowed(tool_name) && self.deny.is_tool_allowed(tool_name)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::filter::{AllowlistFilter, DenylistFilter};

    #[test]
    fn both_empty_allows_all() {
        let filter = CompoundFilter::new(AllowlistFilter::new(vec![]), DenylistFilter::new(vec![]));
        assert!(filter.is_tool_allowed("anything"));
    }

    #[test]
    fn allowlist_only_filters() {
        let filter = CompoundFilter::new(
            AllowlistFilter::new(vec!["read".to_string()]),
            DenylistFilter::new(vec![]),
        );
        assert!(filter.is_tool_allowed("read"));
        assert!(!filter.is_tool_allowed("write"));
    }

    #[test]
    fn denylist_only_filters() {
        let filter = CompoundFilter::new(
            AllowlistFilter::new(vec![]),
            DenylistFilter::new(vec!["write".to_string()]),
        );
        assert!(filter.is_tool_allowed("read"));
        assert!(!filter.is_tool_allowed("write"));
    }

    #[test]
    fn denylist_takes_precedence_over_allowlist() {
        let filter = CompoundFilter::new(
            AllowlistFilter::new(vec!["read".to_string(), "write".to_string()]),
            DenylistFilter::new(vec!["write".to_string()]),
        );
        assert!(filter.is_tool_allowed("read"));
        assert!(!filter.is_tool_allowed("write"));
    }

    #[test]
    fn denied_tool_blocked_even_if_allowed() {
        let filter = CompoundFilter::new(
            AllowlistFilter::new(vec!["dangerous".to_string()]),
            DenylistFilter::new(vec!["dangerous".to_string()]),
        );
        assert!(!filter.is_tool_allowed("dangerous"));
    }
}
