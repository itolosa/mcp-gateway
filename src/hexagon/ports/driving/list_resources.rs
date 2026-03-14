#[derive(Debug, Clone)]
pub struct ResourceDescriptor {
    pub uri: String,
    pub name: String,
    pub json: String,
}

#[derive(Debug, Clone)]
pub struct ResourceTemplateDescriptor {
    pub uri_template: String,
    pub name: String,
    pub json: String,
}
