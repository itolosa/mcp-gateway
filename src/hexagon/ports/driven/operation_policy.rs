/// Driven port: operation policy for provider operations.
pub trait OperationPolicy: Send + Sync {
    fn is_allowed(&self, operation_name: &str) -> bool;
}
