#![allow(clippy::unwrap_used, clippy::expect_used)]

use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, ErrorData, Implementation, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{RoleServer, ServerHandler, ServiceExt};

struct MultiEchoServer;

fn message_schema() -> serde_json::Map<String, serde_json::Value> {
    serde_json::from_value(serde_json::json!({
        "type": "object",
        "properties": {
            "message": { "type": "string" }
        }
    }))
    .expect("static schema")
}

fn extract_message(request: &CallToolRequestParams) -> String {
    request
        .arguments
        .as_ref()
        .and_then(|a| a.get("message").cloned())
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_default()
}

impl ServerHandler for MultiEchoServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("multi-echo-test", "0.1.0"))
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let schema = message_schema();
        Ok(ListToolsResult::with_all_items(vec![
            Tool::new("echo", "echoes input", schema.clone()),
            Tool::new("reverse", "reverses input", schema.clone()),
            Tool::new("upper", "uppercases input", schema),
        ]))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let input = extract_message(&request);
        match request.name.as_ref() {
            "echo" => Ok(CallToolResult::success(vec![Content::text(input)])),
            "reverse" => Ok(CallToolResult::success(vec![Content::text(
                input.chars().rev().collect::<String>(),
            )])),
            "upper" => Ok(CallToolResult::success(vec![Content::text(
                input.to_uppercase(),
            )])),
            _ => Err(ErrorData::invalid_params(
                format!("unknown tool: {}", request.name),
                None,
            )),
        }
    }
}

#[tokio::main]
async fn main() {
    let service = MultiEchoServer
        .serve(rmcp::transport::io::stdio())
        .await
        .expect("serve failed");
    service.waiting().await.expect("server error");
}
