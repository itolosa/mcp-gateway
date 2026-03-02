mod allowlist;

pub use allowlist::AllowlistFilter;

pub trait ToolFilter: Send + Sync {
    fn is_tool_allowed(&self, tool_name: &str) -> bool;
}
