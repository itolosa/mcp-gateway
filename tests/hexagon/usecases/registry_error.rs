use mcp_gateway::hexagon::usecases::registry_error::RegistryError;

#[test]
fn already_exists_display() {
    let err = RegistryError::AlreadyExists {
        name: "test".to_string(),
    };
    assert!(err.to_string().contains("test"));
    assert!(err.to_string().contains("already exists"));
}

#[test]
fn not_found_display() {
    let err = RegistryError::NotFound {
        name: "test".to_string(),
    };
    assert!(err.to_string().contains("test"));
    assert!(err.to_string().contains("not found"));
}

#[test]
fn storage_error_display() {
    let err = RegistryError::Storage("disk full".to_string());
    assert!(err.to_string().contains("disk full"));
    assert!(err.to_string().contains("storage error"));
}
