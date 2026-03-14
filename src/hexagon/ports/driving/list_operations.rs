#[derive(Debug, Clone)]
pub struct OperationDescriptor {
    pub name: String,
    pub description: Option<String>,
    pub schema: String,
}
