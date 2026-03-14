#![allow(clippy::unwrap_used, clippy::expect_used)]

use rmcp::model::{
    Annotated, CallToolRequestParams, CallToolResult, Content, ErrorData, GetPromptRequestParams,
    GetPromptResult, Implementation, ListPromptsResult, ListResourceTemplatesResult,
    ListResourcesResult, ListToolsResult, PaginatedRequestParams, Prompt, PromptMessage,
    PromptMessageRole, RawResource, RawResourceTemplate, ReadResourceRequestParams,
    ReadResourceResult, ResourceContents, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{RoleServer, ServerHandler, ServiceExt};

struct EchoServer;

impl ServerHandler for EchoServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
        )
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

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        Ok(ListResourcesResult::with_all_items(vec![Annotated::new(
            RawResource::new("file:///hello.txt", "hello.txt"),
            None,
        )]))
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, ErrorData> {
        let template = RawResourceTemplate {
            uri_template: "file:///{path}".to_string(),
            name: "file-template".to_string(),
            title: None,
            description: None,
            mime_type: None,
            icons: None,
        };
        Ok(ListResourceTemplatesResult::with_all_items(vec![
            Annotated::new(template, None),
        ]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, ErrorData> {
        Ok(ReadResourceResult::new(vec![ResourceContents::text(
            format!("content of {}", request.uri),
            request.uri.clone(),
        )]))
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, ErrorData> {
        Ok(ListPromptsResult::with_all_items(vec![Prompt::new(
            "greet",
            Some("A greeting prompt"),
            None,
        )]))
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, ErrorData> {
        Ok(GetPromptResult::new(vec![PromptMessage::new_text(
            PromptMessageRole::Assistant,
            format!("Hello from {}", request.name),
        )])
        .with_description(format!("Prompt: {}", request.name)))
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
