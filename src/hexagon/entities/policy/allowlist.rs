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
