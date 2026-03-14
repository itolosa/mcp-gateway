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
