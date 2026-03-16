use rmcp::{
    ErrorData as McpError, ServerHandler, handler::server::tool::ToolRouter,
    handler::server::wrapper::Parameters, model::*, service::ServiceExt, tool, tool_handler,
    tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
struct IndicesArgs {
    #[schemars(description = "Worktree indices to operate on (all if empty/null)")]
    indices: Option<Vec<usize>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct PromoteArgs {
    #[schemars(description = "Worktree index (1-indexed)")]
    index: usize,
    #[schemars(description = "Show plan without executing")]
    dry_run: Option<bool>,
}

#[derive(Clone)]
pub struct RftMcpServer {
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl RftMcpServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        name = "rft_start",
        description = "Start Docker Compose stacks for worktrees"
    )]
    async fn start(
        &self,
        Parameters(args): Parameters<IndicesArgs>,
    ) -> Result<CallToolResult, McpError> {
        let indices = args.indices.unwrap_or_default();
        match crate::commands::start::run(indices, false).await {
            Ok(()) => Ok(CallToolResult::success(vec![Content::text(
                "Started successfully",
            )])),
            Err(error) => Ok(CallToolResult::error(vec![Content::text(
                error.to_string(),
            )])),
        }
    }

    #[tool(
        name = "rft_stop",
        description = "Stop Docker Compose stacks for worktrees"
    )]
    async fn stop(
        &self,
        Parameters(args): Parameters<IndicesArgs>,
    ) -> Result<CallToolResult, McpError> {
        let indices = args.indices.unwrap_or_default();
        match crate::commands::stop::run(indices).await {
            Ok(()) => Ok(CallToolResult::success(vec![Content::text(
                "Stopped successfully",
            )])),
            Err(error) => Ok(CallToolResult::error(vec![Content::text(
                error.to_string(),
            )])),
        }
    }

    #[tool(
        name = "rft_restart",
        description = "Restart Docker Compose stacks for worktrees"
    )]
    async fn restart(
        &self,
        Parameters(args): Parameters<IndicesArgs>,
    ) -> Result<CallToolResult, McpError> {
        let indices = args.indices.unwrap_or_default();
        match crate::commands::restart::run(indices).await {
            Ok(()) => Ok(CallToolResult::success(vec![Content::text(
                "Restarted successfully",
            )])),
            Err(error) => Ok(CallToolResult::error(vec![Content::text(
                error.to_string(),
            )])),
        }
    }

    #[tool(
        name = "rft_list",
        description = "List all worktrees with ports and status"
    )]
    async fn list(&self) -> Result<CallToolResult, McpError> {
        match crate::commands::list::list_as_text().await {
            Ok(text) => Ok(CallToolResult::success(vec![Content::text(text)])),
            Err(error) => Ok(CallToolResult::error(vec![Content::text(
                error.to_string(),
            )])),
        }
    }

    #[tool(
        name = "rft_promote",
        description = "Promote changes from a worktree to current branch"
    )]
    async fn promote(
        &self,
        Parameters(args): Parameters<PromoteArgs>,
    ) -> Result<CallToolResult, McpError> {
        let dry_run = args.dry_run.unwrap_or(false);
        match crate::commands::promote::run(args.index, dry_run, None).await {
            Ok(()) => {
                let action = if dry_run {
                    "Dry run completed"
                } else {
                    "Promoted successfully"
                };
                Ok(CallToolResult::success(vec![Content::text(action)]))
            }
            Err(error) => Ok(CallToolResult::error(vec![Content::text(
                error.to_string(),
            )])),
        }
    }

    #[tool(
        name = "rft_clean",
        description = "Stop all stacks, remove worktrees, clean up Docker resources"
    )]
    async fn clean(&self) -> Result<CallToolResult, McpError> {
        match crate::commands::clean::run().await {
            Ok(()) => Ok(CallToolResult::success(vec![Content::text(
                "Clean completed",
            )])),
            Err(error) => Ok(CallToolResult::error(vec![Content::text(
                error.to_string(),
            )])),
        }
    }
}

#[tool_handler]
impl ServerHandler for RftMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::default())
            .with_server_info(Implementation::new("rft", env!("CARGO_PKG_VERSION")))
            .with_instructions("rft — zero-config Docker Compose isolation for git worktrees")
    }
}

pub async fn run_mcp_server() -> crate::error::Result<()> {
    let server = RftMcpServer::new();
    let service = server
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|error| {
            crate::error::RftError::Config(format!("MCP server failed to start: {error}"))
        })?;
    service
        .waiting()
        .await
        .map_err(|error| crate::error::RftError::Config(format!("MCP server error: {error}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_construction_initializes_tool_router() {
        let server = RftMcpServer::new();
        let tools = server.tool_router.list_all();
        assert_eq!(tools.len(), 6);
    }

    #[test]
    fn tool_router_contains_expected_tool_names() {
        let server = RftMcpServer::new();
        let tools = server.tool_router.list_all();
        let names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
        assert!(names.iter().any(|n| n == "rft_start"), "missing rft_start");
        assert!(names.iter().any(|n| n == "rft_stop"), "missing rft_stop");
        assert!(
            names.iter().any(|n| n == "rft_restart"),
            "missing rft_restart"
        );
        assert!(names.iter().any(|n| n == "rft_list"), "missing rft_list");
        assert!(
            names.iter().any(|n| n == "rft_promote"),
            "missing rft_promote"
        );
        assert!(names.iter().any(|n| n == "rft_clean"), "missing rft_clean");
    }

    #[test]
    fn tool_descriptions_are_non_empty() {
        let server = RftMcpServer::new();
        let tools = server.tool_router.list_all();
        for tool in &tools {
            assert!(
                tool.description.as_ref().is_some_and(|d| !d.is_empty()),
                "tool {} has empty description",
                tool.name
            );
        }
    }

    #[test]
    fn promote_tool_has_required_parameters() {
        let server = RftMcpServer::new();
        let tools = server.tool_router.list_all();
        let promote_tool = tools
            .iter()
            .find(|t| t.name == "rft_promote")
            .expect("rft_promote not found");
        let schema = promote_tool.input_schema.as_ref();
        let properties = schema.get("properties").expect("no properties in schema");
        assert!(properties.get("index").is_some(), "missing index parameter");
        assert!(
            properties.get("dry_run").is_some(),
            "missing dry_run parameter"
        );
    }

    #[test]
    fn server_info_returns_correct_name() {
        let server = RftMcpServer::new();
        let info = server.get_info();
        assert_eq!(info.server_info.name, "rft");
        assert!(!info.server_info.version.is_empty());
    }
}
