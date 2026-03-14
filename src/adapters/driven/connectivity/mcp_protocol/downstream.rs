use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, GetPromptRequestParams, GetPromptResult,
    ListPromptsResult, ListResourceTemplatesResult, ListResourcesResult, ListToolsResult,
    PaginatedRequestParams, ReadResourceRequestParams, ReadResourceResult, ServerInfo, Tool,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ErrorData, ServerHandler};

use crate::hexagon::ports::{
    CliOperationRunner, GatewayError, OperationCallRequest, OperationPolicy, PromptGetRequest,
    ProviderClient, ResourceReadRequest,
};
use crate::hexagon::usecases::gateway::Gateway;

pub struct McpAdapter<U: ProviderClient, C: CliOperationRunner, F: OperationPolicy> {
    gateway: Gateway<U, C, F>,
}

impl<U: ProviderClient, C: CliOperationRunner, F: OperationPolicy> McpAdapter<U, C, F> {
    pub fn new(gateway: Gateway<U, C, F>) -> Self {
        Self { gateway }
    }
}

fn gateway_error_to_mcp(err: GatewayError) -> ErrorData {
    match err {
        GatewayError::InvalidMapping { .. }
        | GatewayError::UnknownProvider { .. }
        | GatewayError::OperationNotAllowed { .. } => {
            ErrorData::invalid_params(err.to_string(), None)
        }
        _ => ErrorData::internal_error(err.to_string(), None),
    }
}

fn domain_content_to_mcp(content: Vec<String>) -> Vec<Content> {
    content
        .into_iter()
        .filter_map(|s| serde_json::from_str(&s).ok())
        .collect()
}

impl<
        U: ProviderClient + 'static,
        C: CliOperationRunner + 'static,
        F: OperationPolicy + 'static,
    > ServerHandler for McpAdapter<U, C, F>
{
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            rmcp::model::ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
        )
        .with_server_info(rmcp::model::Implementation::new(
            "mcp-gateway",
            env!("CARGO_PKG_VERSION"),
        ))
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let tools = self
            .gateway
            .list_operations()
            .await
            .map_err(gateway_error_to_mcp)?;
        let mcp_tools = tools
            .into_iter()
            .map(|t| {
                let schema: serde_json::Map<String, serde_json::Value> =
                    serde_json::from_str(&t.schema).unwrap_or_default();
                Tool::new(t.name, t.description.unwrap_or_default(), schema)
            })
            .collect();
        Ok(ListToolsResult {
            tools: mcp_tools,
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let domain_request = OperationCallRequest {
            name: request.name.to_string(),
            arguments: request
                .arguments
                .map(|m| serde_json::to_string(&m).unwrap_or_default()),
        };
        let result = self
            .gateway
            .route_operation(domain_request)
            .await
            .map_err(gateway_error_to_mcp)?;
        let content = domain_content_to_mcp(result.content);
        if result.is_error {
            Ok(CallToolResult::error(content))
        } else {
            Ok(CallToolResult::success(content))
        }
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        let resources = self
            .gateway
            .list_resources()
            .await
            .map_err(gateway_error_to_mcp)?;
        let mcp_resources = resources
            .into_iter()
            .filter_map(|r| serde_json::from_str(&r.json).ok())
            .collect();
        Ok(ListResourcesResult {
            resources: mcp_resources,
            next_cursor: None,
            meta: None,
        })
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, ErrorData> {
        let templates = self
            .gateway
            .list_resource_templates()
            .await
            .map_err(gateway_error_to_mcp)?;
        let mcp_templates = templates
            .into_iter()
            .filter_map(|t| serde_json::from_str(&t.json).ok())
            .collect();
        Ok(ListResourceTemplatesResult {
            resource_templates: mcp_templates,
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, ErrorData> {
        let domain_request = ResourceReadRequest {
            uri: request.uri.clone(),
        };
        let result = self
            .gateway
            .read_resource(domain_request)
            .await
            .map_err(gateway_error_to_mcp)?;
        serde_json::from_str(&result.json)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, ErrorData> {
        let prompts = self
            .gateway
            .list_prompts()
            .await
            .map_err(gateway_error_to_mcp)?;
        let mcp_prompts = prompts
            .into_iter()
            .filter_map(|p| serde_json::from_str(&p.json).ok())
            .collect();
        Ok(ListPromptsResult {
            prompts: mcp_prompts,
            next_cursor: None,
            meta: None,
        })
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, ErrorData> {
        let domain_request = PromptGetRequest {
            name: request.name.clone(),
            arguments: request
                .arguments
                .map(|m| serde_json::to_string(&m).unwrap_or_default()),
        };
        let result = self
            .gateway
            .get_prompt(domain_request)
            .await
            .map_err(gateway_error_to_mcp)?;
        serde_json::from_str(&result.json)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))
    }
}
