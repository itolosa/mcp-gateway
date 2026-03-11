use crate::hexagon::ports::OperationPolicy;

pub struct CompoundPolicy<A: OperationPolicy, D: OperationPolicy> {
    allow: A,
    deny: D,
}

impl<A: OperationPolicy, D: OperationPolicy> CompoundPolicy<A, D> {
    pub fn new(allow: A, deny: D) -> Self {
        Self { allow, deny }
    }
}

impl<A: OperationPolicy, D: OperationPolicy> OperationPolicy for CompoundPolicy<A, D> {
    fn is_allowed(&self, tool_name: &str) -> bool {
        self.allow.is_allowed(tool_name) && self.deny.is_allowed(tool_name)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::hexagon::entities::policy::allowlist::AllowlistPolicy;
    use crate::hexagon::entities::policy::denylist::DenylistPolicy;

    #[test]
    fn both_empty_allows_all() {
        let filter = CompoundPolicy::new(AllowlistPolicy::new(vec![]), DenylistPolicy::new(vec![]));
        assert!(filter.is_allowed("anything"));
    }

    #[test]
    fn allowlist_only_filters() {
        let filter = CompoundPolicy::new(
            AllowlistPolicy::new(vec!["read".to_string()]),
            DenylistPolicy::new(vec![]),
        );
        assert!(filter.is_allowed("read"));
        assert!(!filter.is_allowed("write"));
    }

    #[test]
    fn denylist_only_filters() {
        let filter = CompoundPolicy::new(
            AllowlistPolicy::new(vec![]),
            DenylistPolicy::new(vec!["write".to_string()]),
        );
        assert!(filter.is_allowed("read"));
        assert!(!filter.is_allowed("write"));
    }

    #[test]
    fn denylist_takes_precedence_over_allowlist() {
        let filter = CompoundPolicy::new(
            AllowlistPolicy::new(vec!["read".to_string(), "write".to_string()]),
            DenylistPolicy::new(vec!["write".to_string()]),
        );
        assert!(filter.is_allowed("read"));
        assert!(!filter.is_allowed("write"));
    }

    #[test]
    fn denied_tool_blocked_even_if_allowed() {
        let filter = CompoundPolicy::new(
            AllowlistPolicy::new(vec!["dangerous".to_string()]),
            DenylistPolicy::new(vec!["dangerous".to_string()]),
        );
        assert!(!filter.is_allowed("dangerous"));
    }
}
