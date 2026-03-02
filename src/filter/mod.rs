mod allowlist;
mod compound;
mod denylist;

pub use allowlist::AllowlistFilter;
pub use compound::CompoundFilter;
pub use denylist::DenylistFilter;

pub trait ToolFilter: Send + Sync {
    fn is_tool_allowed(&self, tool_name: &str) -> bool;
}
