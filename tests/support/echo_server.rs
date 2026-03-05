#![allow(clippy::unwrap_used, clippy::expect_used)]

use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, ErrorData, Implementation, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{RoleServer, ServerHandler, ServiceExt};

struct EchoServer;

impl ServerHandler for EchoServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("echo-test", "0.1.0"))
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let schema: serde_json::Map<String, serde_json::Value> =
            serde_json::from_value(serde_json::json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string" }
                }
            }))
            .expect("static schema");
        Ok(ListToolsResult::with_all_items(vec![Tool::new(
            "echo",
            "echoes input",
            schema,
        )]))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        if request.name.as_ref() == "echo" {
            let input = request
                .arguments
                .and_then(|a| a.get("message").cloned())
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_default();
            Ok(CallToolResult::success(vec![Content::text(input)]))
        } else {
            Err(ErrorData::invalid_params(
                format!("unknown tool: {}", request.name),
                None,
            ))
        }
    }
}

#[tokio::main]
async fn main() {
    let service = EchoServer
        .serve(rmcp::transport::io::stdio())
        .await
        .expect("serve failed");
    service.waiting().await.expect("server error");
}
