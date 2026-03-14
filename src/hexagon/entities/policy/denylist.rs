use std::collections::HashSet;

use crate::hexagon::ports::driven::operation_policy::OperationPolicy;

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
