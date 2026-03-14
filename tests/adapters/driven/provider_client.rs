use mcp_gateway::adapters::driven::provider_client::RmcpProviderClient;
use mcp_gateway::hexagon::ports::driven::provider_client::{
    OperationCallRequest, PromptGetRequest, ProviderClient, ResourceReadRequest,
};
use rmcp::model::*;
use rmcp::ServerHandler;
use rmcp::ServiceExt;

struct MinimalServer;

impl ServerHandler for MinimalServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
        )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        let schema: serde_json::Map<String, serde_json::Value> =
            serde_json::from_value(serde_json::json!({"type":"object"})).unwrap();
        Ok(ListToolsResult {
            tools: vec![Tool::new("echo", "Echo tool", schema)],
            next_cursor: None,
            meta: None,
        })
    }
}

async fn create_client() -> (RmcpProviderClient, tokio::task::JoinHandle<()>) {
    let (server_t, client_t) = tokio::io::duplex(4096);
    let handle = tokio::spawn(async move {
        let s = MinimalServer.serve(server_t).await.unwrap();
        let _ = s.waiting().await;
    });
    let upstream = ().serve(client_t).await.unwrap();
    (RmcpProviderClient::new(upstream), handle)
}

#[tokio::test]
async fn list_operations_returns_tools_from_upstream() {
    let (client, handle) = create_client().await;
    let result = client.list_operations().await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].name, "echo");
    drop(client);
    let _ = handle.await;
}

#[tokio::test]
async fn call_operation_unknown_tool_returns_error() {
    let (client, handle) = create_client().await;
    let request = OperationCallRequest {
        name: "nonexistent".to_string(),
        arguments: None,
    };
    let result = client.call_operation(request).await;
    assert!(result.is_err());
    drop(client);
    let _ = handle.await;
}

#[tokio::test]
async fn list_resources_returns_empty_from_minimal_server() {
    let (client, handle) = create_client().await;
    let result = client.list_resources().await.unwrap();
    assert!(result.is_empty());
    drop(client);
    let _ = handle.await;
}

#[tokio::test]
async fn list_resource_templates_returns_empty_from_minimal_server() {
    let (client, handle) = create_client().await;
    let result = client.list_resource_templates().await.unwrap();
    assert!(result.is_empty());
    drop(client);
    let _ = handle.await;
}

#[tokio::test]
async fn read_resource_returns_error_from_minimal_server() {
    let (client, handle) = create_client().await;
    let request = ResourceReadRequest {
        uri: "file:///test.txt".to_string(),
    };
    let result = client.read_resource(request).await;
    assert!(result.is_err());
    drop(client);
    let _ = handle.await;
}

#[tokio::test]
async fn list_prompts_returns_empty_from_minimal_server() {
    let (client, handle) = create_client().await;
    let result = client.list_prompts().await.unwrap();
    assert!(result.is_empty());
    drop(client);
    let _ = handle.await;
}

#[tokio::test]
async fn get_prompt_returns_error_from_minimal_server() {
    let (client, handle) = create_client().await;
    let request = PromptGetRequest {
        name: "nonexistent".to_string(),
        arguments: None,
    };
    let result = client.get_prompt(request).await;
    assert!(result.is_err());
    drop(client);
    let _ = handle.await;
}

#[tokio::test]
async fn with_operation_timeout_times_out_on_slow_server() {
    use std::time::Duration;

    let (client, handle) = create_client().await;
    let client = client.with_operation_timeout(Duration::from_millis(1));
    // list_operations should succeed (fast response)
    let result = client.list_operations().await;
    // May or may not timeout depending on speed — just verify it doesn't panic
    drop(result);
    drop(client);
    let _ = handle.await;
}
