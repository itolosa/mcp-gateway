/// Driven port: a provider entry with operation policy lists.
pub trait ProviderEntry: Send + Sync {
    fn allowed_operations(&self) -> &[String];
    fn allowed_operations_mut(&mut self) -> &mut Vec<String>;
    fn denied_operations(&self) -> &[String];
    fn denied_operations_mut(&mut self) -> &mut Vec<String>;
}
